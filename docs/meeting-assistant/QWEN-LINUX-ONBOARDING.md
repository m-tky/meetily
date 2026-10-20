# Linux / Windows Qwen setup parity

User request: Linux should use the same automatic Qwen setup as Windows; Qwen3.5 4B and other local summary weights are not required.

## Changes

- Welcome and Settings use the existing shared Qwen setup UI on packaged Linux x86_64 as well as Windows x64. An unchecked consent control precedes downloads. Opening the app, mounting the UI or polling a ready job does not download or select a provider.
- The native download / SHA256 verification / disk-budget / exclusive-lock / cancellation / isolated INT8 initialization / worker-validation implementation is shared. The pinned model remains **Qwen3-ASR 1.7B decoder-INT8**, not Qwen3.5 (a separate summary model).
- Qwen-only onboarding now supports Linux. After explicit setup and successful validation/selection, it completes onboarding without downloading Parakeet or a local summary model, changing summary configuration, or fabricating their download flags. Existing optional summary-model catalogs and existing user settings/files remain intact.
- Linux resolves packaged executables beside the native app (`libexec/qwen-speech-worker`, `libexec/qwen-model-init`) and the packaged `libonnxruntime.so`. Models go into the writable Tauri app-local-data `models/Qwen3-ASR-1.7B` directory, never the Nix store. Saved custom Qwen settings still take precedence for normal use; automatic setup uses its fixed install destination, as on Windows.
- Nix app packaging now includes the initializer and ONNX library link, plus a packaged CA certificate path for HTTPS. The ONNX link is published in `postFixup`, after GUI wrapping, and an actual `ctypes.CDLL` load is required by the build. The existing manual preparation command and the app reuse one initializer derivation. AVX2/FMA and all packaged runtime files are required before advertising automatic setup. Unbundled old preview binaries do not gain this feature automatically.
- Linux uses the extensionless initializer name; Windows retains `.exe` and its Job Object guard. Cancellation kills/reaps the owned initializer on both platforms. Linux abrupt app/process termination during initialization is not newly verified and must not be equated with Windows Job Object teardown coverage.

## Follow-up: ASR-only completion and native restart

User clarified that **Qwen3.5 4B is not required: install only Qwen3-ASR**, then authorized proceeding. No application source change was needed: the corrected packaged candidate already has this route. The old visible application was initially unchanged; after the verification below, the user confirmed saved/window closed and approved replacement (see visible-resume section).

Extended `frontend/tests/gui/qwen_setup_native_smoke.py` with explicit `--prepare` and optional `--reuse-public-model`. Defaults remain unconsented/metadata-only. The follow-up used a fresh isolated HOME/XDG, private D-Bus/Xvfb, null ALSA and an unavailable Pulse socket, with the existing corrected `/nix/store/pr7b0kc0pf8b88hga8gjycvk6ji5z2h6-meetily-local-0.4.1-local/bin/meetily`.

**PASS** (`../logs/meetily-linux-qwen-asr-only-retry-r5.log`):
- Actual HTTPS public-model download progress, then **Cancel setup** through the UI; job becomes cancelled, no provider selection/onboarding completion, attempt-owned partial file removed.
- To avoid another ~4.7 GB transfer at ~1.2 MB/s, copied only the nine original manifest files from the existing public-model directory into the isolated destination. No INT8 cache/settings/completion flags were seeded. This is a cache-assisted retry, **not acceptance of a complete fresh HTTPS download**.
- Explicit UI retry checks all manifest hashes, builds a **fresh** 1,724,760,064-byte INT8 cache through the bundled initializer, validates/selects the worker, and completes Qwen-only onboarding with Japanese selected.
- Parakeet and summary download flags remain `not_downloaded`. No Qwen3.5 directory or unexpected large non-ASR model files appear.
- Native process exits (PID liveness checked), then a new native process restores the selected Qwen paths/provider/model and completed onboarding, reaches the recording screen without requesting a summary model, and starts no new setup or recording.
- Node setup/onboarding-summary regressions **13 PASS**, TypeScript and Python compile/diff checks PASS (`../logs/meetily-linux-qwen-asr-only-regression.log`).

Reproducer from the checkout, inside the pinned `nix-shell shell.nix`:

