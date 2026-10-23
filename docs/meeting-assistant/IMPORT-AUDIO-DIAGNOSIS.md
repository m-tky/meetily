# Recorded-audio import: bounded fidelity / recognition diagnosis

## Authorization and scope

After discussing remaining accuracy limits, the user explicitly requested diagnosis and improvement of any discovered problems. This supersedes the historical pause on accuracy experiments **only for this bounded investigation**. Existing licensed public FLEURS fixtures and the two already saved virtual Windows recordings are the only inputs. No new capture/playback, real microphone/desktop/conversation, cloud inference/authentication, model download, installed-app replacement, installer, tag or release operation.

Baseline: `1a99712`. Tested six-file patch SHA256 `9ddc83ffd0a27ccb39da2d395a264099cd963a1945362a86f20bdd92bf2e4e0c`; Windows candidate SHA256 `72af39fdef4432a541446d7e782c3fc2b24e6081520c65146f1211ee74ce3f68`. Audio/model/language/normalizer/VAD thresholds and context policy remain unchanged except for the finite-resampler correctness repair below. Qwen-only/default 1.7B remains; no retired engine or text correction is introduced. Original recordings, transcripts and profiles are preserved. New diagnostic output stays outside Git.

## Confirmed defect: finite sinc lookahead was never drained

`audio_processing::resample` constructed `SincFixedIn` with exactly the real input length, called `process` once and returned immediately. Its lookahead needs future samples. For these 48kHz inputs, 16kHz output was **86 samples (~5.375ms) shorter** than the rounded rational duration. Very short inputs could disappear completely. In the large-file path, independently resampled 60-second chunks each suffered this loss, so later audio/timestamps could shorten cumulatively.

A production-code regression run before the fix recorded **3 PASS / 2 FAIL**: a final 6ms marker disappeared (measured tail energy 0), and 17 source frames at 48kHz produced 0 instead of 6 output frames. Impulse timing, anti-alias rejection and identity already passed; those requirements were not relaxed.

Repair: supply a kernel-width of EOF zeros, then retain exactly the rounded `input_frames × output_rate / input_rate` real duration. Existing sinc length/interpolation/cutoff/window/phase are preserved. No unvalidated `output_delay()` subtraction is added: the pinned implementation already places the interior impulse at its input time. Zero sample rates are rejected. This fixes the shared **finite** helper, not the separate streaming capture resampler or VAD downsampler.

Six native finite-resampler tests cover short/nonintegral input lengths across 48/44.1/8/16kHz conversions, tail energy, impulse position, anti-alias rejection, identity/empty input and invalid rates. A decoder regression forces the 60-second chunk path on a 61-second synthetic signal and checks exact output length plus boundary/end markers. Dedicated Windows packaging now gates both finite-resampler and decoder tests.

On both recorded files, Linux fixed output gains the missing 86 frames (172287→172373 and 170239→170325). The entire old output prefix is **bit-identical**. The actual VAD-delivered WAV remains **byte-identical** before/after: this utterance ends well before EOF. Therefore this real fidelity bug is **not a demonstrated fix for the reported `ワカシ` recognition**.

## Recognition / decoder controls (no policy tuning)

`examples/import_audio_probe.rs` calls the actual production decoder, mono/normalization/sinc conversion and batch VAD (2000ms), exporting explicitly supplied local fixture PCM into a new directory. It never starts capture/playback/inference or opens an application profile. The worker controls separately use actual existing weights and the unchanged six-thread/full-input float32 protocol.

- Linux: 14 actual 1.7B requests = original WAV plus both saved recordings through native decoding, FFmpeg decoding + native conversion, and FFmpeg 16k conversion, each before and after app VAD.
- Windows: 8 actual 1.7B controls = original, host-preprocessed A full/crop, Windows-native A full/crop, host/Windows FFmpeg-decoded A crops, native B crop. Exported WAVs are passed directly to the same Windows worker; they are not simulated recognition.
- Input duration, samples, PCM hashes, VAD times and raw model outputs are retained. No word is changed to satisfy the reference. Numeral spelling (`30` vs `三十`) is not described as a semantic error.

### Findings

