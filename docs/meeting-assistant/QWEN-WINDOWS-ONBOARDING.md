# Windows Qwen consent-based first run

User decision: add both Welcome and Settings entry points, including Qwen-only first-run completion. This is a source change, **not an update to the published windows-v0.4.1-rc.3 installer**. The user subsequently authorized committing and pushing these source changes to the personal fork's `main`. No new release, VM installation, real recording, login or cloud inference is included.

## Behavior

- Windows x64 installer resources + AVX2/FMA are required for the automatic entry point. Linux/Nix retain manual setup; the Nix app source filter now carries the small compiled-in model manifest.
- The unchecked consent control explains Hugging Face / Apache-2.0, the fixed model location, ~4.7 GB download, ~6.5 GiB free disk budget, and several minutes of local initialization. Japanese/English/Thai must be selected. Opening, mounting or restoring the UI never starts downloads or selects Qwen.
- Native `qwen_setup_*` commands accept consent/job ID, never frontend URLs, executable paths or shell strings. The same pinned official revision and nine file hashes as the existing opt-in PowerShell tool are compiled into the application. Production downloads are HTTPS-only with size caps, SHA256 and atomic no-clobber publication.
- One in-process job and a shared `.meetily-preparation.lock` prevent concurrent preparation (including the manual Windows tool). Existing files are checked before use. Reparse points/linked ancestors, mismatched files and unexpected active weight files are refused, not deleted or overwritten.
- Remaining disk space is checked before downloading. File requests and initialization each have a 30-minute bound. Cancellation interrupts streaming/hash work, kills and reaps the initializer, and drops attempt-owned temporary files. Completed verified files survive retries; there is no byte-range resume and consent is requested again.
- Initialization uses same-volume hardlinks in a temporary directory, preserving weight mtimes without modifying originals. Existing INT8 caches are checked against the pinned payload SHA256 (`96e4ad3f590d30941305c4e7350450247ca9109226b7f2d87661cde73758a3dd`, offset 24, total size 1,724,760,064 bytes); the initializer validates the header and timestamp-derived identity. This payload was previously independently equal on Windows and Linux and was rechecked against the existing public-model cache in this change. An engine/model/cache-format update must explicitly review this pin. Stale/foreign caches fail without replacing the original.
- A kill-on-close Windows Job Object protects the owned initializer on application exit. Abrupt termination can leave `.download-*` / `.meetily-setup-*` temporary files; these are not blindly deleted on restart.
- After initialization, a no-audio worker handshake is checked. The candidate settings are validated again before saving paths/language/provider. UI cancellation or navigation during that candidate check prevents committing selection. Merely polling a ready job does not select it; returning users press **Use prepared Qwen**. Late cancellation revokes a pending ready result. No ASR quality test is implied by the handshake (`model_loaded=false`); the initializer, not the handshake, loads the model/cache.
- Welcome completes onboarding only after Qwen is selected and the native worker is valid. The dedicated completion command does not configure a summary provider, start other downloads, or change Parakeet/summary download flags. Delayed incomplete autosaves cannot undo completed onboarding. Settings-only selection leaves onboarding unchanged.
- Existing normal onboarding/manual preparation and cloud opt-in policy remain. No recording, playback, real microphone/desktop capture, credentials, audio upload or inference is started by setup.

## Verification

All tests use synthetic fixtures, loopback HTTP test servers or existing public-model cache hashes; no model download was performed.

- Linux Rust setup tests: manifest, existing file/hash mismatch, late cancellation/stale job ID, extra weights, cache corruption, streamed size/hash/no-clobber failures, stalled download cancellation, Unicode/space-path initializer cancellation/reaping and staging cleanup.
- Existing Qwen adapter/transport/timeout/restart and local settings regressions pass; onboarding tests cover legacy JSON compatibility, unchanged download flags and stale autosaves. The final `qwen` filter passes 22 tests and `onboarding::tests` passes 3 (one overlaps: 24 unique tests).
- Node library/source tests (including existing distribution/onboarding tests): 39 passed. Isolated Bun UI harness: 4 passed (unchecked consent, explicit start/select, restored-ready explicit action, running cancellation/locking). TypeScript passes.
- Isolated Next production export passes without changing the live `.next` / `out` or restarting any app/server. Nix app/worker/model-preparation derivations evaluate; filtered source contains both the new Rust module and manifest.
- Verbatim Job Object code type-checks against windows-sys 0.61 in an isolated API-check crate on Linux. **This is not a Windows executable/build/runtime test.**

Evidence outside Git: `runtime/windows-audit/qwen-onboarding-{native-final,frontend-final,export-final,winapi-final,cache-pin,nix-r2}.log` (relative to workspace root). Initial Node source-regex formatting mismatch and Nix read-only source-materialization failure were retained, then corrected in test/evaluation commands; no checks were weakened. A first helper invocation also found `python3` absent outside nix-shell and was rerun within the pinned shell. One documentation tool batch had a JavaScript parse error before any operations and was rerun as targeted edits.

## Remaining acceptance

The new Windows UI/native controller still needs an actual Windows installer build and isolated Welcome/Settings setup acceptance, including real HTTPS download/cancel/retry, cache staging/identity, process-exit Job Object behavior and persistence/restart. Previous Windows CLI cold-cache and GUI manual-selection tests do not prove this new integrated flow. The published RC3 remains unchanged. No Windows meeting/audio quality or clean-host acceptance is newly claimed.
