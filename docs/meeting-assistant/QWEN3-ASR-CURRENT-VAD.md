# 承認済み：公式現行VADとv4の独立比較

**後続検証で固定候補は不合格**：追加254窓でquiet speech6窓未検出。詳細は[QWEN3-ASR-VAD-VALIDATION.md](QWEN3-ASR-VAD-VALIDATION.md)。以下は89窓での探索記録を保持する。

前工程の診断後、ask_userで「公式現行VADと比較（推奨）」をユーザーが明示選択。**比較のみ承認、本体/workerへの採用は未承認・未実施。** 1.7B ASRは変更せず、この工程はVAD-onlyでASRの再推論も行っていない。capture/playback/cloud inferenceなし。

## 固定したモデルと契約

[公式Silero repository](https://github.com/snakers4/silero-vad)の調査時HEADを固定：

- revision `1e261b036686cd0017d500ee96acd1c4ba572a9d`
- `src/silero_vad/data/silero_vad.onnx`、2,327,524bytes
- Git blob SHA1 `80c5592ef1f4c9ede3e357bbd02eb863358a6a9d`
- file SHA256 `1a153a22f4509e292a94e67d6f9b85e8deb25b4988682b7e174c65279d8788e3`
- pinned pyproject package version **6.2.3**。package versionを独立したtrained-weight versionの証明とは扱わず、binary hashで特定する。
- MIT、Silero Team。既存`worker/licenses/Silero-MIT.txt`のnoticeは同じ本文（末尾newline差のみ）。モデル・公式wrapper source・pyproject/LICENSEはrepo外の`runtime/qwen3-asr/vad-models/silero-<revision>/`。各fileのGit blob SHA1をpinned treeと照合し、SHA256/URL/bytesを保存。

[pinned OnnxWrapper](https://github.com/snakers4/silero-vad/blob/1e261b036686cd0017d500ee96acd1c4ba572a9d/src/silero_vad/utils_vad.py)の16k pathを診断Rust exampleへport：**512 new samples +64 context = ONNX576**、state **[2,1,128]**、sr **scalar int64 16000**。output [1,1]、stateN [2,1,128]を毎frame検証しnonfinite/未知schemaを拒否。新しいfile/gainごとにstateとcontextをzero resetし、最終chunkのみVAD zero-pad。原PCM/実endpointは不変。v4のh/c[2,1,64] adapterへ新modelを無理に流すことはしていない。

既存のRust ort/宣言済みNix ONNX Runtimeを使用し、Cargo.lock/cache/dependency/appを変更していない。新wrapperは公式同様ORT inter/intra1、v4は既存silero-rsのintra4。**speed比較ではない**。schemaとframe/context helper2 testsは確認したが、公式Python/Torch wrapperそのものとの実行parityはまだ未確認。既存Python venvにonnxruntimeはなく、巨大なTorch等を追加する代わりに既存Rust runtimeでbounded探索を行った。これも正式採用前の残課題。

## 同一89 request windows / 同一endpoint rules

前工程と全く同じ89 windows（speech68、negative21）を、VAD gain none/peak20/peak100で比較。コピー/再送を含むwindow数であり、独立89音源の評価ではない。endpointは**worker設定を共通化**：positive .5 / negative .35、min speech >90ms、silent hold500ms、pre300/post200/merge。公式Pythonの全default settingsとの同一性は主張しない。threshold/gainによりASR側の原PCMを変えていない。

|model / VAD gain|speech window未検出|negative誤検出|
|---|---:|---:|
|v4 / none|6|2|
|公式現行 / none|3|0|
|v4 / peak20（worker現行gain）|2|2|
|公式現行 / peak20|0|1|
|v4 / peak100|1|2|
|公式現行 / peak100|0|1|

現行ONNX +peak20は、旧失敗burst RMS .002/.01を抑制（p_max .021/.036）、quiet EN -40dBとwhite SNR0を検出（p_max .718/.99985）。short word6条件/original30speechもpresence gateを通る。

しかし**新たにnegative harmonic tones RMS .002を誤検出**。VAD-only gain20、p_max .591、interval [0,4736)。noneなら検出されないがquiet EN/THとshort word -40dBを未検出にする。したがって「新版に替えるだけで全部治る」「gainを外せば安全」はどちらも成立しない。synthetic tonesは音楽ではない。VAD-only比較なのでこのtoneでのASR textは測定しておらず、誤検出を`はい。`生成と取り違えない。

二つの逐次pass（current-vad-r1/r2）の全probability JSONLはbyte同一。確率の再現性であり、speech品質/determinismの一般保証ではない。

## 前工程と同じ24設定grid：次の検証候補は得られた

gain3種 ×threshold .3/.5/.7/.9 ×minimum >90/>200msの同じgridを両modelへ適用。v4のゼロwindow-error設定は0。公式現行には7設定が存在した。

**min >90msを維持する候補：peak20 / positive .7（negative .55）**。この89windowsではspeech未検出0、negative誤検出0。peak100/.7/>90msも同様だが、今はgain上限を増やす理由にしない。min >200msでもゼロの設定があるが、まだ未評価の短い本当の相槌を消す危険を残す。

このgridは**同じfixtureで探索・選択したin-sample結果**。未使用noise/quiet条件、別speaker、自然会話に一般化した結果ではない。特にquiet ENのp_max .718はthreshold .7に近い。今は「次の固定候補」であって、既定変更/本体採用/品質受入ではない。

新VADで検出6/6になっても、前工程のASR短語`いいえ`→`家。`/`いい。`問題は別。full/cropの認識結果を再測定して改善を示した工程ではない。raw出力・レビュー・未確定を弱めず、VAD検出を転記の正しさへ読み替えない。

## 再現・コード・証拠

```sh
# meetilyからworker READMEのpinned Nix shell内。
# モデルdirは存在すると拒否する。取得済みなら再取得せずprovenance照合。
python3 benchmarks/qwen3-asr/prepare_current_vad.py ../runtime/qwen3-asr
export RUSTFLAGS='-C target-cpu=znver3'
export CARGO_TARGET_DIR=/home/user/Code/meeting-assistant-research/runtime/qwen3-asr/worker-target
cargo build --release --locked --offline --example current_vad_probe --manifest-path benchmarks/qwen3-asr/worker/Cargo.toml
cargo test --release --locked --offline --example current_vad_probe --manifest-path benchmarks/qwen3-asr/worker/Cargo.toml
# prerequisites: prior robustness/diagnostics fixture artifacts, hash checks enforced.
python3 benchmarks/qwen3-asr/compare_current_vad.py ../runtime/qwen3-asr --tag current-vad-r3
```

`prepare_current_vad.py`はpinned public filesのみ取得、Git blob照合とSHA256保存。`current_vad_probe.rs`はstate/context/shape契約付きのneural forwardだけ。`compare_current_vad.py`は既存fixture/v4 raw/model hashを検証し、共通endpointのpresenceとgridを保存。worker/main/lib/vad・ASR/model・既定policyは変更していない。

`evidence/qwen3-asr/worker/current-vad/`にreport（全window gate intervalsとgrid）、model source provenance、schema、license、source/binary/evidence hashesを保存。weights/audio/全probability arraysはGit外、外部raw hashはprovenanceに記録。logs：`../logs/qwen-worker-current-vad-build.log`、`qwen-worker-current-vad-r1.log`、`qwen-worker-current-vad-r2.log`。最終Rust15+新helper2/Python9/Node11、306 endpoint-reference cases成功、rustfmt check/Python compile成功（`qwen-worker-current-vad-final-check.log`）。model/probe/fixture/全raw hashと既存worker/core source不変を再確認した。

## 次の境界

1. 候補peak20/.7/>90msを**先に固定し**、未使用のnoise変形/別public source/複数speaker短語で検証。新しい失敗に合わせて評価対象を削除しない。公式wrapperとの独立実行parityも確認する。
2. 同一1.7Bの短語context不安定さを別に扱う。新VADで静音抑制できても否定語の転記が正しいとは限らない。
3. この比較承認はmodel replacement/app integrationの承認ではない。統合・live/Windows/実会議/二時間受入は引き続き保留。
