# Same1.7B INT8 ASR input diagnosis (approved after steps1/2)

See [report](../../../../QWEN3-ASR-ASR-INPUT.md). No worker/model/default/app adoption, nonquantized-model comparison, correction implementation, cloud inference, recording or playback.

- `main/`: frozen182windows (117speech/65negative), raw and enriched two-fresh-process outputs, full scored rows/pairs. All182 repeat text/tokens/review/gate/calls; all unconfirmed; negatives ASR0; baseline FLEURS30 match previous fixed text.
- `legacy/`: independently frozen4windows, exact old車／本SNR0 PCM16 signal asserted bit-identical, float32 counterpart, two fresh processes AFTER main completion. 本→ほう。 remains in both. New-noise success does not supersede the failed old seed.
- `provenance.json`, `post-audit.txt`: all186 prepared inputs and parents rehashed, official ASR/model cache unchanged, production worker SHA256 unchanged, diagnostic binary and sources hashed.
- `static-check.txt`: Rust19, Python20, Node18,306endpoint cases, rustfmt/Python compile/diff check PASS. `static-check-first.txt` preserves an initial command error: broad unittest discovery imported existing procedural `test_worker_fixture.py` without its required root; corrected explicit unit-module invocation passes. No failed clips dropped. An audit helper import attempted outside Nix missed libstdc++; no inference ran there, and stdlib audit inside pinned shell passes.

Quiet33pairs: float32 diagnostic error counts improve14/equal16/worsen3. English raw WER10.86% vs extra PCM16 41.14%; corresponding PCM16 gated projection49.71%. Float quiet gate33/33 vs PCM1629/33. Gated scores are **projections**, not an additional actual ASR invocation or integrated-worker evaluation. Source audio is alreadyPCM16; only subsequent quantization is compared. Original references/CER/WER are unchanged; raw text and all worsening cases retained.

Sources/attribution and limitations are in the report and preceding robustness/validation evidence. No new-source holdout/natural-ack/real-noise/live/Windows/long-duration acceptance. All waveforms/models/cache/builds/full logs remain under external `runtime/qwen3-asr`; probabilities are not committed.

Pinned shell: `nix-shell -I nixpkgs=/nix/store/8vdpwi2yls679v4dnjcp3wynsq77jwhl-source benchmarks/qwen3-asr/shell.nix`. Build with existing frozen worker lock, LP64, six threads, `RUSTFLAGS=-C target-cpu=znver3`, external `CARGO_TARGET_DIR`; `cargo build --release --locked --offline --example float_asr_probe --manifest-path benchmarks/qwen3-asr/worker/Cargo.toml`.

Recorded-cohort commands (inside pinned shell; these existing tags intentionally reject reruns; published models/data already prepared):

```sh
../runtime/qwen3-asr/vad-reference-venv/bin/python benchmarks/qwen3-asr/evaluate_float_asr.py ../runtime/qwen3-asr --tag float-asr-r1
rc=$?; printf '%s\n' "$rc" > ../runtime/qwen3-asr/diagnostics/float-asr-r1/run.exit
# This dedicated recheck waits for that main run.exit=0, not an arbitrary tag.
../runtime/qwen3-asr/vad-reference-venv/bin/python benchmarks/qwen3-asr/recheck_float_legacy.py ../runtime/qwen3-asr --tag float-asr-legacy-r1
```

The recorded runs used separate prepare/run phases and a shell exit-status marker; existing tags intentionally reject overwrite. The dedicated legacy recheck references `float-asr-r1`; do not use it as a generic scheduler for another active ASR cohort. Main raw stdout is JSONL; load/inference diagnostics remain in external logs. Actual ASR242calls total across four sequential processes.
