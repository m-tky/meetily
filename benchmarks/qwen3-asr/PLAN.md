# Qwen3-ASR 5650GE CPU benchmark

User requested actual 0.6B/1.7B runs, Japanese/English/Thai accuracy, RTF, memory and latency, unquantized first, quantization if needed, Rust preferred, and implementation after evidence. Existing ASR must remain unchanged.

## Isolation and methodology

- Everything under this directory is an independent benchmark. No changes to Meetily capture/providers/schema/locks. Downloaded engines, venv, models, audio, build products and raw logs live in the sibling `runtime/qwen3-asr` / `logs` directories.
- Public FLEURS `validation`, fixed first 10 rows each of `ja_jp`, `en_us`, `th_th`. No microphone/system capture, playback, cloud inference or app authentication.
- Primary accuracy: CER for Japanese/Thai, WER for English (also report English CER). Explicit NFKC/case/punctuation/whitespace normalization; Thai uses Unicode codepoints, not grapheme clusters. No normalization of digit readings or word synonyms.
- Offline RTF excludes load/file conversion. Model loaded once, one untimed warm-up, sequential batch size 1. No simultaneous ASR processes. Peak RSS via Linux getrusage includes model loading, warm-up and activations; it is process high-water, not only tensor allocation. Report MiB/GiB consistently.
- 2/4/8s prefixes per language are timing-only; partial transcripts have no asserted complete-sentence reference. Three repetitions, median/min/max, first non-whitespace token where engine exposes callback. First token != complete/stable sentence.
- Separately test prerecorded audio paced into streaming input at wall-clock arrival times. Report acquisition wait separately from compute, stable first output and delay after last sample. Offline/file-stream throughput is not live-stream latency.
- Rust release / C -O3, CPU only, AVX2/FMA on Zen 3. Screen thread counts 3/6/12, then compare engines at matched settings. No claim of globally fastest engine without testing every candidate. Background desktop apps remain running; record that the machine is not dedicated.
- Target for a first live candidate: sustained compute RTF <1, plus observed end delay and memory headroom. A value below 1 alone is not acceptance of live meeting reliability.

## Engines and pinned upstreams

