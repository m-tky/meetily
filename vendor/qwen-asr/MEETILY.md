# Meetily's pinned QwenASR engine

Source: https://github.com/huanglizhuo/QwenASR
Revision: `37c1e35be333945dc9b5771a6a33fe4b2064533f`
Subdirectory: `crates/qwen-asr`, version 0.11.0, MIT (see LICENSE).
`UPSTREAM.json` records SHA256 of every original file, including the upstream root license.
Only the required crate is retained; there are no model weights or build artifacts.

Local changes:
- Cargo workspace-inherited edition/license become explicit; standalone workspace.
- Windows-only pinned memmap2 0.9.11 for read-only RAII model/INT8 mappings.
- Windows positional reads/writes use FileExt with partial/interrupted I/O loops.
- Windows uses Rust-owned weight allocations, not POSIX/CRT buffers freed by Vec.
- Windows thread backoff uses std::thread::yield_now; Unix behavior is retained.
- Windows prefetch retains bounded page touches but omits Unix madvise hints.
- Windows BLAS requires an explicit MSVC LP64 OpenBLAS import-library directory
  (`QWEN_OPENBLAS_LIB_DIR`; optional library name `QWEN_OPENBLAS_LIB_NAME`, default
  `libopenblas`). There is no implicit scalar fallback in the worker.
- Synthetic portability integration tests cover Unicode/apostrophe paths,
  concurrent reads, mapping release, INT8 sidecar write/reopen/corruption, and
  large allocations, and rectangular/transposed GEMM. With `blas`, the GEMM
  test calls cblas_sgemm directly; otherwise it exercises the scalar path.
  No models, devices, network or inference are used.

Linux mmap/pread/madvise, BLAS and allocation paths are intentionally retained.
The existing external runtime checkout and already deployed Linux worker are not
modified. Updating this copy requires explicit review of upstream changes, local
patches, provenance hashes, tests and recognition regressions.

From the repo root, run the synthetic tests directly (the crate's small
standalone Cargo.lock pins the same libc/memmap2 versions as the worker):

```sh
cargo test --locked --manifest-path vendor/qwen-asr/Cargo.toml \
  --no-default-features --test meetily_portability
cargo test --locked --manifest-path benchmarks/qwen3-asr/worker/Cargo.toml
```

Do not count a dependency-package test command's zero exit code alone as proof:
verify the actual test count (5 on Linux, 6 on Windows). The direct tests do not
require BLAS. Windows additionally requires the LP64 OpenBLAS DLL on the process
search path when using the worker's `blas` feature. Build/test success is not ASR accuracy,
application integration, installer or redistribution acceptance.
