# Recording destination and Windows file-sync repair

Follow-up: [RECORDED-AAC-IMPORT.md](RECORDED-AAC-IMPORT.md) fixes the separate import destination omission and tests these saved AAC recordings through actual import. Its workflow passes do not establish error-free recognition.

## Decision and scope

The user approved fixing the save-folder failure found in the Windows VM, then separately approved the newly reproduced Windows file-sync failure. Retest was explicitly **virtual audio only**. No database/schema migration, old-recording move, audio-format/ASR/model/language/gain/VAD change, normal application replacement, installer publication or release/tag operation.

Application baseline `9d37569`; exact six-file runtime patch SHA256 `4993a6b26a57724c55620ae3aa526cb8c1de0dd0a31b3d2ecaedde0e68a0676c`. The Windows candidate SHA256 is `1c53897de3f10649234fd4d7d8adb3bf7f4feea27a097308ec3446c5b2933c62`. Host working-tree file hashes were independently compared to the patch manifest after execution. CI-script/source-guard changes were verified separately, without rebuilding/publishing an installer.

## Bugs and minimal corrections

1. **Selected destination was discarded.** Native preferences contained the private `save_folder`, but `RecordingSaver::initialize_meeting_folder` called `get_default_recordings_folder()` unconditionally. Both native start paths now snapshot the selected path with preferences and pass it through the manager to the saver. The path is required, absolute, and used for checkpoints, metadata and the retained transcript sink. An invalid destination fails before capture/pipeline startup, resets manager state, and never silently switches to Music/Documents. Existing default preferences still supply the normal default folder when the user has not chosen a custom folder.
2. **Windows could not sync read-only handles.** The existing checkpoint failure/repair regression failed with OS error 5 after the folder-only change. An independent Windows `FlushFileBuffers` fixture reproduced read-only=5 versus writable=0 with identical file bytes. The float checkpoint writer now flushes and syncs its original writable file handle before publication; the caller no longer reopens it read-only. Final AAC publication syncs the existing writable `NamedTempFile` handle. Sync was **not removed**, and checkpoint bytes/AAC settings/atomic publication and retained-checkpoint policy are unchanged.

Changed runtime files: `audio/{recording_commands,recording_manager,recording_saver,checkpoint_audio,incremental_saver}.rs`; new `recording_save_folder_tests.rs`. No dependency or lock changes. New path regressions cover nested Unicode/space/apostrophe destinations, both auto-save modes, blocked/relative paths with no fallback or accumulation, manager state/no streams on failure, and old Stop-tail sinks remaining bound across changed destinations/sessions.

The dedicated Windows installer script now explicitly makes its verified FFmpeg available to native tests and gates `save_folder_tests`, `audio::recording_saver`, `audio::checkpoint_audio`, and `audio::incremental_saver`. A Node source guard protects these gates. No script was executed to create an installer in this task.

## Executed regression results

- **Linux: 24 unique native save/checkpoint tests PASS.** Commands: `cargo test --locked --offline -p meetily --lib FILTER -- --test-threads=1`, filters `save_folder_tests`, `recording_saver`, `checkpoint_audio`, `incremental_saver`. Counts are 6 + 12 + 2 + 9 with five duplicate tests, not 29 unique tests.
- **Windows: 95 unique native tests PASS, existing import fixture 1 ignore.** Includes the above save regressions, prior model/setup/transport/settings/import/retranscription/onboarding tests and 14 synthetic SIWC/DPAPI tests. The previous checkpoint regression now passes unchanged. The Windows transport fixture is explicitly included; overlapping filters are deduplicated.
- **Node 47 PASS on Linux and Windows**, including the new packaging gate guard. Updated PowerShell packaging script parser: **0 syntax errors**. Earlier TypeScript/export results remain applicable: frontend application code/dependencies were not changed.
- Windows locked/offline release-cache native build PASS (user approved cache reuse). Original cache top-level main/worker/DLL artifacts were restored and independently hash-checked after staging the new candidate; Cargo deps/examples remain updated build cache. Existing installed applications/profiles were not replaced. Source patch and CI guard archive checksums were verified before guest application.

## Actual Windows virtual recording, both real models