| Engine | Revision | Baseline precision / role |
|---|---|---|
| [antirez/qwen-asr](https://github.com/antirez/qwen-asr) | `924694251d9e0f18e5d86bbd06aa3ab5f870002d` | Unquantized original BF16 weights, F32 encoder/compute, BF16 decoder matvec. OpenBLAS + AVX2/FMA. Persistent C runner. |
| [alan890104/qwen3-asr-rs](https://github.com/alan890104/qwen3-asr-rs) | `c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc` | Unquantized BF16 weights converted to F32 on CPU, Candle 0.9.2, no GPU/hub features. Persistent Rust runner. |
| [huanglizhuo/QwenASR](https://github.com/huanglizhuo/QwenASR) | `37c1e35be333945dc9b5771a6a33fe4b2064533f` | **INT8 decoder**, F32 encoder/prefill; not an unquantized baseline even though source weights are BF16. Persistent Rust runner and paced incremental API. |

Models: official `Qwen/Qwen3-ASR-{0.6B,1.7B}`, Apache-2.0. Pin downloaded revisions and SHA256 in provenance. Engine code is MIT. No GGUF/GPU benchmark numbers imported into the CPU result table.

## Preparation findings / failures

- Official weights do not include `tokenizer.json` expected by Candle. Generate it locally from official vocab/merges/tokenizer_config with Transformers 4.57.6; record derived hash. No remote code or Hub inference. Transformer tokenizer conversion emits a regex warning; retain it, do not treat compatibility as verified until real inference.
- Thai row server returns 500/501: 327,250,155-byte row group exceeds 300,000,000-byte scan limit. Fetch/hash the published 327,258,380-byte Thai validation Parquet directly and extract exactly the first ten rows with PyArrow. Do not replace Thai with another language/split.
- Pure-Rust upstream CLI fails to build on Linux: missing `Ordering` import at `crates/qwen-asr-cli/src/main.rs:677`. Its library builds; use our independent API runner, leaving the pinned upstream clone unchanged.
- Our first Rust runner used the wrong `stream_push_audio` argument order; corrected after inspecting its signature. The API requires the full accumulated buffer, not only the newest chunk; final flag must accompany the last real prefix so rollback text is flushed.
- Nix GCC discarded `-march=native` in the first C build. Rebuilt explicitly with `-march=znver3` so AVX2/FMA kernels are compiled. The first build is not the measured optimized engine.
- Initial Nix benchmark shell download wait was bypassed for public downloads by using the already-realized Meetily shell; no system changes. A slow single-connection model fetch was stopped and replaced with bounded 8-worker HTTP Range downloads. Each Content-Range/size is checked and assembled file SHA256 must match official LFS metadata.

## Reproduction outline

Use pinned Nixpkgs `/nix/store/8vdpwi2yls679v4dnjcp3wynsq77jwhl-source` and `nix-shell benchmarks/qwen3-asr/shell.nix`. Clone the above revisions outside the repo using the names `c-native`, `candle-native`, `rust-native`. Local path dependencies intentionally refer to that external workspace.

1. `node benchmarks/qwen3-asr/prepare.mjs ../runtime/qwen3-asr/artifacts models`
2. Create external venv; install PyArrow 20.0.0 / Transformers 4.57.6. For audio prep set `BENCH_PYTHON` to that venv interpreter; run prepare with `audio`.
3. Generate local tokenizer with `prepare_tokenizer.py`. Retain source/derived hashes and actual venv package freeze.
4. Build C `make blas CFLAGS_BASE='-Wall -Wextra -O3 -march=znver3 -ffast-math'`; link `c_runner.c` against all upstream `.o` except `main.o`, `-lopenblas -lm -lpthread`.
5. Build standalone Rust runner manifests in release with `RUSTFLAGS='-C target-cpu=native'`. Each has its own workspace and lock, separate from app Cargo.lock.
6. Validate fixture/model hashes, run sequentially with explicit thread/repetition/mode parameters, and score raw JSONL using `metrics.mjs`.

## Measurement/decision checkpoint

- Actual unquantized C/Candle and decoder-INT8 Rust runs completed for both model sizes; speech accuracy uses 30 fixed public files (319.16s), plus 2/4/8s timing prefixes and direct 5s silence. C++ mixed-Q4 GGUF (CrispASR `8d45f639cc22e6bd35d51b696215b4125ce0cc08`, ggml `c36dab89b662838f0f5d4826c399198c0b90bbfc`) also ran; third-party artifact hashes/revisions are pinned but original BF16 source revision is unpublished.
- Candle forced-language API includes a leading `<asr_text>` control marker. The runner/scorer removes ONLY that exact leading protocol marker and preserves raw output; never normalise away semantic recognition errors.
- Native Rust's intermediate 32-token budget truncated the Thai streamed tail. Preserve that failed run under `results/exploratory/stream-final-32/`; final call now uses 512 tokens. A separate end-of-utterance offline reconciliation test avoids irrevocably keeping incorrect prefixes (e.g. streamed Japanese `海岸` vs reference `海外`). Provisional != immutable correct evidence.
- All tested variants hallucinate nonempty text on direct silence. Small Q4 repeats `世界` up to the 512-token budget. No VAD/noise/long-meeting/Windows acceptance is claimed.
- User chose **INT8**, asking for actual 5650GE optimisation verification rather than assuming `target-cpu=native` is enough. After the corrected BF16/INT8 comparison, user selected **1.7B decoder-INT8** and asked how to mitigate hallucination. Application integration remains unverified. C++ Q4 is comparison evidence, NOT the approved implementation path.
- CPU verification found x86 AVX2 INT8 dispatch with exact sign-extension + `vpmaddwd` (Zen 3 has no VNNI/BF16 hardware dot), release thin-LTO/codegen-units=1, OpenBLAS runtime core `Zen`, and 6 threads faster than 3/12 in preliminary screens.
- **ABI defect discovered:** nixpkgs x86_64 default OpenBLAS uses ILP64 (`USE64BITINT`), but the upstream Rust CBLAS declarations use i32 dimensions. Those initial native-Rust timings/accuracy are archived under `results/exploratory/invalid-blas-abi/` and MUST NOT be used as valid recommendation evidence. No observed success proves this ABI safe. Benchmark shell now explicitly selects `blas64=false` (LP64). Corrected C/INT8 builds and both-size full comparisons completed. Same release upstream tests passed 32/32; old ILP64 library was rejected before model load. Actual public-fixture GDB execution hit/stepped AVX2 `vpmaddwd`, not merely source/binary availability. Additional 1/4/5/8-thread, OpenMP wait, affinity and alternating 5/6 trials completed. 5-thread full accuracy reruns changed output, so timing-only tuning does not prove equivalent quality. C's header-based original BLAS binding was internally consistent; new LP64 C builds will be used for matched comparison.
- User additionally requested **16bit** measurements. First compare official, nonquantized BF16 storage (C CPU runtime expands/accumulates in F32) against corrected decoder INT8. BF16/FP16 storage must not be described as native 16bit arithmetic on 5650GE.

Results, CPU verification, corrected 16bit/INT8 comparison and limitations: [../../docs/meeting-assistant/QWEN3-ASR-CPU.md](../../docs/meeting-assistant/QWEN3-ASR-CPU.md). Formal 6-thread weighted RTF: BF16/C 0.6B **0.278**, 1.7B **0.610**; decoder-INT8/Rust **0.186**, **0.419**. Initial ABI-invalid INT8 numbers are superseded. This is a bounded fixture experiment, not a completed global optimal-runtime/production recommendation. Existing application capture/ASR/locks remain unchanged. No real microphone, desktop capture, playback, credentials or network inference were used.
