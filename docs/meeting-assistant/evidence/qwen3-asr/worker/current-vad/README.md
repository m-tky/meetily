# Approved official-current VAD comparison — no model/policy adoption

Detailed methods/commands/limits: [QWEN3-ASR-CURRENT-VAD.md](../../../../QWEN3-ASR-CURRENT-VAD.md).

User explicitly chose official-current VAD comparison after the same-v4 diagnostics. This approval is NOT replacement/app integration approval.

- `model-provenance.json`: official repo revision1e261b036686cd0017d500ee96acd1c4ba572a9d, pinned model/wrapper/license/pyproject Git blob SHA1 and computed SHA256. Package metadata6.2.3; weight identity is the model hash, not a claimed independent training version. Model binaries/sources are outside Git.
- `report.json`: runtime schema,89same windows, same endpoint settings, gain none/peak20/peak100, full gate intervals/false-positive/miss lists and same24setting grid per model. VAD-only; ASR not rerun. Current default-equivalent gain20/.5/>90ms yields0miss/1tone-FP vs v4's2miss/2burst-FP. Seven exploratory current-model settings yield zero window errors; peak20/.7/>90ms is a future validation candidate, NOT held-out quality acceptance.
- `static-check.txt`: existing Rust15 +current frame-helper2, Python9, Node11,306 endpoint reference cases pass; fmt/compile pass. These do NOT establish official Python/Torch wrapper runtime parity.
- `provenance.json`: unchanged existing worker/core sources, current probe/source/evidence hashes, external raw probability hashes. Two current passes are byte-identical.
- `Silero-MIT.txt`: current upstream MIT notice (Silero Team).

New ONNX contract:512new+64context→576input; state[2,1,128]; scalar int64 sr=16000. Reset state/context per file/gain. Same existing local ORT; current wrapper inter/intra1 vs old adapter intra4. Not a speed comparison. The Rust16k wrapper port is checked against source/schema/helper tests; independent official-wrapper numerical execution parity is still unverified.

Speech/audio reference provenance and licenses remain in sibling `robustness/` and `diagnostics/gate-fixtures.json`: FLEURS (Conneau et al.,2022), CC-BY-4.0; Lingua Libre isolated `いいえ` by Zsrtrgh, CC-BY-SA-4.0. Window counts include copied/repeated audio, not89independent recordings or aligned recall. No real noise/music/keyboard/meetings or natural acknowledgements are accepted. Short-word ASR context errors remain a separate blocker. All audio/weights/large probability arrays stay outside Git. No capture/playback/cloud inference/credential/app/default changes.