This is **not fixture ASR**. The same manifest-verified official model copies and actual worker used in the preceding Windows test were retained. Both copied model-cache hashes remained unchanged after all runs; no download, original-model mutation or cold-cache generation was needed. Source: the licensed public FLEURS Japanese 7.98s clip, CC-BY-4.0, Conneau et al. (2022), with one second of zero padding at each end for virtual playback.

Isolation: explicit PWEC virtual playback endpoint, explicit emulated ICH9 Line In (not a host microphone), no physical passthrough, no other active guest render sessions. Current UI has no microphone-off option; it was **not** treated as if `None` disables a microphone. The owned virt-manager client was disconnected before capture, so **all SPICE channels, including host playback and microphone forwarding, were absent**. A host heartbeat watchdog and guest playback guard reject reconnection/staleness/unrelated audio. Public PCM was rendered to the explicit PWEC WinMM device, not the default/physical speaker. No screenshot/audio/transcript from a real conversation was read.

For each model, actual DOM input/model selection and reload persistence → Start → real WASAPI capture / VAD / real Qwen → Stop with inference pending → manager removal → tail drain → save in the configured private folder → saved-meeting view → new native PID restart/reopen passed:

| Model | Decoded saved duration | Real tail after manager removal | Native PIDs | Result |
|---|---:|---:|---|---|
| 0.6B | 10.752s | 1 segment | 6212 → 4012 | PASS |
| 1.7B | 10.624s | 1 segment | 11196 → 13368 | PASS |

- Native Stop reports `save_succeeded=true`, `transcription_drained=true`; no native warning/error/device-fallback events.
- Nonempty actual raw text, native events and SQLite match; recognized speech has **system/PC tags only**, not inferred speakers. The emulated Line In is silent in this fixture.
- Saved AAC decodes with **zero FFmpeg warnings/errors**, finite/nonzero samples; audio/metadata/transcript hashes remain unchanged across restart and across the second model's session.
- ASR worker exits at Stop, owned temporary input files are removed, owned apps and loopback debug listener are closed. Independent final guest render-session check is empty. Host watchdogs recorded 178/316 isolation checks for Small/Large respectively.
- Decoded sample peaks were about 1.063/1.087 (AAC overshoot); finite/nonzero/error-free decode does **not** prove no clipping, true-peak protection, bit-exact source capture, precise synchronization or long-duration stability. No gain/normalizer change was adopted.
- Small's Japanese errors remain; Large also formats `30` as `三十` on this live fixture. This is workflow execution, **not meeting-accuracy acceptance or automatic correction**.

## Retained failures and final state

The initial pre-playback folder escape, incorrect harness abort `stop_recording` argument/cleanup flag, metadata-only trial relocation telemetry error and verified cleanup remain documented in [QWEN-ONLY-WINDOWS-RETEST.md](QWEN-ONLY-WINDOWS-RETEST.md). Later repair harness initially nested a decoded PowerShell JSON array and failed before tests; retained, flattened correctly, original artifacts verified. The folder-only patch's Windows 11/12 saver failure and independent read-only flush probe are retained. No test was weakened or switched to the default folder to get green.

Git-excluded evidence: `../logs/recording-save-folder-fix/` and `../runtime/windows-qwen-only-retest/evidence/save-folder-fix/`. Final summary: `final-summary.json`; guest originals under `Z:\meetily-windows-transfer.HnU5H1\qwen-only-retest-9d37569\save-folder-fix`. Small recording evidence is `virtual-small-pass/`, Large is the root `recording-result.json`/audit files; `post-recording-proof.json` independently checks both sessions and original artifacts. Test scripts, binaries, models, audio, profiles and raw logs stay outside Git.

VM remains running with the owned console disconnected; test apps/workers/private debug ports are closed. Calibre and existing user recordings were **not deleted**. Normal Linux/Windows apps and sign-ins were not updated. This is a tested local Windows candidate, **not a newly installed NSIS or published release**. Clean-host/installer acceptance, full new-model HTTPS setup, real microphone/physical devices, natural meetings/multi-hour/hard crash, Windows device-loss/fallback safety and real ChatGPT login/inference remain separate/unverified. Legacy generic Windows workflow omission is still unresolved; dedicated distribution route is the intended path.
