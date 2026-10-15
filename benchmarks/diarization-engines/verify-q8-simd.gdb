# Run only the isolated public-fixture diagnostic process, never attach to an app.
set pagination off
set confirm off
set breakpoint pending on
break _ZN12_GLOBAL__N_115tinyBLAS_Q0_AVXI10block_q8_0S1_fE7gemm4xNILi2EEEvllll
run
bt 8
python
import gdb, re
asm = gdb.execute('disassemble _ZN12_GLOBAL__N_115tinyBLAS_Q0_AVXI10block_q8_0S1_fE7gemm4xNILi2EEEvllll', to_string=True)
m = re.search(r'(0x[0-9a-f]+)\s+<[^>]+>:\s+(vpmaddwd|vpmaddubsw)\b', asm)
if not m:
    print(asm)
    raise gdb.GdbError('No expected AVX2 integer-dot instruction found')
print('SIMD target:', m.group(0))
gdb.execute('disable 1')
gdb.execute('tbreak *' + m.group(1))
end
continue
x/i $pc
stepi
x/i $pc
quit
