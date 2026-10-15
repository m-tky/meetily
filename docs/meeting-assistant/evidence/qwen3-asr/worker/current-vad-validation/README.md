# Frozen candidate validation FAILS — no retuning/adoption

Methods/commands/limits: [QWEN3-ASR-VAD-VALIDATION.md](../../../../QWEN3-ASR-VAD-VALIDATION.md).

- `frozen-plan.json`: created before inference. Current pinned ONNX, peak20, positive .7, negative .55, strict minimum >90ms; v4/.5 and current/.5 controls. This isn't production adoption.
- `manifest.json`:254inputs =220speech+34negatives. FLEURS other26 clips ×8, two new lexical speakers ×6, new synthetic seeds/frequencies +two silences. Clean FLEURS clips were previously evaluated: NOT fully independent source holdout. Whole-clip RMS SNR, PCM16 hashes/RMS/peaks/saturation recorded before inference. Saturated cases stay included.
- `report.json`: complete gate intervals/IDs and controls. Candidate6quiet misses/0FP; current .5 control5miss/0FP; v4 control1miss/3FP. Failed candidate is not retuned on these data. All six candidate misses are -40dB EN/TH clips detected by v4.
- `short-asr-r1/r2.jsonl`, `short-asr-summary.json`: unchanged existing v4/full-original-input worker, NOT new-VAD integration. New lexical gate12/12; ASR11/12 normalized matches. 本+white SNR0→ほう。 All12 speech review_required/short_speech_uncertain, all output confirmed=false, two silences empty/ASR0. Fresh workers reproduce text/status/gate/review/counts. Existing normalization only; raw errors retained.
- `lexical-source-0/1.json`: public Commons metadata/revisions/SHA1/Artist/license. 車 by speaker/recorder葵心,2024-10-24, CC0; 本 by speaker/recorderFukujinzuke,2026-10-05, CC BY4.0. Audio attribution/URLs/SHA256/modifications are also in manifest source_records. Resampled mono16k PCM16 and transformed copies stay outside Git. Published transcription, not ASR-generated reference or independently listened conversation.
- `search-validation-*.json`: search coverage for はい/うん/いいえ. No new licensed はい/うん located; don't substitute 車/本 for natural acknowledgements.
- `prepare-license-rejection.txt`: initial preparation rejected CC0 because code initially allowed only CC BY-SA4. This occurred BEFORE inference. Correct license was verified, same sources/plan used in fresh directory. No failing measured audio removed; partial r1 artifacts retained outside Git.
- `static-check.txt`: Rust15+helper2 / Python12 / Node14,306endpoint-reference cases, fmt/compile pass. Doesn't establish official Python/Torch wrapper numerical execution parity.
- `provenance.json`: raw external probability hashes (both models ×two passes byte-identical), unchanged worker/core/model/probe hashes, source/evidence checksums.

No mic/desktop capture, playback, cloud inference, app/default/lock/model replacement, or quality acceptance. Old いいえ/burst/quiet failures remain in prior evidence. Additional transformations and isolated pronunciation words do not establish natural noise, conversational acks, continuous/long-duration or Windows operation.