```sh
python3 frontend/tests/gui/qwen_setup_native_smoke.py \
  --binary /nix/store/pr7b0kc0pf8b88hga8gjycvk6ji5z2h6-meetily-local-0.4.1-local/bin/meetily \
  --driver "$(command -v WebKitWebDriver)" --runtime ../runtime \
  --display :121 --port 4471 --prepare \
  --reuse-public-model ../runtime/qwen3-asr/artifacts/model-1.7B
```

Failures/interruptions retained, not product acceptance: first uncached download deliberately interrupted for transfer cost (`...-prepare.log`); an attempted supervisor cancellation via unsupported WebDriver `/sessions` returned 404. Background Python had inherited ignored SIGINT, so its first interruption did not stop it; only processes with that exact isolated runtime environment were then terminated. First retry found its occupied display/port (`...-retry.log`). Retry r2 incorrectly treated the domain status `error` field on normal cancellation as a WebDriver protocol error; the harness now uses a separate invocation-error envelope. Retry r3 passed the completion/restart checks; r4 with an additional recording-screen check hit the 240s command timeout and is not counted as PASS. Final r5 uses the actual recording button's aria-label and a 600s command allowance and passes all checks. The helper now explicitly handles SIGINT/SIGTERM and checks owned service startup. All logs stay outside Git.

No actual mic/desktop capture, playback, speech/ASR or cloud inference, authentication, credential access, normal-profile changes, release build or publication. The initializer loads/quantizes public weights locally; its worker handshake is not speech-quality acceptance. Full uncached HTTPS completion and clean-host/general-Linux packaging remain unverified.

### Approved visible resume

User confirmed **saved/window closed, replacement allowed**. The old 4454 helper was absent, port was free and no process remained in that exact private profile environment. Resumed the same `dual-preview-4p61rc4k` on DISPLAY `:0` / port `4454`, title Meetily, using the corrected package through `../runtime/nixos-asr-only-candidate`, `--preserve-settings --no-ready-screenshot`. No new package/build/release was created; normal app and old 4453 profile/candidates were retained. The existing external ASR model remains selected; setup was not re-run on the real profile.

Metadata/DOM-only verification **PASS** (`../logs/meetily-linux-qwen-asr-only-visible-verification-r3.log`): embedded origin, same profile, existing sign-in and Luna after browser reload, Qwen ASR provider/model and Japanese preserved, new automatic setup available with consent unchecked, job absent/recording false. Left **Settings → Transcription** open. Native executable verified against the new package, owned host capture streams **0** (`...-visible-native-r3.log`). No meeting text/audio/native logs/screenshots or credential contents were read.

Initial verification tried direct document navigation to an exported route, which returned the recording page rather than Settings; changed only the verification to click the real Next link. A subsequent `.click()` did not activate the Radix tab, so the test now dispatches its expected mousedown then click. Both timeout logs retained. Native verification initially matched the package wrapper rather than `.meetily-wrapped`, then assumed a globally unique process despite a timed-out isolated-test leftover. Final verification scopes to the exact visible profile environment and actual executable. Leftovers from isolated `qwen-onboarding-*` test environments were terminated without touching the visible profile (`...-test-cleanup.log`).

### Home Manager installation audit

On the user's follow-up, read-only inspection found `~/.config/home-manager/profiles/desktop.nix` enables `programs.meetily` with the flake package, lock revision `cd9156f7dcbabad2b3a4aa807df521efe88f5350`. The installed `~/.nix-profile/bin/meetily` launcher resolves to package `/nix/store/wm2ph1k73m1v031l6mmn88l97k6vqr6c-meetily-local-0.4.1-local`, which includes the worker, initializer and actual ORT library. The module installs the runtime, **not the model weights during activation**; the application requests consent and downloads/prepares ASR into writable app-local data. `qwenModelDirectory = null` does not disable app setup; non-null is an optional preprepared-path configuration. No Qwen3.5 summary model is required.

