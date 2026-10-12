"""Stop an actual public-fixture ASR warm-up on an AVX2 integer-dot instruction.
Diagnostic only: no timing/accuracy result. GDB intentionally terminates the child.
"""
import argparse,pathlib,re,subprocess
p=argparse.ArgumentParser();p.add_argument('root');a=p.parse_args();root=pathlib.Path(a.root).resolve()
binary=root/'native-lp64-target/release/qwen-native-bench';out=root/'cpu-audit';out.mkdir(exist_ok=True)
syms=[]
for line in subprocess.check_output(['nm','-n',str(binary)],text=True).splitlines():
 m=re.match(r'([0-9a-f]+) [tT] (.+)$',line)
 if m:syms.append((int(m[1],16),m[2]))
i,addr,sym=next((i,addr,sym) for i,(addr,sym) in enumerate(syms) if 'avx' in sym and 'matvec_int8' in sym)
asm=subprocess.check_output(['objdump','-d',f'--start-address={addr}',f'--stop-address={syms[i+1][0]}',str(binary)],text=True)
ip=int(re.search(r'^\s*([0-9a-f]+):.*\bvpmaddwd\b',asm,re.M)[1],16)
commands=out/'gdb.commands'
commands.write_text(f'''set pagination off
set confirm off
set language c
set print thread-events off
tbreak *'{sym}'+{ip-addr}
run
info symbol $pc
x/i $pc
bt 6
info threads
set scheduler-locking step
stepi
x/i $pc
quit
''')
completed=subprocess.run(['gdb','--batch','--quiet','-x',str(commands),'--args',str(binary),str(root/'artifacts/model-0.6B'),'6','offline',str(root/'results/native-0.6B-t6-pilot-r3.tsv'),'1'],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=120)
(out/'gdb-runtime.txt').write_text(completed.stdout)
print(completed.stdout)
assert completed.returncode==0,completed.returncode
assert 'Temporary breakpoint' in completed.stdout and 'vpmaddwd' in completed.stdout,'Actual integer-dot breakpoint did not fire'
