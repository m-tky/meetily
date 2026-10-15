# Public robustness probes — quality acceptance FAILED

Same repaired 512/full-input worker binary, 1.7B decoder INT8/encoder-prefill F32, Linux LP64 six threads. No policy/model changes, no app integration, capture/playback or cloud inference. Detailed methods/results/limits: [QWEN3-ASR-ROBUSTNESS.md](../../../../QWEN3-ASR-ROBUSTNESS.md).

- `synthetic-r1/`: FLEURS JA2/EN1/TH1 ×8 conditions (32speech) +18 non-speech requests. Burst-noise two conditions generate `はい。`; quiet/noisy English is missed. This tiny subset is NOT the prior 30clip baseline. FLEURS attribution: Conneau et al. (2022), CC-BY-4.0, original source/API/hash in existing artifact manifest and these source hashes.
- `lexical-r1/`: one human-recorded isolated `いいえ` ×6 conditions +2silence. Reference comes from published transcription, not independent listening. Detection6/6 but normalized transcript matches3/6. Padding→`家。`, -40dB/white SNR0→`いい。`. All flagged short-speech outputs preserved. Not spontaneous acknowledgement acceptance; はい/うん not evaluated.
- `failure-recheck-r1.jsonl`: seven selected conditions repeated using two fresh workers,14requests. Both burst false positives, both English misses, and short-word clean/padded/-40dB behavior reproduce.
- `provenance.json`: unchanged binary hash and evidence/source file hashes.

Audio/model/builds remain outside Git. No original or derived audio distributed here. The lexical source is **Zsrtrgh**, speaker/recorder, 2025-11-03, Lingua Libre, [Wikimedia Commons source](https://commons.wikimedia.org/wiki/File:LL-Q5287_(jpn)-Zsrtrgh-%E3%81%84%E3%81%84%E3%81%88.wav), [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/). Local transformations: mono16k PCM16 resampling, zero padding, attenuation and synthetic noise. Preserve attribution/license/changes and share-alike if distributing derived audio. Source revision/extmetadata/timestamp/SHA1/SHA256 are retained in lexical metadata/manifest.

SNR uses whole-clip RMS, not active speech. Synthetic bursts are not real keyboard recordings; tones are not music. Nominal .1burst PCM saturates200samples; actual RMS is recorded. Initial metadata completion notes are in the manifests. No missing/review-required speech removed; errors/misses/negative insertions remain in report/raw results. Recognition status is not confidence, and all text stays `confirmed:false`.

Verification: noise-generation2/report3 + existing supervisor4/metrics5 tests passed; Python compile passed. Commands/logs in the detailed report. These are tool/protocol checks, not speech-quality acceptance. Real conversation/environmental noise, Windows/live/two-hour reliability remain unverified.
