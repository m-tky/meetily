# Synthetic native minutes GUI smoke (Linux)

For **user-operated visible native preview** (not an automatic smoke), use
`scripts/preview_synthetic.py` as documented in
`docs/meeting-assistant/USER-PREVIEW.md`. It isolates hardware/profile access,
prepares artificial editable minutes and real local Qwen, and plays synthetic
speech only after the user presses Start. It never starts recording itself.

No capture, ASR/LLM inference, auth or real profile access. A new HOME/XDG profile,
private D-Bus and disabled audio endpoints are created under the supplied runtime.
Buttons use rendered DOM `.click()` (WebKit pointer clicks after split-pane navigation
were unreliable); text uses WebDriver input. IPC, SQLite, native restart and Blob
Downloads are real, not mocks. This does **not** verify physical pointer interaction,
recording/Stop, revision-conflict UI, hard crash/power loss or packaging.

From the repo Nix shell (`nix-shell shell.nix`), using the existing staged sidecars:

```sh
export CARGO_TARGET_DIR=/home/user/Code/meeting-assistant-research/murmur/target
cargo build --locked --offline -p meetily --example gui_fixture_bootstrap
cargo build --locked --offline -p meetily
# Separate terminal, same Nix shell; restart after production builds overwrite .next:
cd frontend
NEXT_TELEMETRY_DISABLED=1 pnpm exec next dev -H 127.0.0.1 -p 3118
```

Then from the repo root in the Nix shell:

```sh
python3 frontend/tests/gui/minutes_native_smoke.py \
  --binary "$CARGO_TARGET_DIR/debug/meetily" \
  --bootstrap "$CARGO_TARGET_DIR/debug/examples/gui_fixture_bootstrap" \
  --driver "$(command -v WebKitWebDriver)" --runtime ../runtime
```

`../runtime` must already exist. Default display `:108` and driver port `4445` must
be free; override `--display` / `--port` if needed. Only owned process groups are
stopped. Profiles/screenshots/downloads/databases stay outside Git. `result.json`
contains a boolean `passed`; partial events alone are not a passing result.

Checks: explicit folder registration → saved meeting view → edit revision 2 with
stable item/evidence IDs and before/after history → native restart → saved edit
view → JSON matching canonical and Markdown containing edited text → unchanged
raw metadata/transcripts.

Observed failures: old Next dev process returned CSS/JS 404 after `.next` was
replaced; restarting it restored hydration. WebKit pointer click did not open the
edit form; DOM-driven interaction is explicitly scoped above. Restart waiting
original text was incorrect after editing, so readiness now uses the stable evidence.

Successful final run: `../logs/meetily-gui-minutes-final.log`.

## Recording / Stop-tail / fault smoke

`recording_native_smoke.py` additionally starts a private Pulse server with ONLY a
null sink + private Unix protocol (no hardware modules), disables ALSA hardware,
and plays eSpeak synthetic speech into that sink. Real app VAD runs; the ASR JSONL
worker is a deterministic fixture, not real weights/inference. No cloud/auth.

```sh
for fault in none worker-crash save-failure; do
  python3 frontend/tests/gui/recording_native_smoke.py \
    --binary "$CARGO_TARGET_DIR/debug/meetily" \
    --bootstrap "$CARGO_TARGET_DIR/debug/examples/gui_fixture_bootstrap" \
    --driver "$(command -v WebKitWebDriver)" --espeak "$(command -v espeak-ng)" \
    --runtime ../runtime --fault "$fault" || break
done
```

Defaults: display `:109`, port `4446`. Start/Stop buttons use DOM `.click()`; setup
uses public native commands. A delayed worker result is released only after Stop
removes the native manager. The test verifies saved audio by decoding, raw transcript,
SQLite reopen after native restart, worker exit/private input cleanup and recording
file/checkpoint hashes. Crash tests require visible ASR warning and surviving Stop;
failed recognition remains incomplete despite later success. Save failure targets
metadata publication, keeps checkpoints, then **the harness manually repairs the
fault target** before restart. No automatic repair or hard-crash guarantee.

Details and recorded failures: `docs/meeting-assistant/RECORDING-GUI.md`.
Latest three-scenario run: `../logs/meetily-gui-recording-tail-final.log`.
Regression hook harness (separate Bun process because it mocks React hooks):
`bun test tests/lib/recording-controls-errors.test.tsx` from frontend.

## Checkpoint timing regression (new WAV transport)

New recordings checkpoint lossless float32 WAV, then AAC-encode once at Stop.
The save-failure GUI requires preserved `audio_chunk_*.wav`. All recording GUI
runs now fail on any stored-audio decode stderr at warning level, not just an
FFmpeg nonzero exit. Legacy MP4 checkpoints are still detected; explicit legacy
recovery returns partial/timing warning and never overwrites existing audio.
See `docs/meeting-assistant/CHECKPOINT-TIMING.md` for native boundary tests,
10-minute recheck, decoder-tail differences and updated NixOS candidate results.

## Bounded real-worker soak / embedded NixOS candidate

Add `--soak-seconds 600` to the explicit real-Qwen command below to repeat full
synthetic bursts with 5-second gaps. The test samples owned native/worker RSS,
saved segments and private temporary input files; it checks public errors, one
persistent worker, full save/drain and restart equality. It does not score accuracy,
prove a leak-free budget or measure the internal queue directly. Monitor unit tests:
`python3 -m unittest discover -s frontend/tests/gui -p 'test_soak_monitor.py'`.

For a custom-protocol candidate use `--clean-native-env --expected-origin tauri://localhost`
with both recording and minutes smokes. These strip app-launch development library
settings while keeping hardware/profile isolation. The Nix candidate wrapper itself
supplies runtime libraries. The test runner still uses the development tools.
See `docs/meeting-assistant/LINUX-STABILITY-PACKAGING.md` for results, retained
FFmpeg decode warnings, and the NixOS-only candidate/synthetic launcher.

## Explicit actual local Qwen inference

The recording script defaults to the fixed fixture. Opt into real local inference
with all three `--real-worker`, `--real-model`, `--real-ort` paths; fault flags cannot
be combined. It never downloads models. Example from the repo Nix shell:

```sh
python3 frontend/tests/gui/recording_native_smoke.py \
  --binary "$CARGO_TARGET_DIR/debug/meetily" \
  --bootstrap "$CARGO_TARGET_DIR/debug/examples/gui_fixture_bootstrap" \
  --driver "$(command -v WebKitWebDriver)" --espeak "$(command -v espeak-ng)" \
  --runtime ../runtime \
  --real-worker ../runtime/qwen3-asr/worker-target/release/qwen-speech-worker \
  --real-model ../runtime/qwen3-asr/artifacts/model-1.7B \
  --real-ort /nix/store/x46rnziynpalr4phm6yh0x46s42xq5kw-onnxruntime-1.27.1/lib/libonnxruntime.so
```

Real mode uses the direct worker (no response proxy/fixture/delay), completes all
synthetic playback before Stop, observes the app-owned float32 mono16k temporary
WAV and owned real worker PID, and requires a real tail after manager removal.
All raw segment texts must match native events/disk/SQLite, including after restart.
Audio is decoded and duration checked; cleanup/process exit and file hashes are
verified. Raw outputs aren't corrected or scored for quality. Full details and
limits: `docs/meeting-assistant/REAL-QWEN-GUI.md`; latest verification log:
`../logs/meetily-gui-real-qwen-verified.log`.
