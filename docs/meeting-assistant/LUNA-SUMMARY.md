# Shared Luna configuration for minutes and post-meeting summaries

## User decision

After seeing the separate legacy Ollama summary settings, the user requested proper Luna setup and then said to continue. The selected ChatGPT/Luna setting now drives both live minutes and explicit post-meeting Generate/Regenerate Summary. This supersedes the previous SETTINGS-UI.md note that post-meeting summaries remain independently configured when ChatGPT is selected.

## Routing and safety

- `resolveSummaryModel` overlays the existing summary configuration with the shared `minutes_settings` provider/model. Does not copy legacy API keys/endpoints into the ChatGPT configuration. Local-provider behavior remains unchanged.
- Under ChatGPT, Settings shows one **AI model** section plus summary language, not the old Ollama model/auto-summary block. Saved-meeting AI Model buttons open the same Settings location.
- Cloud post-meeting summary generation requires the user's Generate/Regenerate action. Both the recording-navigation auto-setup and page auto-generation guards block automatic cloud summaries. A saved provider/model is not upload consent. Existing explicitly enabled live-minutes Stop-final behavior is unchanged.
- Native `api_process_transcript` verifies the connected, plan-enabled account and selected model catalog **before** initializing a summary job. Binds the app's protected SIWC directory/account once for the entire job, including chunks, synthesis and language passes. No API key DB lookup for this provider.
- `LLMProvider::ChatGptPlan` routes through existing official SIWC Responses inference (`store=false`, stream completion validation, no unsupported parameters), not chat completions, Codex credentials or private APIs. Cannot construct this provider from an unbound provider string. Missing auth/model errors; no fallback.
- SIWC directory-based inference shares the same implementation as live minutes. Checks the bound account/consent before sending and again after completion; active-account changes discard the result. Summary cancellation wraps/drops the inference future and does not invoke local/API-key fallback.
- Existing post-meeting chunking, templates, language handling, revision storage and cancellation pipeline retained. No DB migration, new dependency, lock update, ASR/audio/model/language setting change, or release.

## Validation

- Native **78 PASS**: SIWC14, summary client10 (2 new), processor30, service24. New tests prove absent-account/no-API-key/no-local-endpoint fallback, and cancellation before auth-store/network access. Existing SSE/body/account tests retained.
- Frontend **36 PASS**: shared-summary-model4 + prior settings8/device6/document3/runner7/source8. Checks shared model selection, model/credential isolation, local behavior and no automatic cloud-upload permission. TS and diff checks pass; isolated frontend production export + locked/offline native debug build succeed.
- Initial checks failed on one non-exhaustive Rust logging match and the missing TS provider-map key; corrected. Initial logs retained. No real Cloud inference was used by these checks. Full positive post-meeting plan inference on an actual account is still untested here; prior user-reported Luna synthetic success is not relabeled as that test.

## Reflected trial candidate

User explicitly approved restarting to reflect this change. Confirmed idle, stopped only owned4454 helper, resumed same private `dual-preview-4p61rc4k` with `--preserve-settings --no-ready-screenshot`, title Meetily. Old/normal apps and previous candidates kept.

- GC root `../runtime/nixos-luna-summary-candidate` → `/nix/store/cvh8dnm02rdyqg07dxfcw4lg0qgmd50x-meetily-local-candidate`.
- Native SHA256 `f5bd592432cf7db81b9542f9ebfb69930e8f1865cf7ce1d058f1b7c14e318889`, input = stage. Build directory `../runtime/luna-summary-build-IKGT8i`; worker unchanged, original dev executable hash-restored; original frontend `.next`/`out` untouched.
- Metadata/DOM-only checks confirm existing sign-in, Luna selection, preferences surviving full reload, no legacy Ollama block, no main-screen auth/model selectors. Settings Summary left open.
- No Generate/Regenerate, inference/test/login, recording or playback executed; no real audio/body/native recording-log read/export or screenshot.
- Logs: `../logs/meetily-luna-summary-{types,native-check,native-tests,frontend-tests,candidate-build,preview,ui-verification}.log`; private verification `luna-summary-verification.json`.

Manual next step: open a saved meeting and explicitly click **Generate Summary** to send its transcript to the selected ChatGPT plan model. Quality and output should be reviewed by the user. The agent has not executed this step.
