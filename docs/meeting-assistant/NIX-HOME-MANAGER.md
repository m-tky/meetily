# Source-built Nix / Home Manager integration

Current product policy remains independent Mic/PC transcription, one shared ASR worker, no human diarization or ForcedAligner. See SOURCE-TRANSCRIPTION.md and LUNA-SUMMARY.md. No GitHub release/tag is created by this integration.

## Build

```sh
nix build github:m-tky/meetily/feat/live-minutes#meetily
```

The flake locks nixpkgs. Builds use checked-in Cargo/pnpm lockfiles; Git dependencies are resolved at their locked commits. Qwen Rust source is vendored at `vendor/qwen-asr`, based on huanglizhuo/QwenASR `37c1e35be333945dc9b5771a6a33fe4b2064533f` with documented Windows portability patches and upstream provenance/license, rather than a sibling runtime checkout. The worker lockfile includes the Windows mapping dependency; the main app and pnpm lockfiles are unchanged. The worker uses LP64 OpenBLAS and a fixed `znver3` Rust target, matching this Ryzen 5650GE host (not a portability claim for arbitrary CPUs). It builds optimized; the app/helper retain the debug profile used in the trial. There is no public binary release.

The native build embeds a fresh production frontend with a deterministic build ID. Fonts, FFmpeg 7 and ORT come from pinned Nix packages; no build-time font/helper downloads are needed. Fixed-output dependency fetching still requires network/cache access. This is a source-rebuildable package, not a claim of independently demonstrated bit-for-bit reproducibility.

## Public model preparation

```sh
# Explicit download/initialization, about 4.7 GB of official BF16 assets plus local INT8 cache:
nix run github:m-tky/meetily/feat/live-minutes#prepare-qwen-model -- /writable/model/directory
# Validate an existing directory without downloads, inference or changes:
nix run github:m-tky/meetily/feat/live-minutes#prepare-qwen-model -- --check /writable/model/directory
# Reconstruct the cache only from existing verified official files:
nix run github:m-tky/meetily/feat/live-minutes#prepare-qwen-model -- --offline /writable/model/directory
```

`nix/qwen-model.json` pins official Qwen/Qwen3-ASR-1.7B revision `7278e1e70fe206f11671096ffdd38061171dd6e5`, file sizes and SHA256 hashes. Downloads do not look up Hub credentials, do not replace mismatching existing files, and publish only checked files. The small source-built initializer loads official weights to construct/validate the decoder INT8 cache; it accepts no audio and performs no transcription/LLM inference. The runtime Rust engine uses official vocab/merges directly, so the historical Candle benchmark's derived tokenizer/Transformers environment is not required. No aligner weights are prepared.

Weights remain writable user data outside the Nix store; models/auth/recordings are not Git assets. Home Manager activation never downloads models or invokes inference.

## Home Manager

```nix
inputs.meetily = {
  url = "github:m-tky/meetily/feat/live-minutes";
  inputs.nixpkgs.follows = "nixpkgs";
};
# In the chosen host module:
imports = [ inputs.meetily.homeManagerModules.default ];
programs.meetily = {
  enable = true;
  # Optional: existing isolated app profile; use a STRING, never a Nix path literal.
  profileDirectory = "/absolute/existing/profile";
  qwenModelDirectory = "/absolute/prepared/model/directory";
  microphoneSource = "EchoCancel_Mic";
};
```

`m75q` deliberately reuses the same private dual-input trial profile and existing verified model directory under the research runtime. They are persistent user data: do not delete that runtime when cleaning up source/build directories. No profile migration, authentication copying, model/language/gain/VAD reseeding, automatic recording, or cloud upload is performed. A new profile instead follows normal onboarding/Settings.

Activation changes only the local ASR worker/model/ORT paths in `qwen-asr.json`, atomically, keeping other keys. The first existing file is retained as `qwen-asr.pre-home-manager.json`; authentication and transcript/recording files are never opened. Home Manager rollback does not automatically restore this mutable file; restore the retained ASR settings explicitly if returning to an older candidate after removing the package.

The menu entry/`meetily` command launches the installed package using the retained HOME/config/data/cache, the normal desktop session bus, X11 and host Pulse server. ALSA preserves the trial's `EchoCancel_Mic only` device identity. The existing preview lease blocks installation-launch while the owned test helper is active; finish/exit the preview before first installed launch. Repeated normal launches use Tauri's existing single-instance behavior. `--help`/`--version` are wrapper-only diagnostics and do not start the app.

ChatGPT plan login/model selection remains in Settings. Existing Luna selection and credentials are retained; choosing a model is not consent to send real meeting content. Post-meeting cloud summaries still require an explicit Generate/Regenerate action.

## Verification record

- Optimized Nix worker build: 14 library + 4 binary tests passed, including explicit OpenBLAS thread-budget application. Four-core-affinity startup/shutdown with the existing six-thread request also passed without loading ASR models or audio.
- Nix model preparation/settings safety tests: 10 passed (checksums, no-overwrite/offline behavior, atomic settings, original backup, symlink refusal).
- Existing official model files: all pinned hashes verified read-only; cache presence checked, not a cache-integrity proof.
- Fresh public-model-only offline cache initialization: passed in a separate runtime directory; no audio/inference.
- Native app/embedded frontend: build verification and GUI checks recorded in external `logs/meetily-nix-*`; an initial smoke caught missing custom-protocol features/dev URL and was retained before correction.

The new post-meeting Luna inference path still has no agent-run actual-account positive inference test. Installation/build checks do not establish natural-meeting quality, long-session throughput, other-OS support, or release readiness.
