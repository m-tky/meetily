# Same1.7B INT8：PCM16追加量子化とfloat32保持をASRまで比較

## 承認・境界

次の隔離診断の提案に対するユーザーの「ok」を受けて実施。**同じ1.7B decoder INT8、encoder/prefill F32**、六thread、pinned Nix/LP64/frozen dependenciesを維持した。非量子化model比較（前回の提案3）、補正実装、cloud推論、本体integration、worker/model/default変更はしない。録音・再生なし。

元音源は既にPCM16。比較するのは減衰・noise生成時の**追加**PCM16丸めと、それをしないfloat32であり、録音masterの精度復元ではない。float32のASR経路は別のdiagnostic exampleで、現workerのPCM16契約は変更していない。

## 固定した実験

- 既存FLEURS30＋公開単語3（いいえ／車／本）の33source。
- baseline33、追加−40dBの33×2形式=66、単語padding／white SNR0の3×2×2=12、各言語first sourceのwhite SNR0×2=6、前回のnegative全65：**182窓**。
- VADは既にparity確認したofficial wrapper、固定.7/.55/>90ms、**VADのみpeak100**。ASRにはgainなしの全original inputを渡す。candidate/default採用ではない。
- gate missでも、referenceが公開speechの窓だけ診断としてASRを実行。negativeはgateが通った場合だけ実行。reference/hotword/過去textはASRに渡さない。各requestにfresh session、2fresh processesで反復。
- 実ASRのscoreと、gate missで空textとなる**gated projection**のscoreを分離。projectionは第2の実ASR呼出し／worker統合結果ではない。
- 新しい単語noiseにseed5122026を使用したため、既存車／本SNR0（7212028/7212029）も別の4窓×2fresh processesで追加recheck。PCM16 branchが既存失敗WAVのsamplesとbit同一であることをassert。旧失敗を新noiseの成功で置換しない。
- 真に別sourceのholdoutではない。planはASR前に保存、threshold/score/referenceは測定後に調整しない。

## 結果

主batchは**182/182**、legacyは**4/4**、text／tokens／gate／actual calls／review flagsが2fresh processesで一致。1passあたり実ASRは117＋4calls、negative65は全てgateなし／ASR0／空text。全出力unconfirmed。既存FLEURS30のbaseline textはfixed-full結果と全30一致。official model files／INT8 sidecar／tokenizer等のhashはbefore/after不変、production workerも変更なし。

追加−40dB：

| metric | float32保持・診断ASR | 追加PCM16・診断ASR | 追加PCM16・gated projection |
|---|---:|---:|---:|
| 日本語CER（13source） | 14/530 = **2.64%** | 16/530 = 3.02% | 3.02% |
| 英語WER（10source） | 19/175 = **10.86%** | 72/175 = 41.14% | 87/175 = 49.71% |
| タイ語CER（10source） | 85/1002 = **8.48%** | 150/1002 = 14.97% | 14.97% |

VAD検出はfloat32 **33/33**、追加PCM16 **29/33**（miss4は英語）。float32でgateだけでなくASR自体も改善した。一方、33pairの診断scoreはfloat32が**改善14／同点16／悪化3**。悪化したTH indices2/3/8のerror増分は+2/+3/+9。個別ケースで一律に良くなるとは主張しない。baseline英語WER5.14%よりfloat32静音WER10.86%はまだ悪い。

### 残る短語／noise失敗

両形式とも以下が残る：

- `いいえ`＋padding → **`家。`**
- `いいえ`−40dB／white SNR0 → **`いい。`**
- **旧noise seed**の`本`＋white SNR0 → **`ほう。`**（両形式で再現）

新seedの`本`は両形式`本。`だが、旧失敗を修復した証拠ではない。車は旧／新seedとも`車。`。新sentence-noise3pairs、short-extra6pairs、legacy2pairsは各言語のerror countに改善なし（同点はtext同一を意味しない）。padding／context／noiseに対する短語の誤認識は追加量子化だけでは説明・解決できない。

## 判断

**float32保持は追加情報損失を防ぐ有望な診断結果。ただし品質受入れ・採用は保留。** gain100の全575窓でのmiss、旧burst false-positive、短語の否定意味変化、自然な「はい／うん」、real noise／音楽／keyboard、continuous/live／Windows／長時間は解決・検証していない。synthetic negative65のFP0を実会議保証にしない。

次の提案は別licensed sourceでprecision条件を事前固定して検証すること（追加承認が必要）。補正は別途raw/evidence/historyと未確認proposalを保持する設計のみ。意味・否定・数値・日時・担当・決定の変更を自動確定しない。未認識／zero化audioを後処理で復元したことにしない。

## 実装・証跡

- `benchmarks/qwen3-asr/worker/examples/float_asr_probe.rs`：bounded mono16k f32le読込、nonfinite／range／length検査、strict metadata、fresh states、六thread/LP64検査。f32 tiny value保存・不正入力・reference hint拒否のtests。
- `evaluate_float_asr.py`：frozen prepare、モデル／input hashes、VAD mask、sequential two processes、actual/projection分離。
- `recheck_float_legacy.py`：既存noise signalのbit同一assert、main完了を待ってからsequential実行。
- `summarize_float_asr.mjs`：既存CER/WER、反復不一致／hash／call mismatchの検査、全raw保持、悪化も報告。
- compact evidence：`evidence/qwen3-asr/worker/asr-input/`。波形／model／cache／build／logsはGit外 `../runtime/qwen3-asr/diagnostics/{float-asr-r1,float-asr-legacy-r1}`、`../logs/qwen-float-asr-{prepare,run,legacy}.log`。

Attributionは既存[robustness](QWEN3-ASR-ROBUSTNESS.md)／[validation](QWEN3-ASR-VAD-VALIDATION.md)に従う：FLEURS (Conneau et al.2022), CC-BY4；いいえ/Zsrtrgh CC-BY-SA4；車/葵心 CC0；本/Fukujinzuke CC-BY4。独立listen／自然会話評価ではなく、公開referenceを使用。
