# Qwen-only checkpoint: independent local retest

Subsequent current-source Windows VM execution: [QWEN-ONLY-WINDOWS-RETEST.md](QWEN-ONLY-WINDOWS-RETEST.md). The Windows-not-executed statements below describe this earlier Linux retest checkpoint, not the later VM results.

Tested source: `3187df6`, branch `feat/live-minutes`, checkout under `/home/user/Code/meeting-assistant-research/meetily`. This is a fresh execution of tests, not reuse of the previous handoff's logs. No release, tag, workflow dispatch, visible application replacement, real microphone/desktop capture, physical playback, user-content access, login or cloud inference.

## Executed successfully

- Node: `node --test frontend/tests/lib/*.test.mjs`: **46 PASS**.
- Native, pinned nixpkgs development shell, `cargo test --locked --offline -p meetily --lib FILTER -- --test-threads=1`: **59 PASS, 1 existing ignore** across `audio::transcription` (23), settings (2), retranscription (17), import (14 + 1 ignore), onboarding (3). Some inherited import fixture tests can return early without their archived sample; the GUI below supplies actual decoding/VAD/storage coverage.
- `pnpm exec tsc --noEmit`: PASS. Isolated Next export: PASS, using a source-only copy with existing node_modules and packaged Source Sans font substitution; original frontend outputs untouched.
- Current-source worker: **19 PASS** (15 library + 4 binary), locked/offline debug build, LP64 OpenBLAS, 6 threads, `znver3`. Current-source startup catalog script: **6 PASS**. These startup probes load no weights and perform no inference.
- Current-source embedded custom-protocol **debug** app and fixture bootstrap: build PASS. Existing sidecars were used; no existing app executable was overwritten (the retest build explicitly required its destination not to exist).
- Fresh isolated batch GUI: **PASS**, both model selections, other-size import override, selected-size retranscription, persisted JSON/SQLite equality, failed/cancelled inference preserving old text, busy-operation refusal, native PID exit/restart. ASR here is deliberately a fixture.
- Separate fresh-profile **live GUI for each size**: private Pulse null sink, disabled hardware ALSA, eSpeak artificial audio, real capture/VAD, fixture ASR, manager-removed Stop-tail, save/event/raw/SQLite equality, worker/temp cleanup and native restart: **PASS**. No screenshots.
- **Actual 0.6B and 1.7B live GUI, separate fresh profiles** with current-source worker and copied real model: private virtual capture of artificial eSpeak, actual inference pending when Stop removes manager, tail preservation, audio/transcript persistence, cleanup and native restart/reopen: **PASS**. This is recognition/storage execution evidence, not Japanese accuracy acceptance.

## Actual model inference, not a mock

Existing official 0.6B and 1.7B files were checked against every size/SHA256 in the current pinned manifests, then reflink-copied into new retest-only directories. Original weights/cache were not changed; cache SHA256 stayed unchanged on the copies. No model download or cache regeneration was needed.

Each model processed 5s silence, two unique public Japanese FLEURS clips, then a repeat of the first clip. Silence returned `no_speech`, speech returned `recognized` with `model_loaded:true`, repeat text was identical, and explicit shutdown exited 0. Public WAV hashes match the existing fixture manifest (CC-BY-4.0, Conneau et al., FLEURS 2022).

Scoring uses the repository's existing NFKC/lowercase/Unicode punctuation-and-whitespace removal and edit distance, **without text correction or numeric reading conversion**. Only two unique clips / 74 reference characters are scored; the repeat is not double-counted.

| Model | Character errors | CER | Interpretation |
|---|---:|---:|---|
| 1.7B | 0 / 74 | 0% | These two fixed public clips only; not natural-meeting acceptance |
| 0.6B | 9 / 74 | 12.16% | Actual Japanese substitutions/deletions, not an integration crash |

Example small-model output changes `価値があります` to `私があります` and `帰国後` to `企画後`. Both sizes execute, but **small is not accuracy-equivalent**. Offering both models does not fix earlier short-word/noise failures. These debug builds are not throughput benchmarks or proof of real-time capability.

## Windows: implemented paths versus acceptance

Static inspection of this same commit confirms:

- Shared Qwen-only catalog/UI, live adapter, batch import/retranscription and model-specific settings are not Linux-only. Windows support is x64/MSVC; auto setup additionally checks AVX2/FMA and bundled runtime presence.
- `frontend/scripts/build-windows-installer.ps1` builds worker + initializer and bundles LP64 OpenBLAS, ORT, CRT/OpenMP and **both** manifests. `tauri.windows-release.conf.json` includes `binaries/qwen`. Dedicated `windows-prerelease.yml` invokes this script.
- Windows uses WASAPI/CPAL capture, not Linux's native Pulse timed route. Historical VM/CI successes in WINDOWS-AUDIT.md are prior checkpoints, not this Qwen-only version's successful Windows run.

Remaining concrete concerns (source findings, not new Windows runtime reproductions):

1. The legacy generic `.github/workflows/build-windows.yml` does not build/stage the Qwen worker/initializer or use the dedicated Qwen resource overlay. Default `tauri.conf.json` lacks the Qwen resource mapping. Do not distribute that workflow's output as an equivalent Qwen-ready installer.
2. Dedicated Windows packaging runs provider/setup/onboarding native filters, but does not gate the newly shared batch/retranscription/settings-migration filters or both-size GUI workflows. Its early runtime script explicitly probes only 1.7B without weights. This leaves the new 0.6B/batch paths under-verified on Windows, even though they are implemented.
3. `audio/devices/platform/windows.rs` still uses partial-name matching and a default-device fallback when the selected device is not found. UI preflight may catch ordinary missing selections, but this resolver behavior warrants a device-loss/race test; it is not equivalent to a demonstrated fail-closed endpoint contract.

The `win11` VM was found **shut off** and was not started or modified in this retest. No Windows compiler/runtime execution or current-version Windows installer acceptance is claimed. Full Windows parity needs a current-commit Windows build plus isolated both-size setup/import/retranscription/recording/save/restart and device-loss tests. Release publication is not part of that acceptance.

## Evidence and retained attempts

Git-excluded evidence root: `../logs/qwen-only-retest/` (workspace-relative). Key logs: `native-transcription-r2.log`, `native-and-ts.log`, `node.log`, `worker-r2.log`, `gui-r2.log`, `real-inference-r2.log`, `real-inference.json`, `quality.json`, `real-gui-small.log`, `real-gui-large.log`. Runner scripts and test-only model copies/profiles stay outside Git.

Initial attempts are retained: first native build exceeded the tool's 120s bound (rerun passed); the broad benchmark shell waited for unnecessary tool downloads and was stopped in favor of a minimal same-pin LP64/ORT shell; first GUI runner used the wrong packaged font subdirectory; first inference runner omitted protocol `op:transcribe` and correctly received a protocol failure (fixed only the runner). No production behavior or assertion was weakened to pass.

No full new-model HTTPS download, clean-host/release installation, long-file/multi-hour/hard-crash, real dual-input conversation, cloud/auth or Windows runtime acceptance. Passing fixture/structural tests must not be reported as speech-quality acceptance.
