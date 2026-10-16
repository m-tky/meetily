# Independent 1.7B decoder-INT8 worker — 512-sample fix

Linux-only bounded local-file prototype; **NOT integrated with Meetily**. User approved repair after the hallucination investigation. No capture/playback/cloud inference/credentials/DB/app-lock changes. Model remains 1.7B **decoder INT8 / encoder-prefill F32**. Windows/live drafts/two-hour meetings remain unverified.

## Latest scope override: opt-in app adapter

The user subsequently approved completing the Linux MVP, superseding the historical integration hold below **only for Linux opt-in integration**. The app now has a local Qwen adapter, settings and start/stop hooks; existing ASR/defaults remain unchanged. Adapter tests and generated-silence smoke pass, not speech/live/quality acceptance. See `docs/meeting-assistant/QWEN3-ASR-SETUP.md` and `LINUX-MVP.md`. No automatic correction, default replacement, actual capture or cloud tests. Historical failed robustness results remain unchanged.

## Latest Windows scope override: opt-in app and installer

The user subsequently approved Windows x64/MSVC app integration before packaging.
The adapter gate and lifecycle are now enabled on that target; public Japanese
WAV recognition/repeat/cleanup passed through the app adapter. ASR defaults and
known quality limitations are unchanged. See `docs/meeting-assistant/WINDOWS-AUDIT.md`
for current evidence, and `WINDOWS-INSTALLER.md` for the unsigned CPU NSIS build,
separate model preparation and Windows-tag Pre-release workflow. Do not treat
compilation, adapter smoke or packaging as real-meeting/clean-host acceptance.

The extra `qwen-model-init` executable is opt-in (`--features model-preparation
--bin qwen-model-init`), preserving the existing default Linux/Nix bin set. The
Windows installer build explicitly enables it; no models are bundled.

## Windows x64/MSVC worker portability baseline (historical)

The engine now lives at `vendor/qwen-asr`, pinned with license/provenance and
Windows-only OS patches; no external `../runtime` engine is needed. Linux
Nix packaging uses the same source. This supersedes Linux-only **compilation**
above, not Windows application integration or recognition acceptance.

Windows debug build, worker tests (17), engine portability tests (6 in both
scalar and OpenBLAS modes), and
ready/ping/generated-float32-silence/shutdown pass. ASR weights were not loaded;
the app's Linux-only Qwen gate deliberately remains until real inference and
lifecycle tests pass. See `docs/meeting-assistant/WINDOWS-AUDIT.md`.

In a VS x64 developer PowerShell, use an NTFS target directory and process-local
settings (never point this at the deployed Linux worker):

```powershell
$env:QWEN_OPENBLAS_LIB_DIR = 'C:\\path\\OpenBLAS-0.3.30-x64\\lib'
$env:PATH = 'C:\\path\\OpenBLAS-0.3.30-x64\\bin;' + $env:PATH
$env:OPENBLAS_NUM_THREADS = '6'
$env:OMP_NUM_THREADS = '6'
$env:ORT_DYLIB_PATH = 'C:\\path\\onnxruntime-1.22.x\\onnxruntime.dll'
$env:CARGO_TARGET_DIR = "$env:LOCALAPPDATA\\MeetilyQwenTarget"
cargo test --locked --manifest-path vendor/qwen-asr/Cargo.toml --no-default-features --test meetily_portability
cargo test --locked --manifest-path benchmarks/qwen3-asr/worker/Cargo.toml
cargo build --locked --manifest-path benchmarks/qwen3-asr/worker/Cargo.toml
```

Use the **LP64** official archive `OpenBLAS-0.3.30-x64.zip`, not `x64-64` (ILP64).
Its SHA256 is `8b04387766efc05c627e26d24797ec0d4ed4c105ec14fa7400aa84a02db22b66`.
Retain the OpenBLAS license; stage `libopenblas.dll` alongside the worker for
standalone testing. ORT remains explicit and must match the supported API.
Release optimization, ASR quality, installer/DLL redistribution and clean-host
application acceptance remain separate work.

## Optional ForcedAligner experiment (not app adoption)