Actual installed ORT dlopen PASS using its own glibc 2.44 loader. The initial probe from the older pinned shell's Python/glibc 2.42 failed with missing `GLIBC_2.43`; this was a cross-generation probe mismatch, not proof of an app defect. Reproducer uses `env -u LD_LIBRARY_PATH /nix/store/h4wfwic161kxrr74jlzla5lsm28hgary-glibc-2.44-25/lib/ld-linux-x86-64.so.2 --library-path /nix/store/h4wfwic161kxrr74jlzla5lsm28hgary-glibc-2.44-25/lib /nix/store/d64q19q1xjdwfhqx6czvrjgrhq0n3lcc-python3-3.14.7/bin/python3 -c 'import ctypes; ctypes.CDLL("/nix/store/wm2ph1k73m1v031l6mmn88l97k6vqr6c-meetily-local-0.4.1-local/libexec/libonnxruntime.so")'`.

No Home Manager config/lock/profile changes, switch, installed normal-profile launch or model download performed in this audit. The full installed Home Manager GUI download/setup path was not newly tested; the preceding acceptance used the separate corrected package and isolated profile.

## Earlier verification

No model download, audio capture/playback, ASR/cloud inference, authentication, user-profile changes, visible app replacement, release creation or publication was performed by the earlier tests below.

- Native `qwen` filter: **23 PASS**; onboarding: **3 PASS** (one overlaps, 25 unique). Includes runtime-presence-without-model-files, model manifest/checksum/no-clobber/cancellation/cache/linked-path tests and existing adapter regressions. Linux initializer cancellation test now uses the actual Linux filename and confirms process reaping / staging cleanup.
- Node setup / Windows distribution / onboarding-summary tests: **23 PASS**, including Linux explicit setup selection, optional-summary behavior and packaging source guards.
- Isolated Bun setup UI harness: **5 PASS** (unchecked consent, explicit setup/select, Linux path selection through the same consent UI, restored-ready action, running cancellation).
- TypeScript `tsc --noEmit`: PASS.
- Nix app / manual preparation / initializer derivation evaluation: PASS. Extracted initializer builds successfully at `/nix/store/8hhqszxdbdzi6rj0dzacrkkqcrai0mzl-qwen-model-init-0.1.0`.
- Initial full Nix debug app build/export succeeded, but the resulting package **FAILS actual ONNX dlopen**: wrapGAppsHook treated the executable-mode `.so` symlink in `libexec` as a program and replaced it with a launcher. Metadata-only fresh Welcome/reload UI initially passed; that alone did not establish runtime readiness. Preserved initial package `/nix/store/5fxm0dq1ajbg4irjx8xpng90ij50457i-meetily-local-0.4.1-local` is NOT the corrected candidate. Moved the library link to postFixup and added a mandatory dlopen check. Corrected full debug build/export **PASS** at `/nix/store/pr7b0kc0pf8b88hga8gjycvk6ji5z2h6-meetily-local-0.4.1-local` (exit 0), including the real shared-library load.

- Corrected packaged native GUI in a fresh HOME/XDG, private D-Bus/Xvfb, null ALSA and unavailable Pulse socket: **PASS**. Welcome exposes automatic setup, consent is unchecked and the setup button disabled, bundled native runtime resolves and ONNX actually dlopens, browser reload keeps the same behavior, setup status remains absent, no Qwen model directory/provider settings are created, recording remains false. No screenshots, consent/start clicks or model requests. Helper closes only its owned processes. Reproducer: `frontend/tests/gui/qwen_setup_native_smoke.py` with the packaged binary and WebKitWebDriver.
- `git diff --check` and Python helper compilation: PASS.

Logs outside Git: `../logs/meetily-linux-qwen-{native.log,frontend.log,ui.log,nix-eval.json,nix-eval.log,initializer-build.log,package-build.log,package-build-r2.log,native-gui.log,native-gui-r2.log,ort-initial-failure.log}`. The initial dlopen failure is retained. One initial multi-edit was rejected for a non-unique match before changing any file, then corrected; not a product-test failure.

At the earlier checkpoint, actual HTTPS download/cancel/retry and selected-provider persistence/native restart after preparation were unverified; the follow-up above verifies cancellation, cache-assisted retry/fresh initialization and persistence/native restart. Complete uncached HTTPS download remains unverified. The original default native GUI test still checks only unconsented first-run availability and browser reload. The existing visible/installed app was not replaced or restarted. Existing Windows acceptance limits remain in [QWEN-WINDOWS-ONBOARDING.md](QWEN-WINDOWS-ONBOARDING.md). No claim of meeting/ASR quality or portable general-Linux delivery is added.
