# Independent 1.7B decoder-INT8 speech-gated worker (prototype)

User selected 1.7B INT8 and approved the hallucination-mitigation plan. This is a **Linux-only, bounded local-file prototype**, not a replacement for Meetily's recorder/provider. No microphone, desktop capture, playback, network inference, credential access or DB migration. Windows/live drafts/long-meeting operation are not implemented or accepted.

## What it does

- Reuses the **same pinned `emotechlab/silero-rs` revision `26a6460`** used by Meetily, including its embedded ONNX model and endpointing algorithm. Does not substitute RMS-only speech decisions or force raw audio through on a VAD miss.
- Initial parameters: 16kHz / 30ms frames, 0.50 positive / 0.35 negative thresholds, 90ms minimum, 500ms redemption, 300ms pre / 200ms post padding. Experimental worker values, NOT a change to app VAD parameters (app uses 250ms minimum).
- Bounded VAD-only gain targets a peak of 0.1 with a 20x ceiling; silence remains zero. Original PCM goes to ASR unchanged. This is per-file peak lookahead, not an online AGC implementation. Noise is still classified by the neural VAD, not by energy. Gain can amplify real background noise: realistic mixtures remain unverified.
- New VAD session per bounded request. Upstream `reset()` doesn't clear all audio/timestamp buffers, so blindly reusing it could carry previous audio into silence handling. VAD session is dropped before ASR runs.
- **No speech → zero ASR calls**, including after a previous speech request. No empty final flush, no prior transcript carry.
- Lazily loads the selected 1.7B model once after the first qualified speech. Preserves immutable weights in one process; each bounded recognition window uses a fresh KV/encoder/prompt session, forced Japanese/English/Thai, no past-text conditioning, no energy-based silence compaction. Intervals refer to original samples, not an invented word-level alignment. Overlapping padded intervals merge to avoid duplicate audio. Within a bounded request, recognition uses one window spanning the first through last qualified interval, preserving internal pauses/context. Purely unqualified inputs still make zero ASR calls. This is not a live endpointing policy.
- Final incomplete VAD frame is padded for inference only; EOF interval is clamped to the **real original endpoint**. Long speech is bounded by rejecting files over 30s, not silently truncating them.
- Raw output is retained. Repeated text, empty output, oversized text, near-generation-limit and **sub-1s speech intervals** get review flags. Short `はい`/`うん` are not blacklisted/deleted. Duration flags can include legitimate short answers; they are not hallucination detection certainty.
- All machine output is `confirmed=false`. `window_final` means this recognition pass ended, NOT correct/approved meeting evidence. No draft is emitted in this first file prototype.

## Protocol / isolation

Stdout JSONL protocol version 1; stderr is separate. Ready handshake includes limits. Requests must end in newline, at most 4096 bytes, unique ASCII IDs (1..64 chars). Unknown fields/operations reject; truncated/oversized framing terminates worker. At most 1024 IDs retained per worker lifetime, then restart required. One input is mono 16kHz PCM16 WAV, nonempty and at most 30 seconds. File paths are local trusted-caller inputs, not a remote API/sandbox.

```json
{"op":"transcribe","id":"u-001","wav":"/absolute/public-fixture.wav","language":"Japanese"}
{"op":"ping","id":"health-001"}
{"op":"shutdown","id":"stop-001"}
```

Statuses: `no_speech` (empty text/zero calls), `recognized`, `review_required`, `error`; all recognition responses carry `confirmed=false`. Segments carry start/end samples, raw text, review reasons and inference time. `vad_gain` is diagnostic, not confidence. Machine confidence is **not fabricated** from text length or VAD segment duration.

The Python `worker_client.py` is an experimental local supervisor: single-flight/nonblocking busy rejection (no unbounded queue), response ID match, 128KiB response limit, 18GiB child address-space cap, deadline. Timeout/protocol failure/cancel **kills and reaps only its owned child**, and the next request reloads a fresh worker; it does not retry failed utterances automatically or touch recording. This is whole-worker cancellation, not in-decoder cooperative cancellation. A restart incurs model reload. Not connected to Tauri.

Native offline loop currently has a fixed 2048 generated-token cap, but exposes only text-token count, not EOS. >=1900 text tokens is a conservative review flag, **not proof of EOS/complete recognition**. The supervisor deadline is the hard compute bound. Mid-utterance segmentation/live queue policy needs separate validation.

## Build / testing

All build products/audio/weights stay in `../runtime/qwen3-asr`, not Git. Existing app Cargo/pnpm locks stay unchanged. New standalone Cargo.lock is frozen after generation.