The latest user authorized four isolated public/synthetic alignment tests, with an official implementation control on degradation. The experimental worker accepts `--aligner-dir DIR`, optional `transcribe` field `align:true`, and `{"op":"align","id":"a-1","wav":"/absolute/public.wav","text":"提供文","language":"Japanese"}`. Default transcription remains alignment-free. Current local WAV contract accepts mono16k PCM16 or float32, bounded to30s. Alignment is Japanese-only/max512 Unicode scalars, lazy model/fresh session, preserves raw text/offsets, and always `confirmed:false`/`recognition_verified:false`. Silence transcription skips alignment; supplied-text alignment is explicitly unverified. Diagnostic crop-context with inline alignment rejects.

Build into `../runtime/qwen3-asr/align-worker-target`, **never replace the app-used worker-target binary**. Native Japanese units are characters, not official nagisa words. The official CPU-only control/align subprocess is separate, not a general worker replacement. Character and official paths both have remaining attribution failures; aligning wrong words can return plausible timestamps. Read `docs/meeting-assistant/QWEN3-FORCED-ALIGNMENT.md` for results, model pin/license, commands, resource/overlap limitations and retained failures. ASR/diarization remain conceptually parallel; inline postprocessing must not become the mandatory app ASR gate. No app integration/preview update/release.

## Fix

- Same pinned `emotechlab/silero-rs@26a6460` and embedded **official Silero v4 ONNX** (SHA256 `a35ebf52fd3ce5f1469b2a36158dba761bc47b973ea3382b3186ca15b1f5af28`). Uses **`forward()` with exactly 512 samples / 32ms**. Never calls `process()`, which internally re-chunks into incompatible 480 samples. No external dependency/cache modification or new model download.
- `src/vad.rs` ports v4 trigger/temp-end hysteresis rules to original **sample coordinates**, with worker settings: .50 positive / .35 negative, >90ms minimum speech, 500ms silence hold, 300ms pre / 200ms post padding. Overlapping asymmetric pads merge. `licenses/Silero-MIT.txt` preserves upstream notice. This is a local port with explicit worker settings, not all defaults of official Python.
- New neural h/c state per request. Padding is only for a final VAD frame; intervals clamp to actual endpoint. Padded zeros cannot by themselves authorize a too-short real input. VAD-only peak gain targets .1, capped 20x; no energy-only decision. ASR still receives unchanged original PCM.
- **Presence-only gating is default**: no qualified speech → empty/zero ASR calls/no final flush. Speech → one complete **original bounded input** (at most 30s), including natural internal pauses and start/end context. No tiny-island independent decoding. This repairs crop-induced output changes but does NOT remove noise inside a speech-bearing window.
- `--crop-context` is a diagnostic comparison only: same new VAD/session/model/threads but ASR sees first-to-last qualified VAD interval. Not the default. Ready/result metadata expose `vad_frame_samples:512` and `recognition_context:full_input|vad_envelope`.
- Weights load lazily once on first speech and remain resident. Fresh KV/encoder/prompt per recognition, forced Japanese/English/Thai, no past-text conditioning, no silence compaction. No empty/NaN/overlong input or speech carry from a previous request.
- Raw output is never word-blacklisted. Empty/repetitive/oversized/near-token-limit/brief gate evidence → review reasons. Short real `はい`/`うん` aren't deleted. No invented confidence. All recognition is **`confirmed:false`**. `window_final` means a pass ended, not approved evidence. No live draft in this file prototype.

## Local JSONL protocol / bounds

Version 1 ready handshake; stdout only JSONL. Requests <=4096 bytes, newline terminated, unique ASCII IDs 1..64 chars. Unknown fields/operations reject. Oversize/truncated framing terminates process. 1024 IDs maximum then supervisor must restart. Trusted local paths only, NOT a remote sandbox. WAV: nonempty mono16kHz PCM16, <=30s; overlong files reject, never silently trim.

```json
{"op":"transcribe","id":"u-001","wav":"/absolute/public.wav","language":"Japanese"}
{"op":"ping","id":"health-001"}
{"op":"shutdown","id":"stop-001"}
```

Statuses `no_speech`, `recognized`, `review_required`, `error`. Result segments identify actual input sample spans (not word alignments). Gate intervals and recognition spans are distinct. Brief review counts gate evidence, not padded/full input duration.