1. **No gross AAC decoder degradation found.** At 48kHz, native decoding has 1024 leading frames relative to FFmpeg (21.333ms). After aligning those frames, compared PCM residual SNR is ~132.8dB / 125.5dB on A/B; max differences ~5.4e-7 / 2.24e-5. The earlier unaligned 16k comparison is confounded by fractional-frame phase and must not be described as codec noise. The general MP4 edit-list/gapless discrepancy remains a timing limitation; do not strip a hard-coded 1024 frames from arbitrary formats. FFmpeg9 and the previous FFmpeg7 also retain different final padding; exact sample-count parity across decoders is not asserted.
2. **VAD exports the corresponding PCM slice exactly** in these fixtures. This is sample transport evidence, not human-annotated proof of correct speech boundaries on all recordings.
3. **Context matters.** Windows-native recording A is correct when its full ~10.77s input is passed to the worker, but its ~4.24s batch-VAD crop gives `散策するなワカシがあります`. Linux's full native A/B and native crops are correct in this run; the FFmpeg+native-resample B crop instead gives `のワクシ`, while its full input is correct. Decoder replacement is therefore not a demonstrated universal cure.
4. **Tiny numerical differences matter in this case.** Host and Windows native crops have identical length/times; maximum PCM difference is only ~1.1921e-7. In the SAME Windows worker, host A crop recognizes `のは価値`, Windows A crop recognizes `なワカシ`. This is not a different-language/model/copy-corruption comparison. Four alternating requests plus two reversed-order requests in a fresh worker reproduce each input's respective exact text (three observations per input across two processes). This rules out request order as the explanation in these controls. Final GUI imports also retain the earlier raw outputs, including the errors.

These observations establish input/context sensitivity, **not** a proof that model training is the only cause or that a particular backend is defective. Full context works for this short example, but replacing batch segmentation, always adding silence, rounding/quantizing PCM, gain tuning or adopting a different decoder requires broader validation (including known quiet-speech/noise/short-word failures). None was adopted to fit this sentence. Recognition quality is still not accepted.

## Verification and limitations

Linux: **124 unique native PASS**, existing import fixture **1 ignore**; includes six SRC, 18 decoder, 20 pipeline, 23 transport/setup, import/retranscription and save regressions. Node **48 PASS**. Optional older audio fixtures may return early; the explicit recorded-file probes and actual-worker controls supply the actual AAC/ASR coverage here. No lock/dependency/schema change.

Windows: **121 unique native PASS** (including synthetic SIWC14; existing import fixture 1 ignore), Node **48 PASS**, packaging PowerShell parser **0 errors**. Repaired candidate re-imports both saved AAC files via each actual model (4/4), creates four isolated selected-folder meetings, preserves original/copy hashes and JSON/SQLite/null-source equality, cleans job workers/temp input, and reopens all four after native PID **7724→3740**, with unchanged persisted hashes. Preferred global Large and original private recording preferences are restored. On Windows too, old/fixed pre-VAD prefix is bit-identical and VAD-delivered WAV hashes are unchanged. The prior recognition errors are consequently **still present**, not rewritten or accepted as correct.

All official model file hashes and both copied cache hashes match their manifests; original recordings and cache top-level executable/DLL backups remain unchanged/restored. Owned app, all tested workers and debug listener are closed. Final summary: `../runtime/import-audio-diagnosis/evidence/final-summary.json` (32 actual ASR requests across bounded controls + final GUI). Isolated import/restart is a functional check, not an exact-reference or natural-meeting acceptance gate. No normal installed application is updated by this work.

Git-excluded evidence: `../runtime/import-audio-diagnosis/`, `../logs/import-audio-diagnosis/`; guest source/evidence `Z:\meetily-windows-transfer.HnU5H1\qwen-only-retest-9d37569\import-audio-diagnosis`. Relevant artifacts: pre-fix failing/native passing logs, `inference-baseline.json`, `cross-platform-comparison.json`, `fix-comparison.json`, Windows control/repeat JSON, patch manifest, candidate build and GUI audits.

Harness failures are retained: the first Windows PCM dumper made one 4-byte write per sample across virtiofs and was too slow; only its verified owned probe was stopped, its controller exited/restored originals, partial output/logs were preserved. The probe now uses `BufWriter` with explicit flush; DSP/ASR inputs/assertions were not changed. A host-side attempt to move a guest-owned partial directory was denied; that original remains in place, with a separate attempt archive. A metrics-print command lacked host `python3` after the successful native checks; rerunning with the existing venv succeeded. No user data was deleted to clean up a failed trial.
