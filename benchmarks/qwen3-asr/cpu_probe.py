"""Read-only Linux CPU/BLAS/ELF probe; no model inference or system configuration."""
import argparse,ctypes,hashlib,json,os,pathlib,re,subprocess,tempfile
p=argparse.ArgumentParser();p.add_argument('root');a=p.parse_args()
root=pathlib.Path(a.root).resolve();out=root/'cpu-audit';out.mkdir(exist_ok=True)
binary=root/'native-lp64-target/release/qwen-native-bench'
ldd=subprocess.check_output(['ldd',str(binary)],text=True)
libpath=re.search(r'libopenblas[^ ]* => ([^ ]+)',ldd).group(1)
b=ctypes.CDLL(libpath);info={}
for key in ['openblas_get_config','openblas_get_corename']:
 f=getattr(b,key);f.restype=ctypes.c_char_p;info[key]=f().decode()
assert 'USE64BITINT' not in info['openblas_get_config'],'Rust i32 FFI requires LP64'
info['openblas_threads']=b.openblas_get_num_threads()
with tempfile.TemporaryDirectory(dir=out) as tmp:
 c=pathlib.Path(tmp)/'abi.c';exe=pathlib.Path(tmp)/'abi'
 c.write_text('#include <stdio.h>\n#include <cblas.h>\nint main(void) {printf("%zu\\n",sizeof(blasint));}\n')
 flags=subprocess.check_output(['pkg-config','--cflags','openblas'],text=True).split()
 subprocess.run(['gcc',str(c),*flags,'-o',str(exe)],check=True)
 info['c_header_blas_integer_bits']=int(subprocess.check_output([str(exe)]))*8
 assert info['c_header_blas_integer_bits']==32
info['binary_sha256']=hashlib.sha256(binary.read_bytes()).hexdigest()
info['openblas_sha256']=hashlib.sha256(pathlib.Path(libpath).read_bytes()).hexdigest()
info['linked_openblas']=libpath;info['ldd']=ldd
info['lscpu']=json.loads(subprocess.check_output(['lscpu','--json'],text=True))
info['topology']=subprocess.check_output(['lscpu','-p=CPU,CORE,SOCKET'],text=True)
info['allowed_cpus']=sorted(os.sched_getaffinity(0))
info['rust_features']=subprocess.check_output(['rustc','--print','cfg','-C','target-cpu=znver3'],text=True)
for key,path in [('governor','/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor'),('thp_policy','/sys/kernel/mm/transparent_hugepage/enabled')]:
 if pathlib.Path(path).exists():info[key]=pathlib.Path(path).read_text().strip()
syms=[]
for line in subprocess.check_output(['nm','-n',str(binary)],text=True).splitlines():
 m=re.match(r'([0-9a-f]+) [tT] (.+)$',line)
 if m:syms.append((int(m[1],16),m[2]))
info['kernel_assembly']={}
for name in ['matvec_int8','argmax_int8_range','matvec_f32']:
 found=[(i,addr,sym) for i,(addr,sym) in enumerate(syms) if 'avx' in sym and name in sym]
 if not found:continue
 i,addr,sym=found[0];stop=syms[i+1][0]
 assembly=subprocess.check_output(['objdump','-d','-C',f'--start-address={addr}',f'--stop-address={stop}',str(binary)],text=True)
 (out/f'{name}.asm').write_text(assembly)
 info['kernel_assembly'][name]={'symbol':sym,'start':hex(addr),'instructions':sorted(set(re.findall(r'\b(vpmovsxbw|vpmaddwd|vfmadd\w*ps)\b',assembly)))}
assert {'vpmovsxbw','vpmaddwd'}.issubset(info['kernel_assembly']['matvec_int8']['instructions'])
(out/'probe.json').write_text(json.dumps(info,ensure_ascii=False,indent=2)+'\n')
print(json.dumps({k:v for k,v in info.items() if k not in ['ldd','lscpu','topology','rust_features']},ensure_ascii=False,indent=2))