`worker_client.py`: single-flight/busy rejection (no unbounded queue), ID match, 128KiB responses, 18GiB child address-space cap, deadline. Timeout/protocol failure/cancel kills and reaps only its owned child; next request reloads a fresh worker. No automatic utterance retry. Whole-worker cancellation, not cooperative decoder cancel. Restart incurs load. The upstream offline loop caps total generated tokens at 2048 but exposes text-token count, not EOS; >=1900 text tokens is only a conservative review flag. Supervisor deadline is hard compute bound.

## Reproduce

Existing upstream app Cargo/pnpm locks remain unchanged. Standalone worker Cargo.lock stays frozen. Weights/audio/builds in `../runtime/qwen3-asr`, not Git. The shell declares local ORT, skips binary downloads, uses LP64 OpenBLAS. Worker rejects ILP64/thread mismatch before model load. Host-tuned binary is not portable to arbitrary CPUs.

```sh
cd meetily
nix-shell -I nixpkgs=/nix/store/8vdpwi2yls679v4dnjcp3wynsq77jwhl-source benchmarks/qwen3-asr/shell.nix
export RUSTFLAGS="-C target-cpu=znver3"
export CARGO_TARGET_DIR=/home/user/Code/meeting-assistant-research/runtime/qwen3-asr/worker-target
cargo test --release --locked --offline --manifest-path benchmarks/qwen3-asr/worker/Cargo.toml
cargo build --release --locked --offline --manifest-path benchmarks/qwen3-asr/worker/Cargo.toml
cargo build --release --locked --offline --example vad_endpoint_probe --manifest-path benchmarks/qwen3-asr/worker/Cargo.toml
python3 benchmarks/qwen3-asr/test_vad_reference.py "$CARGO_TARGET_DIR/release/examples/vad_endpoint_probe"
python3 -m unittest discover -s benchmarks/qwen3-asr -p test_worker_client.py -v
python3 benchmarks/qwen3-asr/test_worker_fixture.py ../runtime/qwen3-asr --vad-only
python3 benchmarks/qwen3-asr/test_worker_fixture.py ../runtime/qwen3-asr
# Sequential, never simultaneous model workers:
python3 benchmarks/qwen3-asr/evaluate_worker.py ../runtime/qwen3-asr --tag fixed-full
python3 benchmarks/qwen3-asr/evaluate_worker.py ../runtime/qwen3-asr --tag fixed-crop --crop-context
node benchmarks/qwen3-asr/metrics.mjs ../runtime/qwen3-asr/artifacts/manifest.json ../runtime/qwen3-asr/worker-fixtures/fixed-full-accuracy.jsonl ../runtime/qwen3-asr/worker-fixtures/fixed-full-accuracy.report.json
```

## Verification and boundaries

- Rust gate/framing tests 15 passed; Python supervisor4 and metric5 passed. Independent Python reference comparison **306** seeded/edge probability sequences passed (v4 trigger/temp-end logic with documented worker padding/minimum settings). No speech-quality claim follows from synthetic endpoint parity.
- Full fixed corpus: 10 Japanese / 10 English / 10 Thai + silence. Default full-input **all 30 speech transcripts exactly match the corrected 6-thread ungated baseline**: JA CER **3.05%**, EN WER **5.14%**, TH CER **6.49%**. Silence now empty / zero calls vs baseline `はい。`. No metric normalization weakened, no flagged raw text discarded. One matching pass is not universal determinism.
- Same corrected VAD with diagnostic cropping: **2.48% / 5.71% / 10.18%**. Thus fixing geometry alone doesn't fix context-dependent recognition/number rendering. Default full input preserves reference baseline behavior.
- Eight negative fixture cases (silence before/after speech, four white-noise levels,50/440Hz tones): no ASR calls/empty text. Initial negative requests don't load ASR weights. Final extra silence also checked.
- Public Japanese at original/-20/-40dB and added silence: original sentence retained. The formerly missed 500ms unaligned crop now reaches ASR and is retained as `review_required` (`この場合。`). **Its exact words / genuine acknowledgement quality aren't established**. Never describe a detection success as perfect short-speech ASR.
- Incomplete-frame EOF at original sample128001 retained exactly; cut syllables cannot be reconstructed reliably. Long-silence-tail stress is included in the extended fixture runner.
- Current limited regression passes do not establish real environmental noise/short acknowledgements/conversation/speakers/Windows/realtime/two-hour reliability. Presence gating can still miss weak speech or authorize non-speech; long contextual silence costs compute. No LLM rewriting or automatic approval.

