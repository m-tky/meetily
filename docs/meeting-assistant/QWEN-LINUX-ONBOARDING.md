# Linux / Windows Qwen setup parity

User request: Linux should use the same automatic Qwen setup as Windows; Qwen3.5 4B and other local summary weights are not required.

## Changes

- Welcome and Settings use the existing shared Qwen setup UI on packaged Linux x86_64 as well as Windows x64. An unchecked consent control precedes downloads. Opening the app, mounting the UI or polling a ready job does not download or select a provider.
- The native download / SHA256 verification / disk-budget / exclusive-lock / cancellation / isolated INT8 initialization / worker-validation implementation is shared. The pinned model remains **Qwen3-ASR 1.7B decoder-INT8**, not Qwen3.5 (a separate summary model).
- Qwen-only onboarding now supports Linux. After explicit setup and successful validation/selection, it completes onboarding without downloading Parakeet or a local summary model, changing summary configuration, or fabricating their download flags. Existing optional summary-model catalogs and existing user settings/files remain intact.
- Linux resolves packaged executables beside the native app (`libexec/qwen-speech-worker`, `libexec/qwen-model-init`) and the packaged `libonnxruntime.so`. Models go into the writable Tauri app-local-data `models/Qwen3-ASR-1.7B` directory, never the Nix store. Saved custom Qwen settings still take precedence for normal use; automatic setup uses its fixed install destination, as on Windows.
- Nix app packaging now includes the initializer and ONNX library link, plus a packaged CA certificate path for HTTPS. The ONNX link is published in `postFixup`, after GUI wrapping, and an actual `ctypes.CDLL` load is required by the build. The existing manual preparation command and the app reuse one initializer derivation. AVX2/FMA and all packaged runtime files are required before advertising automatic setup. Unbundled old preview binaries do not gain this feature automatically.
- Linux uses the extensionless initializer name; Windows retains `.exe` and its Job Object guard. Cancellation kills/reaps the owned initializer on both platforms. Linux abrupt app/process termination during initialization is not newly verified and must not be equated with Windows Job Object teardown coverage.

## Verification

No model download, audio capture/playback, ASR/cloud inference, authentication, user-profile changes, visible app replacement, release creation or publication was performed by the tests.

- Native `qwen` filter: **23 PASS**; onboarding: **3 PASS** (one overlaps, 25 unique). Includes runtime-presence-without-model-files, model manifest/checksum/no-clobber/cancellation/cache/linked-path tests and existing adapter regressions. Linux initializer cancellation test now uses the actual Linux filename and confirms process reaping / staging cleanup.
- Node setup / Windows distribution / onboarding-summary tests: **23 PASS**, including Linux explicit setup selection, optional-summary behavior and packaging source guards.
- Isolated Bun setup UI harness: **5 PASS** (unchecked consent, explicit setup/select, Linux path selection through the same consent UI, restored-ready action, running cancellation).
- TypeScript `tsc --noEmit`: PASS.
- Nix app / manual preparation / initializer derivation evaluation: PASS. Extracted initializer builds successfully at `/nix/store/8hhqszxdbdzi6rj0dzacrkkqcrai0mzl-qwen-model-init-0.1.0`.
- Initial full Nix debug app build/export succeeded, but the resulting package **FAILS actual ONNX dlopen**: wrapGAppsHook treated the executable-mode `.so` symlink in `libexec` as a program and replaced it with a launcher. Metadata-only fresh Welcome/reload UI initially passed; that alone did not establish runtime readiness. Preserved initial package `/nix/store/5fxm0dq1ajbg4irjx8xpng90ij50457i-meetily-local-0.4.1-local` is NOT the corrected candidate. Moved the library link to postFixup and added a mandatory dlopen check. Corrected full debug build/export **PASS** at `/nix/store/pr7b0kc0pf8b88hga8gjycvk6ji5z2h6-meetily-local-0.4.1-local` (exit 0), including the real shared-library load.

- Corrected packaged native GUI in a fresh HOME/XDG, private D-Bus/Xvfb, null ALSA and unavailable Pulse socket: **PASS**. Welcome exposes automatic setup, consent is unchecked and the setup button disabled, bundled native runtime resolves and ONNX actually dlopens, browser reload keeps the same behavior, setup status remains absent, no Qwen model directory/provider settings are created, recording remains false. No screenshots, consent/start clicks or model requests. Helper closes only its owned processes. Reproducer: `frontend/tests/gui/qwen_setup_native_smoke.py` with the packaged binary and WebKitWebDriver.
- `git diff --check` and Python helper compilation: PASS.

Logs outside Git: `../logs/meetily-linux-qwen-{native.log,frontend.log,ui.log,nix-eval.json,nix-eval.log,initializer-build.log,package-build.log,package-build-r2.log,native-gui.log,native-gui-r2.log,ort-initial-failure.log}`. The initial dlopen failure is retained. One initial multi-edit was rejected for a non-unique match before changing any file, then corrected; not a product-test failure.

Actual HTTPS download/cancel/retry and selected-provider persistence/native restart after model preparation with the newly packaged Linux app remain unverified. The new native GUI test checks unconsented first-run availability and browser reload only. The existing visible/installed app was not replaced or restarted. Existing Windows acceptance limits remain in [QWEN-WINDOWS-ONBOARDING.md](QWEN-WINDOWS-ONBOARDING.md). No claim of meeting/ASR quality or portable general-Linux delivery is added.
