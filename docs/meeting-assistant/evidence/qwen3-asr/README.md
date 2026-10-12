# Public-only Qwen3-ASR evidence

All transcripts/references here are public FLEURS reading fixtures or synthetic silence. No WAV, model, private meeting, credential or database is included. FLEURS: Conneau et al. (2022), Google FLEURS, CC-BY-4.0; dataset/model download sources and SHA256 are in the fixture manifest / benchmark preparation provenance. Audio/weights/builds and full run metadata remain outside Git in `../runtime/qwen3-asr/`.

- `validated-summary.json`: formal corrected LP64 6-thread BF16/C vs decoder-INT8/Rust accuracy, timing-prefix medians, paced results, measured pilot inference/CPU totals. Invalid initial ILP64 Rust results are not included.
- `*-t6-accuracy-r1.jsonl`: all 30 speech + direct silence raw measured outputs. Model load / warm-up excluded from inference duration; process max RSS includes both.
- `native-*-t5-accuracy.report.json`: additional 5-thread complete scorer reports. Thread count changed some recognition output; do not equate faster with identical quality.
- `fixture-manifest.json`: fixed public references, IDs, durations, SHA256, language and timing-only fixture metadata. Google FLEURS row API did not expose the dataset Git revision; individual downloaded bytes are hashed. Thai validation was extracted from direct Parquet after the API exceeded its scan limit.
- Display-only trailing whitespace from GDB/objdump is trimmed in committed text; original diagnostic output is retained outside Git. Instruction bytes/text and measured results are unchanged.
- `cpu-probe.json`, `matvec_int8.asm`: actual LP64 library/header integer width, hardware/topology, library dispatch, flags and AVX2 integer-dot disassembly.
- `gdb-runtime.txt`: actual Japanese ASR warm-up hits `vpmaddwd` in the INT8 matvec function; one instruction step and active thread snapshot. Diagnostic terminated before completion; NOT timing/accuracy evidence.
- `qwen-ilp64-rejection.stderr`: controlled negative test of old incompatible library. Refuses before model load.

Binary snapshots/roles (all upstream engine/model math unchanged):

| Role | SHA256 |
|---|---|
| Corrected C runner, primary BF16 data | `19abc259390a58eee9871db430da7392f5d761b7c099eb562d4cc50d494cae42` |
| Corrected native runner, primary 6-thread data, before per-clip CPU-time instrumentation | `d2a51ddf55c045566c3c4ec87812245a9438a59718e3de3420303ae5ab44ec00` |
| Native runner with CPU-time instrumentation, optimization confirmations / 5-thread accuracy / GDB snapshot | `05b2ccb95c7b6a0b5e75315bf6215361f513012ebb82a5d294a8940b67dd8c3e` |
| Same measured source after rustfmt + final successful build check | `e11173b4db8547977be5ac3521513e82c4af0000005d40a1e23771d1986200de` |

The primary native binary is retained externally as `native-lp64-target/release/qwen-native-bench.before-cpu-counters`. Primary JSONL intentionally has no `cpu_seconds` field; later pilots/5-thread JSONL do. Do not infer a single identical binary hash for all wrapper-instrumentation phases. Later driver metadata captures executable SHA256 before spawn.

Metric regression tests: 5 passed. Upstream `qwen-asr` release library tests with `--no-default-features --features blas`, `target-cpu=znver3`, LP64: 32 passed, 0 failed. Matvec/argmax integer-dot scalar-reference tests are included. Public evidence is not meeting/Windows/long-duration acceptance.