## Follow-up robustness evaluation: FAILED

The same binary passed the limited fixed corpus but fails broader probes. Of18 synthetic negative requests,2 low-amplitude burst cases emit `はい。` with ASR1 and no review flag (`confirmed:false` remains). English speech at -40dB/white SNR0 is missed. One CC-BY-SA human-recorded isolated `いいえ` is detected in6/6 conditions but matches the published transcription in only3/6; surrounding zero padding changes it to `家。`. Selected failures reproduce with two additional fresh workers. No policy/model changes were made to mask these failures. Do NOT integrate with the app or advance to live acceptance yet.

See `docs/meeting-assistant/QWEN3-ASR-ROBUSTNESS.md` and `evidence/qwen3-asr/worker/robustness/` for raw outputs, attribution, scope and commands. Probes use four FLEURS clips and one isolated word, not spontaneous conversation. Synthetic bursts/tones are not real keyboard noise/music. Natural はい/うん, real environmental noise and meetings remain unverified.

## Same-v4 diagnostic follow-up: no policy adopted

`docs/meeting-assistant/QWEN3-ASR-DIAGNOSTICS.md` and `evidence/qwen3-asr/worker/diagnostics/` record24 threshold/gain/min-duration settings across89 windows, none with zero false-positive and zero missed-speech windows. ASR-only known-speech diagnosis can read the two missed English conditions (contraction differs), but is NOT a worker fallback. Short-word14 inputs × full/crop both match only7/14; some errors lack review reasons. One-off100x PCM restoration also fails the short word. Existing worker/model/default/bounds remain unchanged. User subsequently approved official-current VAD comparison ONLY. See `docs/meeting-assistant/QWEN3-ASR-CURRENT-VAD.md`: pinned current ONNX improves the old failed gate cases, but introduces a tone false-positive at unchanged settings. Exploratory peak20/.7/>90ms has zero errors on the same89 windows, NOT held-out/meeting acceptance. New weights live outside Git and are used only by a separate example; embedded v4 and worker defaults remain unchanged. Follow-up frozen-candidate validation FAILS:254additional windows,6quiet misses/0FP; old v4 control1miss/3FP. New2speaker lexical gate12/12 but unchanged v4/full-input ASR matches11/12 (本+white SNR0→ほう。), all reviewed/unconfirmed. No retuning/adoption. See `docs/meeting-assistant/QWEN3-ASR-VAD-VALIDATION.md`. Subsequently approved steps1/2 verify unmodified official Python/Torch vs Rust (same runtime: all296,667frames bit-identical; historical Nix gates identical), and compare fixed VAD gains/extra PCM16 quantization. Float32-retained -40dB+gain100 detects33/33 vs29/33 after PCM16 rounding; five profiles across575windows remain imperfect. See `docs/meeting-assistant/QWEN3-ASR-VAD-INPUT.md`. That phase performed no ASR/step3/correction/cloud/app adoption. Subsequent approved same1.7B INT8 diagnostic compares float32-retained vs extra PCM16 inputs using separate `float_asr_probe` (NOT a new worker contract). Quiet English raw WER10.86% vs41.14%;14/33 pairs improve,3 worsen. Fresh-process182/182 plus exact legacy noise4/4 reproduce; baseline30texts unchanged, negatives65ASR0, all unconfirmed. Padding/noise いいえ→家。/いい。 and legacy 本→ほう。 remain. Gated scores are projections, not integrated-worker measurements. See `docs/meeting-assistant/QWEN3-ASR-ASR-INPUT.md`. No worker/model/default/correction/app replacement; natural short acknowledgements/real noise/live still need verification.

## Failed/history evidence retained

Initial480 VAD + per-endpoint recognition regressed Thai to14.27%; envelope to10.48%; direct silence yielded words. Silence-prime trial caused white-noise false detection and was reverted. -40dB speech lost its start without VAD gain. Original first build lacked OpenSSL; isolated shell fixed it. Prior `cargo clippy` attempt was unavailable (not a completed lint). These logs/reports remain, not overwritten or counted as current success. Source research: `docs/meeting-assistant/QWEN3-ASR-HALLUCINATION-RESEARCH.md`; current fix results: `docs/meeting-assistant/QWEN3-ASR-WORKER-FIX.md` and `evidence/qwen3-asr/worker/fixed/`.
