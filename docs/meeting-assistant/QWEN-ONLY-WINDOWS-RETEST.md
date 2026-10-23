# Qwen-only: current-source Windows VM execution

Source: `9d37569` (application code is the Qwen-only checkpoint `3187df6`). The user requested virt-manager Windows testing, authorized VM login, and explicitly approved reuse of the existing release build cache after the isolated debug build's disk safety gate stopped. No installer/tag/release publication, workflow dispatch or normal application/profile replacement.

## Environment and isolation

- Existing libvirt/virt-manager `win11`, Windows 11 x64, 8 vCPU/8GiB. VM started and login completed with user authorization; no login secret was written to a file or test log. First virt-manager launches lacked a usable display; the owned Wayland console launch succeeded. Hypervisor keyboard input and the pre-existing virtiofs `Z:` share were used, not a new remote shell/firewall service.
- Source ZIP is a tracked-file subset of the above commit, SHA256 `1063f55381874dccced8c796a9f30d2ff0ba4d9772039d1fd242d6e711bb5984`. No audio/model/database/credential/private log in that archive. Existing pnpm lock equality was checked before read-only dependency reuse; no package installation or lock change.
- New source/candidate/GUI directories: `%LOCALAPPDATA%\MeetilyWindowsDev\qwen-only-retest-9d37569`. Unique app identifier `local.meetily.windows-qwenonly-retest`; fresh, marked roaming/local profiles and separate WebView2 data. Recording preferences were preseeded to a private test folder and verified by native IPC before onboarding completion, avoiding normal recording/recovery folders. No screenshots of meeting content.
- WebView2 automation port 9347 was asserted loopback-only; native IPC/UI/storage were real. DOM events are automation, not physical pointer acceptance. No recording, microphone/system capture, playback, cloud inference or real login to ChatGPT occurred in these tests.
- The old cache's **top-level main/worker executables and DLLs** were hash-verified into backups, then restored and rechecked after the new candidate was copied aside. Cargo deps/examples/fingerprints are build cache and were updated, not claimed unchanged. Installed applications and ordinary profiles were not replaced. The VM remains running; test app/owned workers and private debug listener were closed.
- User additionally permitted Calibre-library deletion, but it was **not necessary and was not done**. No existing user-data folder was deleted.

## Freshly executed Windows results

- **Node 46 PASS**, TypeScript PASS, isolated frontend export PASS.
- Current-source **worker 19 PASS** (15 library + 4 binary), worker/initializer release build PASS with existing LP64 OpenBLAS/ORT/CRT runtime.
- **57 unique native tests PASS**, existing import fixture **1 ignore**. Settings migration 2, transcription 20 (+ one Windows transport test initially ignored), retranscription 17, import 14 (+ existing ignore), onboarding 3. The explicit include-ignored provider run passes all 5 provider tests: four overlap with the transcription run, and the additional Windows child transport test covers timeout/crash/cancel/restart/Unicode-temp cleanup. Do not double-count those four.
- **14 synthetic SIWC tests PASS** on this current-source test executable, including Windows protected storage/DPAPI, signed claims, callback/PKCE and stream/error handling. Tests use synthetic credentials/temp directories/loopback fixtures; they are **not** a real account login, cloud request, refresh/endurance or user-credential read.
- Current-source native app, protocol fixture, GUI bootstrap and app-adapter smoke build PASS, custom-protocol frontend and unique test identifier. This is a manually staged test candidate, **not** a newly installed NSIS distribution.
- **6 actual clean-PATH worker startup/refusal conditions PASS**: explicit no-weights 1.7B and 0.6B; small config auto-detection; config/request mismatch; unknown identity; absent config/no identity. These startup probes do not load weights or perform inference.

## Actual models through the Windows GUI

Both existing official models were checked against all pinned manifest file sizes/SHA256 on the host and copied using reflinks into a new test-only shared directory. Tokenizer hashes and original cache identities were verified. Only copied shard mtimes were normalized to Unix epoch for Windows FileTime portability; only the copied cache's 8-byte identity field was rebound. Payload/config/layout stayed identical. Both copied cache SHA256 values were independently rechecked unchanged after all Windows runs. No weights downloaded, original model modified or new cache generation claimed.

With the real current-source worker, actual prepared models and the public Japanese FLEURS 7.98s clip `fleurs-ja_jp-validation-2-1607.wav` (CC-BY-4.0, Conneau et al. 2022):

1. Native Welcome discovers both installed models, checks both bundled setup-tool paths, completes Qwen-only onboarding and keeps recording inactive.
2. **1.7B:** actual model selection / rendered import dialog / Japanese selection / Import / saved meeting / rendered Retranscription dialog / completion PASS.
3. **0.6B:** Settings → Transcription → Small → Use installed model / actual import and retranscription via Small PASS. These are not fixture workers.
4. Native model configuration stays selected after each job. Actual read-only SQLite versus `transcripts.json` text/times/null source equality passes for both meetings.
5. Old native PID 12116 exits; new PID 9444 reopens the same private profile. Small remains selected, recording stays false, both meetings' raw hashes/SQLite contents are unchanged. Owned worker cleanup and debug listener closure PASS.
6. Separately, **each size** passes the actual Rust app-adapter smoke: float32 silence for ja/en/th, real public speech twice with identical repeat text, explicit worker shutdown and Unicode/apostrophe/space temporary-audio cleanup. No playback/capture is used by this file-based smoke.

This establishes current Windows execution of both model choices, onboarding selection, shared adapter, file import, retranscription, persistence and restart. It does **not** establish small/large accuracy equivalence or natural meeting quality. Small retains recognition errors; the preceding Linux two-clip quality comparison remains a limited fixture result, not Windows-wide accuracy acceptance.

## Retained attempts and remaining work

- Initial isolated debug-target attempt stopped at the free-space guard without a build or deletion. User then approved release-cache reuse. The first source archive omitted `.github` required by the Node source guards: that attempt failed with ENOENT, original binaries were restored, the archive was corrected and the complete run repeated successfully. Test assertions/production code were not weakened. Existing compile warnings remain in the logs.
- **Not yet tested in this current-version VM run:** Windows microphone/PC recording → Stop-tail → AAC/SQLite save/reopen, simultaneous inputs/device removal, full new-model HTTPS download/init/selection, Small cold initialization on Windows, newly built NSIS install/uninstall, clean host, multi-hour/hard crash, actual SIWC account login/cloud inference.
- The previous generic Windows workflow's missing Qwen staging, dedicated CI batch/Small coverage gaps and Windows device resolver's partial-name/default fallback are still source findings, not fixed by this test. Use the dedicated Windows distribution route, not the legacy generic workflow, for future packaging verification.

Evidence (Git-excluded): `../runtime/windows-qwen-only-retest/evidence/`, mirrored from `Z:\meetily-windows-transfer.HnU5H1\qwen-only-retest-9d37569`. Final gates: `build-result.json`, `verify-result.json`, `gui-result.json`, `security-result.json`, `final-summary.json`; SQLite comparisons `db-before-restart.json` / `db-after-restart.json`; native/worker/frontend logs and original failed attempts are retained. Host model-copy provenance is `public-model-fixtures/fixture-manifest.json` in that test share. Source/test scripts and bulky binaries/models/private profiles stay outside Git.
