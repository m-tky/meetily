# Public-only worker prototype evidence

No real meetings, microphone/desktop capture, credentials or model/audio weights. FLEURS (Conneau et al., 2022), CC-BY-4.0, fixed parent manifest/provenance; derived fixtures carry byte SHA256. Synthetic white noise/tone generation is deterministic in `test_worker_fixture.py`.

- `vad-only.jsonl`, `asr-gated.jsonl`: final non-primed, bounded-gain gate fixtures. Eight negative cases per suite, all empty / zero ASR calls. Positive diagnostics are **not** blanket pass assertions: a 500ms unaligned crop wasn't detected. Original and attenuated Japanese full sentence/tail observations are inspectable.
- `gated-accuracy.jsonl`, `window-accuracy.report.json`: final bounded-window recognition, complete 10 Japanese + 10 English + 10 Thai + one silence. All raw review-required text counts toward errors. Japanese CER 2.86%; English WER 6.29%; Thai CER 10.48%; silence empty/zero calls.
- `fragmented-accuracy.report.json`: failed alternative recognizing every VAD endpoint independently; Thai CER 14.27%. Includes raw anomalies like an added leading `はい。`. Archived comparison, **not accepted production behavior**.
- `prime-negative-rejected.log`: rejected 300ms VAD silence-prime trial; white noise .002 triggered a speech interval. This was VAD-only (no ASR inference). Priming reverted. Gate test stopped at its first failed negative assertion.

Ungated selected 1.7B baseline: Japanese CER 3.05%; English WER 5.14%; Thai CER 6.49%, see parent evidence/report. Implementations/input boundaries differ: this is not a quantization comparison. Recognition time excludes VAD/load but includes prototype fresh-session setup; these are not formal warm-up latency benchmarks. RSS is unmeasured/null, not zero. No claims about realtime/drafts/Windows/long meetings/determinism.

**Speech-quality acceptance is still blocked**: final Thai/English are worse than ungated. The existing Meetily recorder/ASR stays unchanged. Pure-no-speech suppression works on these fixtures only; natural non-speech, mixed noise, short real acknowledgements and quiet conversational speech remain unverified. Code, protocol, limits, failed trials and commands: `benchmarks/qwen3-asr/worker/README.md`.