```sh
cd meetily
nix-shell -I nixpkgs=/nix/store/8vdpwi2yls679v4dnjcp3wynsq77jwhl-source benchmarks/qwen3-asr/shell.nix
export RUSTFLAGS="-C target-cpu=znver3"
export CARGO_TARGET_DIR=/home/user/Code/meeting-assistant-research/runtime/qwen3-asr/worker-target
cargo test --release --locked --offline --manifest-path benchmarks/qwen3-asr/worker/Cargo.toml
cargo build --release --locked --offline --manifest-path benchmarks/qwen3-asr/worker/Cargo.toml
python3 -m unittest discover -s benchmarks/qwen3-asr -p test_worker_client.py -v
python3 benchmarks/qwen3-asr/test_worker_fixture.py ../runtime/qwen3-asr --vad-only
python3 benchmarks/qwen3-asr/test_worker_fixture.py ../runtime/qwen3-asr
python3 benchmarks/qwen3-asr/evaluate_worker.py ../runtime/qwen3-asr
node benchmarks/qwen3-asr/metrics.mjs ../runtime/qwen3-asr/artifacts/manifest.json ../runtime/qwen3-asr/worker-fixtures/gated-accuracy.jsonl ../runtime/qwen3-asr/worker-fixtures/gated-accuracy.report.json
```

Shell supplies a declared local ONNX Runtime via `ORT_DYLIB_PATH`/`ORT_LIB_LOCATION`, skips ORT binary downloads, and uses **LP64** OpenBLAS. Worker rejects ILP64 / mismatched BLAS threads at startup. This binary is host-tuned, not portable to arbitrary CPUs.

## Observations / failures (do not hide these)

- Pure silence before AND after speech, four deterministic white-noise levels, 50/440Hz tones: **all eight negative cases had empty output/zero ASR calls** in the non-primed gain trial. No ASR model was loaded for initial noise-only requests.
- Without VAD gain, -40dB scaled public Japanese speech lost its beginning. With bounded VAD-only gain, this **one** scaled clip's full sentence returned; original ASR PCM was unchanged. Not a general quiet-speech/noisy-room result.
- In the initial per-endpoint trial, a natural leading sound in the public clip triggered Silero and decoded `はい。` / padded variant `は。` despite the reference having neither. Whether that leading sound is genuine unreferenced speech is not verified without audio-content confirmation; reference mismatch alone is not proof of hallucination. A sub-1s review flag prevents silent promotion; raw output remains. **VAD does not guarantee no hallucination.**
- EOF truncation at sample 128001 (not divisible by 480) kept the exact real endpoint. Text of a cut word is not guaranteed to recover.
- A 500ms sentence crop was not detected. It is an unaligned short-speech proxy, NOT a validated `はい` fixture. Genuine short acknowledgements/quiet voices still need curated prerecorded/aligned fixtures; unit tests only prove no blacklist.
- Tried priming fresh VAD with 300ms silence: it made white noise amplitude .002 (after bounded gain) produce a false speech interval. Negative assertion failed; **priming was reverted**. Failed trial/probe are retained outside Git, not counted as success.
- Independent decoding of every VAD endpoint regressed the complete corpus: JA CER 3.62%, EN WER 5.71%, TH CER **14.27%** vs ungated 3.05%/5.14%/6.49%. This failed segmentation strategy was archived under `exploratory-fragmented/`, not accepted. Now one bounded window spans all qualified intervals to avoid cutting natural pauses; the complete corpus was rerun with this strategy: JA CER **2.86%**, EN WER **6.29%**, TH CER **10.48%**. Pure silence returned empty text / zero ASR calls. Thai remains worse than ungated 6.49%, and English is worse than 5.14%. All raw review-required output is included. **Quality acceptance has NOT passed; do not integrate into the application.** More conservative context boundaries or presence-only gating require a separate controlled comparison.
- First worker build failed because ORT's transitive build dependencies required OpenSSL. Added OpenSSL to the isolated shell; locked offline build then succeeded. No host OS changes/ORT downloads.
- Rust gate/protocol tests: 11 passed; Python supervisor tests: 4 passed (timeout+restart, ID mismatch, oversized response, busy rejection). These do not establish model/meeting quality. Final Rust formatting, Python compilation and five metric regressions passed. `cargo clippy` was attempted but is unavailable in this shell (`no such command: clippy`), not a completed lint check.

Full fixed-corpus gated precision and current observations are published under `docs/meeting-assistant/evidence/qwen3-asr/worker/` after the sequential run. Score **all raw text**, including review-required segments: don't improve CER by dropping failures. No live/Windows/two-hour/adoption claim.

Further research: `docs/meeting-assistant/QWEN3-ASR-HALLUCINATION-RESEARCH.md` identifies a **v4 ONNX / 480-sample vs official >=512 input contract mismatch inside the reused dependency**. A separate forward-only 512 diagnostic improves short/quiet neural detections but also raises sensitivity to the clip's leading sound; current worker policy is unchanged. Thai raw-CER regression contains substantial number-spelling changes, not proven speech loss. Correct VAD compatibility and compare same-worker/full-PCM presence gating before treating trimming or model choice as the sole cause.
