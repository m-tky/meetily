# Meeting assistant evaluation

Read `CLAUDE.md` and `docs/meeting-assistant/BASELINE.md` before continuing.

- This is the current official Meetily checkout, a2cb62e827da7ef59f65064c97233efb2313878e, researched on `research/linux-baseline`.
- Base selection is reopened. Wait for the user to approve switching before large feature migration; keep Murmur's checkpoint (`../murmur`, feat/live-minutes, 93b8397).
- Backend is native Rust/Tauri; the old Python backend is unsupported archive.
- Use the pinned pnpm 10 development shell and frozen upstream lockfiles. Never mix pnpm 12 auto-install with pnpm 10 in the same node_modules.
- Use `MEETILY_DISABLE_ANALYTICS=1` for experiments. Test in isolated XDG directories; do not record actual conversations/microphones without permission.
- User selected app-independent official SIWC OSS plan usage and gpt-6-luna. Do not copy Codex/pi credentials or use backend-api/Codex hardcoded client IDs. Official auth is still UNIMPLEMENTED; see `../murmur/docs/meeting-assistant/ADR-002-OFFICIAL-SIWC.md` for primary sources and implementation checklist.
- Audio stays local; cloud transcript/minutes processing must be explicit. Do not retain the stock onboarding claim that data never leaves the device if enabling cloud mode.
- Canonical minutes must stay independent of destination services. Preserve stable item IDs, evidence references, changes and user edits.
- Both apps have rendered successfully under Linux/Xvfb. That does not verify recording, Japanese quality, diarization, long meetings, recovery, or Windows.
- Murmur's qwen3:1.7b opt-in Japanese decision-reversal meaning test is a known failure. Do not weaken it to get a green result.
- Record decisions, command results and failures in-repo; keep real audio, credentials, app databases and bulky artifacts out of Git.
