# Windows x64 CPU distribution — Meetily Local

This distribution uses `local.meetily.desktop`, separate from upstream Meetily and the synthetic Windows audit profile. Automatic updater endpoints and updater artifacts are disabled. The NSIS `*-setup.exe` installs for the current user. It is **unsigned**: Windows SmartScreen may warn. Do not describe it as a signed/stable release.

## Published pre-release

[windows-v0.4.1-rc.3](https://github.com/m-tky/meetily/releases/tag/windows-v0.4.1-rc.3) provides `Meetily.Local_0.4.1_x64-setup.exe`, its SHA256 sidecar, build metadata and the successful CI install/uninstall report. Installer SHA256: `cadd2e7fb084a0b16aa963e76a12d92e16ee2ef05b3e59beeb651630654e63e4` (49,212,462 bytes). This is the CI-built release, not the earlier VM candidate. Models remain a separate explicit setup below.

## Local build

Use an NTFS checkout, VS2022 C++/Windows SDK, Rust 1.99.0 MSVC, Node.js 22+, pnpm 10.34.5 and 7-Zip. From the repository root:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File frontend/scripts/build-windows-installer.ps1
```

The script pins/hashes LLVM 18.1.8 (old bindgen compatibility), LP64 OpenBLAS 0.3.30, CPU ONNX Runtime 1.22.0 and the existing FFmpeg 8.0.1 archive. It keeps tool/environment changes process-local, uses locked Cargo/frozen pnpm dependencies, builds the CPU helper and Qwen worker/model initializer, runs Windows adapter tests, then builds the NSIS release installer. `dist/windows/` contains the installer, SHA256 and build metadata. `-LlvmRoot`, `-NativeTargetDirectory`, `-WorkerTargetDirectory` and `-CacheDirectory` support existing developer caches. Native build targets must not be on virtiofs. Native tests reuse the release dependency graph rather than building a second GiB-sized debug graph. `-SkipInstall`/`-SkipTests` are developer-only shortcuts, not the CI defaults.

## Consent-based setup (new source builds; not published RC3)

Windows x64 installer builds now expose **Agree and set up Qwen** on the Welcome screen and Settings → Transcription → Qwen. Select Japanese, English or Thai and check the explicit consent box. No network/model preparation starts merely by opening the screen. The target directory, Hugging Face source, Apache-2.0 license, ~4.7 GB download, ~6.5 GiB free-space budget and CPU initialization cost are shown before consent. AVX2/FMA and bundled runtime/initializer are required.

Setup checks existing files against the same compiled-in pinned manifest as the manual script, downloads only missing files over HTTPS, verifies byte count/SHA256 before atomic no-clobber publication, initializes INT8 in an isolated same-volume hardlink staging directory, and validates the local worker without audio. Existing mismatched files/cache and unexpected active weights are refused, not overwritten. The fixed engine/model cache payload is also SHA256-checked (timestamp identity is checked by the initializer). Disk space is checked against missing weights plus cache/headroom; downloads/initializer have bounded timeouts. **Cancel setup** stops the owned initializer and removes this attempt's incomplete staging/download files; verified complete files remain. Retry requires consent again and re-downloads only interrupted/missing files (no byte-range resume). A Windows kill-on-close Job Object also protects the initializer if the application exits. Forced termination may leave temporary `.download-*` / `.meetily-setup-*` files/directories; these are not automatically deleted on restart.

Successful setup selects Qwen. The Welcome route then completes transcription-only onboarding, without configuring a summary provider, downloading other models or marking them downloaded. Existing normal onboarding and manual paths remain available. Merely restoring/polling an old setup does not auto-select: use **Use prepared Qwen** to finish if you left the screen. Settings selection does not itself complete onboarding. No microphone, recording, cloud inference or ChatGPT sign-in is started. Linux keeps its existing manual/Nix configuration.

## Explicit Qwen model setup

Model weights are **not bundled**. Qwen requires AVX2/FMA, sufficient RAM, and roughly 6 GiB for the model plus derived INT8 cache (allow extra free space). A small 8 GiB VM has run the isolated worker, but this is not a recommendation or proof that a full meeting workload fits.

The installed application includes `qwen/` beside the application executable, containing the worker, initializer, their local DLLs, model manifest and preparation script. From that directory:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\prepare-qwen-model.ps1 -Download
```

This explicitly downloads only the public Qwen/Qwen3-ASR-1.7B revision `7278e1e70fe206f11671096ffdd38061171dd6e5`, verifies every official file SHA256, and initializes its local decoder INT8 cache. It never reads/uploads audio or credentials. Without `-Download`, missing files fail offline. Existing mismatched files are not overwritten. Preparation is exclusive; a concurrent invocation fails. Do not copy an untrusted sidecar cache.

For the **published RC3 installer**, the original first-run onboarding remains: finish its normal initial setup before selecting Qwen. RC3 has no Qwen-only initial setup. New source builds add the consent-based first-run route described above; running the manual script alone never changes onboarding.

Default model directory: `%LOCALAPPDATA%\local.meetily.desktop\models\Qwen3-ASR-1.7B`. Optional `-ModelDirectory C:\...` uses another directory; enter that absolute path in Settings → Transcription → Qwen. Bundled worker/ORT fields are prefilled for a fresh Windows profile. Select Japanese, English or Thai and explicitly press **Validate and select Qwen**. No model/provider is switched automatically. Missing model/dependency/unsupported CPU errors must not silently fall back to another ASR.

A cache copied between filesystems/OSes may be invalidated by shard timestamp precision and re-quantized. The audit's metadata-rebound public fixture was a test preparation, **not** an automatic installed-model import feature.

## Runtime and legal notices

The worker and its OpenBLAS/ORT DLLs are colocated under `qwen/`; VC143 CRT/OpenMP redistributable DLLs are app-local as well. No developer PATH should be required. Templates and the existing FFmpeg/helper are also packaged. App settings, credentials, recordings, model weights and build caches must not be release assets. WebView2 is required; its Microsoft bootstrapper may need Internet on machines without WebView2.

Meetily and the pinned Qwen engine are MIT; OpenBLAS is BSD-3-Clause; ONNX Runtime is MIT; public Qwen model metadata declares Apache-2.0. FFmpeg's existing essentials build is **GPLv3**, not LGPL. Its original GPL license, full build configuration/source reference, application license and Qwen/OpenBLAS/ORT licenses are retained in the installer. Public distributors remain responsible for corresponding-source/third-party license obligations; preserve these notices and the exact FFmpeg source/build information, not just the MIT application license. This is not a legal compliance certification.

## Release automation

`.github/workflows/windows-prerelease.yml` is defined for your own repository/fork, **not** the current upstream origin. Push `windows-v0.4.1` (or `windows-v0.4.1-rc.1`), matching the base application version, to your own remote after reviewing/committing the complete sources (including `vendor/qwen-asr/` and the model manifest). The workflow validates the tag, builds on Windows 2022, runs native tests and isolated install/worker/helper/FFmpeg/uninstall checks, then publishes a **Pre-release**, never latest/stable. Assets include the `.exe`, `.exe.sha256`, `windows-build.json` and `installer-smoke.json`. Existing stable/draft releases are not overwritten. Publication on `Zackriya-Solutions/meeting-minutes` is explicitly blocked.

Manual `workflow_dispatch` runs build downloadable Actions artifacts only. Action revisions and Rust/pnpm/tool archives are pinned; no signing/updater secrets are needed. Defining the workflow does not push a commit/tag or publish a release. RC3's actual CI build/install/uninstall/publish passed; later source changes require a fresh run and do not update that published installer.

## Acceptance status

Windows VM release NSIS generation and isolated installation have passed, including installed-worker silence, app-local DLL paths, helper ping/shutdown and FFmpeg with the developer PATH removed. Repository `docs/meeting-assistant/WINDOWS-AUDIT.md` records the precise candidate/final artifacts, additional app/model tests, failures and limits. Packaging and small public-audio tests do not prove loopback capture, Stop/save/recovery, clean-host dependency closure or long-duration meeting quality. No real microphone/meeting/cloud test is implied by packaging. RC3 passed its actual GitHub Actions release gates. The new consent-based setup flow has only synthetic/native Linux and frontend/API checks so far; its Windows installer/GUI/download acceptance remains pending. See [QWEN-WINDOWS-ONBOARDING.md](QWEN-WINDOWS-ONBOARDING.md).
