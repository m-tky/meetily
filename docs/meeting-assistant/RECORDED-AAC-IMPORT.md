# Import of actually saved Windows recordings

## Scope and correction

User clarified the request was **importing recorded audio**, not another recording test, then approved correcting the import destination before verification. Inputs are the two `audio.mp4` AAC files actually captured/saved in the preceding Windows virtual-only test, not the original source WAV and not a fake worker. No new recording, playback, microphone capture, cloud inference, model download, normal profile/application replacement, Calibre deletion, installer or release operation.

A separate destination bug remained in `audio/import.rs`: despite the live saver repair, import still directly called `get_default_recordings_folder()`. Import now snapshots `load_recording_preferences(&app).save_folder` before worker initialization, then creates the meeting only in that selected absolute destination. Invalid relative/blocked destinations fail rather than using Music/Documents. No existing imported/recorded meeting is migrated. Audio decoding/VAD/ASR/model configuration and copied audio bytes were not changed.

Added two native destination/collision regressions, a caller/source guard, and `audio::import` to the dedicated Windows packaging native gates. Runtime application baseline `fd91a5d`; four-file patch SHA256 `8d33ca6f7c3e9d1285dde38a474b9751bf62a1b4dec6274fc3cf3d5d0fbf1285`. Tested candidate SHA256 `985ba233808df239d45694b681aa4921ca44c1a1635d9ba4afa14aa38777b4a7`. Source hashes were compared with the host working tree after execution; no lock/dependency change.

## Actual import results

Inputs: recording A = 10.752s decoded audio; recording B = 10.624s. Both contain the same licensed public Japanese FLEURS sentence from separate virtual capture trials. A/B label files, not a claim that ASR model size generates audio.

Windows WebView2 DOM-driven drag/drop-event import dialog, explicit Japanese + model override, real current-source Qwen worker/weights, actual decoder/VAD, native import/JSON/SQLite and native PID restart were used. OS file-chooser/physical pointer interaction is not claimed.

| Saved input | Import model | Import/save/reopen | Recognized segments |
|---|---|---|---:|
| A (`audio.mp4`, AAC) | 1.7B | PASS | 1 |
| A (`audio.mp4`, AAC) | 0.6B | PASS | 1 |
| B (`audio.mp4`, AAC) | 1.7B | PASS | 1 |
| B (`audio.mp4`, AAC) | 0.6B | PASS | 1 |

All four imports:

- Create independent new meeting IDs/folders in the explicitly configured private destination, even though every input basename is `audio.mp4`.
- Copy the compressed audio **byte-for-byte** (SHA256 matches the recorded input), leaving both original audio/metadata/transcript hashes unchanged.
- Persist nonempty fresh recognition with raw JSON/SQLite text and audio times equal. Imported mixed-file source remains null/unknown, never an invented Mic/PC/person attribution or copied live-system tag.
- Keep the preferred global model at 1.7B despite per-import Small overrides; job-owned workers and temporary float input are cleaned.
- Reopen all four imported transcripts after native PID **7512 → 14296**; JSON/SQLite content/hashes are unchanged. Original private recording preferences are restored. Test app and loopback debug listener are closed.
- Keep recording=false and audio/video elements paused throughout. No recording or playback was used for this test.

Native import reports 10.773333s / 10.645333s, respectively: approximately **21.33ms longer** than the preceding FFmpeg decode measurements. This small observed decoder-duration difference is retained, not hidden; its cause/gapless timing behavior was not independently diagnosed. Exact PCM/timestamp parity is not claimed.

## Quality is not accepted

**4/4 workflow passes do not mean error-free speech recognition.** On recording A, even 1.7B produces a wrong phrase (`価値` in the reference becomes `ワカシ` in its output). Small also has substitutions such as `そそられる` → `誘られる` and `価値` → `私`/`わかち`. On recording B, 1.7B matches this short reference sentence apart from punctuation; this is not general meeting-quality acceptance.

Raw outputs remain untouched. This test does not establish whether the differences originate in captured audio, decoding/resampling, VAD context or model behavior, and no gain/VAD/context/model/correction change was adopted to obtain passing results. The supported conclusion is **recorded AAC import, persistence and reopen work for these files**, with recognition-quality limitations remaining.

## Regression and evidence

- Linux native import: **16 PASS, existing fixture 1 ignore**. Includes selected Unicode/space/apostrophe folder, same-title isolation and blocked/relative destination rejection. Older optional sample tests can return early; actual AAC decode/recognition coverage comes from the Windows GUI above.
- Windows native: **97 unique PASS, existing import fixture 1 ignore**, including save/checkpoint, transport/setup, import/retranscription/settings/onboarding and 14 synthetic SIWC tests. Existing tests were not weakened.
- Node: **48 PASS on each OS**; updated packaging PowerShell parser: **0 syntax errors**. No installer was built or published.
- Original cache top-level main/worker/DLL files restored/hash-verified; model-copy cache hashes unchanged. Normal apps/profiles and user recordings are not replaced.

The first import completed but its harness failed parsing a Japanese JSON result piped through PowerShell's legacy console encoding. That failure is retained under `attempt-1-output-encoding/`; the actual app's JSON/SQLite/Japanese display were not corrupted. The harness now writes audit JSON directly as UTF-8 and reads that file; it did not change recognition text or assertions. The complete four-way matrix was rerun successfully. The first trial remains a private test meeting; it was not deleted to hide the failed attempt.

Git-excluded evidence: `../runtime/windows-qwen-only-retest/evidence/recorded-aac-import/final-summary.json`, and guest originals in `Z:\meetily-windows-transfer.HnU5H1\qwen-only-retest-9d37569\recorded-aac-import`. Key files: `build-result.json`, `gui-result.json`, `audit-before-restart.json`, `audit-after-restart.json`, `post-proof.json`, source manifest and retained attempt. Linux logs: `../logs/recorded-aac-import/`. Scripts, binaries, audio, models, databases and raw logs stay outside Git.

Only these short mono AAC-in-MP4 recordings were verified. Long files, other codecs/devices, natural meetings, clean-host/new-installer acceptance and general recognition quality remain unverified. This is a tested isolated Windows candidate, not an update of the normal installed application.
