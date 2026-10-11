# Meeting assistant evaluation

Read `CLAUDE.md`, `docs/meeting-assistant/PROGRESS.md`, `BASELINE.md`, and `ADR-002-OFFICIAL-SIWC.md` before continuing (the latter three under docs/meeting-assistant).

- This is the current official Meetily checkout, a2cb62e827da7ef59f65064c97233efb2313878e, researched on `research/linux-baseline`.
- User explicitly approved switching to Meetily. Develop on feat/live-minutes; keep Murmur's checkpoint (`../murmur`, feat/live-minutes, 93b8397/50f8b6d) as MIT reference, not the active base.
- Backend is native Rust/Tauri; the old Python backend is unsupported archive.
- Use the pinned pnpm 10 development shell and frozen upstream lockfiles. Never mix pnpm 12 auto-install with pnpm 10 in the same node_modules.
- Use `MEETILY_DISABLE_ANALYTICS=1` for experiments. Test in isolated XDG directories; do not record actual conversations/microphones without permission.
- User selected app-independent official SIWC OSS plan usage and gpt-6-luna. Do not copy Codex/pi credentials or use backend-api/Codex hardcoded client IDs. Official auth/provider code now exists in siwc.rs with synthetic tests, and the user reported successful real browser login + Luna synthetic Japanese fixture. This is one USER-REPORTED successful request, not proof of general meeting quality or long-duration reliability. See docs/meeting-assistant/ADR-002-OFFICIAL-SIWC.md for primary sources. Never print credential-file contents, ID-token-hint authorization URLs, or browser command lines after login starts.
- Audio stays local; cloud transcript/minutes processing must be explicit. Do not retain the stock onboarding claim that data never leaves the device if enabling cloud mode.
- Canonical minutes must stay independent of destination services. Preserve stable item IDs, evidence references, changes and user edits.
- Both apps have rendered successfully under Linux/Xvfb. That does not verify recording, Japanese meeting quality, diarization, long meetings, recovery, or Windows.
- Read docs/meeting-assistant/ASR-LINUX-EVALUATION.md for audio work: public Japanese 3-clip CPU fixture measured, small has a known silence hallucination and RTF>3. User approved a Linux-only native PulseAudio monitor backend (PipeWire Pulse compatibility), preserving mic/Windows CPAL; it is NOT implemented yet. Test only dedicated synthetic sinks, not real desktop/mic conversations.
- Murmur's qwen3:1.7b opt-in Japanese decision-reversal meaning test is a known failure. Do not weaken it to get a green result.
- Record decisions, command results and failures in-repo; keep real audio, credentials, app databases and bulky artifacts out of Git.
