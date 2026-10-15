# 同じv4/1.7Bでのgate・ASR・context切り分け

その後ask_userで公式現行VADの**比較のみ**をユーザーが明示承認し、別artifactで実施した。最新結果は[QWEN3-ASR-CURRENT-VAD.md](QWEN3-ASR-CURRENT-VAD.md)。以下は承認前のsame-v4診断記録であり、モデル/本体採用承認ではない。

ユーザーの「続けて」で、追加robustness失敗を固定fixtureのまま診断した。**設定変更は採用していない。本体統合禁止・追加品質不合格を維持。** same worker binary/weights/LP64/6threads、VADはsame v4/512。capture/playback/cloud推論、新model/dependency/app変更なし。

## 1. 閾値・gainだけでは両方を救えない

VAD-only exampleでfresh neural stateの確率列を取得。ASR用PCMを変えず、VAD gainだけを`none` / 現行`peak20` / 診断`peak100`へ変えた。peak20/100はいずれもpeak .1目標・増幅のみ・上限20/100倍。positive threshold .3/.5/.7/.9、negativeはpositive-.15、min speech >90/>200ms、silent hold500ms/pre300/post200/mergeを固定：**24通り**。

対象は追加noise50 + short word8 + original corpus31 = **89 request windows**（speech68、negative21）。87 distinct WAV paths。コピー/再送を含むため独立89音源や音声全体のannotated recallではない。各windowのspeech presenceのみを数える。各neural出力の現行endpointがRust実装と一致すること、追加評価の現行gate判定58件が再現することをチェック。frame probabilitiesはGit外、SHA256のみ保存。formatting後にgate-r2で再実行し、gate-r1と確率JSONLがbyte同一だった。

|診断設定（gain / threshold / min）|speech window未検出|negative誤検出|
|---|---:|---:|
|現行 peak20 / .5 / 90ms|2|2|
|gainなし / .5 / 90ms|6|2|
|peak100 / .5 / 90ms|1|2|
|peak100 / .5 / 200ms|1|1|
|peak100 / .7 / 200ms|2|0|
|peak100 / .9 / 90ms|5|0|

**24設定中、未検出0かつ誤検出0はない。** これは全v4対策の不可能性の証明ではなく、この限定gridの結果。元30speechが通る設定でも、noisy/quiet/短語では失敗する。

- gain100でquiet EN -40dBのmax pは .459→.963、検出を回復。しかしwhite SNR0 ENはgain上限変更でもp_max .261のまま未検出。
- burst RMS .002はgainなしでもp_max .892、現行gainでは .703。RMS .01はgain1でもp_max .540。増幅を外すだけでnoise誤検出を解決できない。
- peak100/.7/>200msでnegativeは全部抑制できるが、white SNR0に加えて**burst SNR10の本当のEN speechも消す**。quiet ENだけの成功で採用しない。
- min durationもtrigger/temp-end/EOFで決まるintervalの長さであり、実際に声が連続した時間とは違う。長い誤検出を短語filterだけで排除できる保証はない。

## 2. EN取りこぼしはVADがボトルネック、このclipではASRが読める

新example `ungated_speech_probe`で**正解転記のある既知speechだけ**をVADなしで認識。immutable modelを常駐させ、各passでfresh KV/encoder/prompt、forced language、skip_silence=false/past conditioning=false/full bounded PCMを使用。worker fallbackを追加したものではない。LP64/thread fail-closed、18GiB address-space cap、240s process timeout。

9条件 ×2 fresh recognition sessions =18requests。EN clean/-20dBは基準文、未検出だった-40dB/white SNR0でも両passで:

> When you call someone who is thousands of miles away, you're using a satellite.

referenceの`you are`が`you're`になり、既存raw WERは**13.33%**。意味を概ね保持した読み出しであり「完全一致」ではない。-40dB PCMを診断100倍にしても同じtext。EN1clipの成功を全noise/quiet speechへ一般化しない。speechを知らない本番inputでVADを無条件bypassするとnegativeの幻覚を復活させ得るため、採用しない。

## 3. 短語はgate検出とは別にASR contextが不安定

前のlicensed Lingua Libre/Zsrtrghの単語`いいえ`をそのまま利用。published transcriptionに基づき、独立聴取でreferenceを確定したものではない。natural conversation/はい/うんは未評価。元6条件に前/後/両側100/300/1000msの既知zero padding8条件を追加：**14入力 × full/crop =28requests**。原sampleを勝手に整音/trimしていない。

same workerの既定full_inputと既存診断`--crop-context`を逐次比較。

|条件|full raw|crop raw|
|---|---|---|
|原音|いいえ。|いいえ。|
|前後1s zero（既存padded）|家。|い。|
|-20dB|いいえ。|いいえ。|
|-40dB|いい。|いい。|
|white SNR10|いいえ。|いいえ。|
|white SNR0|いい。|いい。|
|前100ms|いい。|いい。|
|前300ms|いい。|一。|
|前1000ms|いいえ。|イ。|
|後100/300/1000ms（各3条件）|全ていいえ。|全ていいえ。|
|両側100ms|家。|いいえ。|
|両側300ms|いい。|イエ。|

