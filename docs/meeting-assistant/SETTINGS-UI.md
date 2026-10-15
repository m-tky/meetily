# Settings UI cleanup

> Follow-up: the user's next request now also unifies post-meeting summaries under the selected ChatGPT/Luna configuration. The independent legacy model panel is hidden in ChatGPT mode. See [LUNA-SUMMARY.md](LUNA-SUMMARY.md) for implementation, consent gates, validation and the newest candidate. The separate post-meeting configuration description below is historical for that mode.

## User decision

Move ChatGPT authentication/model selection into Meetily Settings; use English UI and remove supplementary prose. This changes UI and saved preferences, not authentication protocol, ASR language, capture, or model inference behavior.

The user reported successful Luna sign-in/synthetic testing after the previous setup. That report is distinct from agent-run inference: no real transcript or audio was inspected or submitted.

## Implementation

- **Settings → Summary → Meeting minutes** now contains provider, ChatGPT account/sign-in/model/test/usage controls, or Ollama model name. Existing post-meeting summary configuration remains separate and is labeled accordingly; its provider/backend contract is unchanged.
- Main recording panel contains selected provider/model, a Settings link, and explicit start/stop/final-update buttons. No inline authentication or model selectors.
- Shared `MinutesSettingsContext` uses the existing app `preferences.json` store, key `minutes_settings`. Stores provider and model names only. Native protected SIWC credentials/account selection stay untouched. No schema/API/lock/dependency changes.
- Serialized preference writes preserve concurrent patches, roll back the in-memory key on save failure, and retain preferences after navigation/reload. Update consent is **never** persisted or automatically started. Defaults stay Ollama with an empty model; only this user's already-selected ChatGPT/Luna preference was explicitly applied in the trial UI.
- Recording panel checks native account connection/plan consent and the selected model's actual catalog availability before enabling updates. Missing account/model blocks; no model/provider fallback.
- Onboarding links to the same Settings screen. The layout permits Settings before onboarding completion without exposing the recording screen or marking onboarding complete.
- English labels/errors/status in input controls, minutes document/editor/history, runner, login controls, and modal. Source scan finds no Japanese literals in frontend TS/TSX. This does not translate meeting content or change Japanese transcription/output language.
- Removed recurring Qwen warnings and static explanatory paragraphs in input/recording/general/summary/minutes views. Retained actionable errors, edit-conflict handling, protection/provenance badges, and one short privacy label: `ChatGPT plan · Text sent to OpenAI`.

## Verification

32 frontend tests PASS: settings8, device6, document3, runner7, source8. New tests cover whitelist/no credentials or automatic consent, serialized writes/reload, save-failure rollback/retry, actual React settings selection persistence, main-panel restoration without auth controls/auto-start, disconnected/no-consent/unavailable-model blocking. TypeScript and diff checks PASS.

A source-only isolated frontend copy was exported; Tauri debug/custom-protocol/no-bundle/locked/offline build and Nix stage succeeded. Original frontend `.next`/`out` and original dev executable preserved; executable restore SHA verified. Prior candidates retained; no release/portable installer.

- New candidate: `../runtime/nixos-settings-ui-candidate` → `/nix/store/w4490m37x7k87b1nr06inbzdbsqv5rx5-meetily-local-candidate`.
- Native SHA256: `dd9903567beaa7d36109b05ae6f1587ec523347741eff3143cda87eb6b7ab661` (input = stage); worker byte-equal old candidate.
- Build directory: `../runtime/settings-ui-build-QKL53E`; build script supports `CANDIDATE_LABEL=settings-ui` without replacing the previous source candidate root.
- User explicitly approved restarting the trial app. Checked idle, shut down only owned4454 helper, resumed **same** private `dual-preview-4p61rc4k` with `--preserve-settings --no-ready-screenshot`, title `Meetily`. No provider/language/device reseeding. Normal/old4453 app not stopped.
- DOM/metadata-only verification: Settings Summary opened, existing sign-in and Luna catalog selection preserved, preference survives full React reload, main screen has no auth/model selectors, returned to Settings Summary. No login/test/inference/start/stop/final-update clicked; no screenshots, real body/audio/native recording-log reads or exports.
- Logs: `../logs/meetily-settings-ui-tests-{r1,final}.log`, `meetily-settings-ui-types.log`, `meetily-settings-ui-candidate-build.log`, `meetily-settings-ui-preview.log`, `meetily-settings-ui-verification.log`. Private verification JSON is in the trial profile; session IDs/account data are not printed.
- Helpers: `../runtime/verify-settings-ui.py`, `stop-saved-trial-preview.py`; `scripts/preview_meeting_inputs.py` adds a guarded resume-only `--preserve-settings`. Python compilation passed.

Actual meeting minutes inference, natural-meeting quality, extended recording and other OSes are not newly validated by this UI change.