両modeとも**検出14/14、既存正規化で一致7/14**。cropで1件を救う一方、fullで正しかった前1000msを壊す。旧30clipでcropが英語/タイ語を悪化させた事実も維持。どちらかに一律変更、両結果の都合よい選択、LLM補正による合格扱いはしない。

診断ungated例でもclean=`いいえ。`、padded=`家。`、-40dB=`いい。`を各2passで再現。-40dB PCMを100倍にしても`言え。`で不一致。量子化後のPCM増幅は失われた情報を復元しないし、単純音量補正だけで否定語品質は救えない。この診断はdecoder-INT8とBF16の因果比較ではない。

**review flagもconfidenceではない。** 両側100/300ms full、両側300ms cropの誤りは`recognized`（review理由なし）だった。gate intervalはpaddingを含むため、short heuristicだけで全ての短語を要確認にできるわけではない。全出力`confirmed:false`・raw保持を維持し、誤認識を確定証拠へ昇格しない。

## コード・検証・証拠

- `worker/examples/vad_gain_probe.rs`: VAD-only gain ablation、same weights/512。
- `diagnose_gate.py`: probability取得、診断endpoint設定grid、Rust現行parityと既存判定照合。diagnostic scalarはf32 boundaryを明示。Silero MIT notice維持。
- `diagnose_context.py`: same worker full/crop、既知zero pad比較。
- `worker/examples/ungated_speech_probe.rs`, `diagnose_ungated.py`: known speechのみのgate bypass/PCM gain診断、fresh session×2。
- `summarize_diagnostics.mjs`: missing/duplicate/hash/mode不整合を拒否し、mode別・grid別のcompact reportを作る。失敗とraw textを残す。

```sh
# meetilyからworker READMEのpinned Nix shell内で実行。
export RUSTFLAGS='-C target-cpu=znver3'
export CARGO_TARGET_DIR=/home/user/Code/meeting-assistant-research/runtime/qwen3-asr/worker-target
cargo build --release --locked --offline --example vad_gain_probe --example ungated_speech_probe --manifest-path benchmarks/qwen3-asr/worker/Cargo.toml
# fresh tag必須。各ASR processは逐次、同時に走らせない。
python3 benchmarks/qwen3-asr/diagnose_gate.py ../runtime/qwen3-asr --tag gate-r3
python3 benchmarks/qwen3-asr/diagnose_context.py ../runtime/qwen3-asr --tag context-r2
python3 benchmarks/qwen3-asr/diagnose_ungated.py ../runtime/qwen3-asr --tag ungated-r2
node benchmarks/qwen3-asr/summarize_diagnostics.mjs ../runtime/qwen3-asr gate-r3 context-r2 ungated-r2 summary-r2
python3 -m unittest discover -s benchmarks/qwen3-asr -p test_gate_diagnostics.py -v
```

`evidence/qwen3-asr/worker/diagnostics/`にcompact summaries、full raw context/ungated結果、manifest、source/binary/evidence SHA256を保存。大きな全probability arraysとgrid詳細、audio/weights/buildはruntime。provenanceで外部raw hashも保持。最終Rust15/Python9/Node11 testsと306 endpoint-reference cases成功、rustfmt check/Python compile成功。new diagnostic scalar parity300列はPython3 tests内の確認であり、speech品質テストではない。最終logは`../logs/qwen-worker-diagnostic-final-check.log`。計算時間は探索diagnosticでwarm-up/performance benchmarkではない。少数短語でのthread/input/session determinismを一般保証しない。

最初のungated example buildはupstreamのOption-return APIに`?`を直接使ってcompile失敗。`anyhow::Context`に修正して再build成功。失敗logは`../logs/qwen-worker-diagnostic-build.log`へ保持し、成功testとして数えない。その他logs：`qwen-worker-gate-diagnostic-r1.log`、`qwen-worker-context-diagnostic-r1.log`、`qwen-worker-ungated-diagnostic-r1.log`、`qwen-worker-diagnostic-summary-r1.log`。

## 次の境界

- v4の単純threshold/gain変更は採用しない。新しい公式VADとの同一fixture比較が次の有力候補だが、**新重み/geometry/state/API契約を調べて別途承認後に比較する**。それでもASR短語問題は別に残る。
- 短語は別speaker/別referenceの自然会話を追加し、context/別engineの対照を行う。1speakerに合わせたcrop秒数や語置換で解決扱いにしない。
- real recording、app integration、live/Windows/two-hour受入は引き続き行わない。

公式VAD候補の一次情報：[Silero repository](https://github.com/snakers4/silero-vad)、[README](https://github.com/snakers4/silero-vad/blob/master/README.md)。MIT/ONNX wrapper availabilityは確認したが、新modelはまだ取得・実行・採用していない。
