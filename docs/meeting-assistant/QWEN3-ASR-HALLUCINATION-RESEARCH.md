# Qwen3-ASR幻覚/発話保護の調査（2026-10-12）

> 後続の「修正して」で独立workerを修正済み。current512/full-inputの結果は[QWEN3-ASR-WORKER-FIX.md](QWEN3-ASR-WORKER-FIX.md)へ。本書は修正前の調査履歴を保持し、production採用/会議品質の合格を意味しない。

ユーザー依頼：「どうすれば解決できそうかを調べてほしい。」調査・公開fixtureの診断のみ。本体capture/provider、workerの現行gate/認識policy、モデル選択、既存lockは変更しない。選択は1.7B decoder INT8（encoder/prefill F32）のまま。音声をcloudへ送らず、実音声capture/playback/credential参照なし。

## 結論

**モデル交換/ブラックリストより先に、VADのモデル入力互換性を直し、発話有無判定とASR文脈境界を別々に評価する。** 問題は少なくとも三つ：①無音から生成、②VADの見落とし/誤検出、③入力境界による出力・表記変化。今のraw CER悪化をすべて音声欠落/幻覚と説明するのは不正確。修正案の本体採用はまだ決定しない。

## 1. 実装上の具体的な互換性問題

再利用した `emotechlab/silero-rs@26a646003cd8532ae2dde424ccdab1b6cdf5d7b0` はモデルを30ms = **480 samples@16kHz**で呼ぶ（`src/lib.rs` process内で固定）。一方、embedded ONNXのGit blob SHAは **e6db48d6e2a0797a2ec173c008384f7710189344**、SHA256は **a35ebf52fd3ce5f1469b2a36158dba761bc47b973ea3382b3186ca15b1f5af28**。GitHub APIのofficial **Silero v4.0**モデルblobと一致した。これは最新v6ではない。

Official v4 wrapperは `sr / x.shape[1] > 31.25` を拒否する。16kHzでは最低512 samples。学習windowは512/1024/1536で、それ以外は性能に影響すると明記。Rust側はこのチェックなしで480を送っている。**推論が成功することはofficial入力契約への適合ではない。** 既存アプリから同じ実装を再利用したことだけで妥当性を推定していた点を訂正する。

worker側の `FRAME` だけ512にしても依存libraryが480へ再分割するので直らない。最小案は独立実験用のpinned forkで内側のframe/time/sample計算を32ms/512へ揃えofficial参照結果と比較。中期案はofficial現行ONNX + official互換wrapperをRust/ORTで使う。v5/v6は512の新規samples +64のcontext、state `(2,batch,128)`を扱うため、古いh/c wrapperへ新しい重みだけ差し替えてはいけない。最新releaseは調査時v6.2.3（2026-09-23）だが新しいだけで短い相槌/false-positiveが治る保証はない。

### 同一v4重みのframe比較（新たな診断）

`worker/examples/vad_frame_probe.rs` をlocked/offline releaseでbuild。`VadSession.forward`へ480/512を直接入力し、同じPCM、各variantで新しいstate、gainなし/現行上限20倍gainを比較。ASR/endpoint policyは実行しない。weights/package/app変更なし。

| fixture | 480 | 512 |
|---|---:|---:|
| 500ms公開文crop、gainなし：最大speech probability | 0.168 | **0.574** |
| 同crop：p>0.5 frames | 0 | **5（160ms）** |
| -40dB日本語clip、gainなし：p>0.5 frames | 44（1.32s） | **107（3.424s）** |
| 無音 / 白色noise .002と.1 / 440Hz tone | p>0.5は全variantで0 | 同左 |

短cropの見落としにframe geometryが関係する証拠。ただしcropは未整列proxyで、真の「はい」の検出/認識成功を証明しない。positive-frame countはground-truth recallでもない。

**反証も保持：** 元日本語clipの冒頭約1秒の最大pは480で0.511、512で0.872（positive 2→14）。正しいframeにすると短音への感度は上がるが、自然音のfalse positiveが減るとは限らない。冒頭の「はい」が本当に幻覚か、実際の相槌/息/別音かは、reference不一致だけでは断定できない（音声を聴いていない）。**語を削除せず、整列・内容確認が必要。** 純無音から「はい」が出る既存ASR測定は別の確かな問題。

Raw probabilitiesは `evidence/qwen3-asr/worker/vad-frame-research.jsonl` / `vad-frame-leading-research.jsonl`。probe binary SHA256 **e6d2ffe98d4122038d657076a57e45cbf822a754d6293f723d9ecd893cd9eec1**。main worker binary/strategyを変更した測定ではない。

## 2. タイ語CER悪化の内訳：欠落だけではない

既存全30件のraw outputを再照合。TH baselineは **65/1002文字error (6.49%)**、bounded-window workerは **105/1002 (10.48%)**。

数字を含む3件（row4/6/7）は合計errorが **5→58（+53）**。例：row6の`1767`がタイ語の数詞 `หนึ่งพันเจ็ดร้อยหกสิบเจ็ด`へ変化、row7の`36`/`116`も数詞表記へ変化。この3件以外は **60→47（-13）**。全体の+40を「発話が40文字欠落した」と解釈してはならない。英語WERは9→11/175 wordsで、広範な脱落と断定できる大きさでもない。

主raw CER/WERは変更しない。別に数字/固有名詞/否定/末尾の意味保持を分類し、表記違いと誤認識・脱落・挿入を区別する。数詞の意味等価性は個別に確認し、評価を緑にするための一括正規化を入れない。VADによる切断が本当に原因かは語音の整列/境界比較が必要。

またbaselineはQwenCtx再利用、workerはモデルからfresh session、入力PCMもcropされたので複数変数が違う。**同一worker・同一PCM**のcounterfactual比較を先に行うべき。量子化自体の不具合という証拠はない。

## 3. 解決案の優先順位

### A. 最優先：モデル互換VAD + gate/切出しの分離

1. まずsame v4/512のofficial互換参照で、短音/quiet/noiseと境界を測る。続いてpinned現行v6と比較する。本体VADは変更しない。
2. **presence-only control**：VADで発話が全くなければASR/flushを呼ばない。発話がある30秒以内の入力は**原PCM全体**を同じworkerへ渡す診断modeを比較する。短い孤立VAD islandを別々に解読しない。
3. contextを保持したgateが有利なら、次に安全なpre/post余白やpauseでのwindow切りを調整する。発話有無判定のthresholdをASRの音声切断点として直結しない。窓上限/原timestamp/内部pause保持は維持。

presence-onlyは「発話を含む窓のnoiseが消える」方法ではない。弱い声をVADが見落とせば丸ごと捨てる問題も残る。長い無音を含む窓で誤生成/余分なcomputeが増えないか、終端遅延と合わせて比較する。30秒を推奨最適値とはしない。

### B. 相槌/quiet speech保護

- `はい`/`うん`など実在語の禁止、RMSだけのgate、1秒未満の一括削除をしない。
- short/quietの**正解内容と語音時刻が確認できる公開録音**を追加。単なる500ms cropやunit testでは合格にしない。
- sensitivity/false-positiveを同時測定。感度を上げるgain/threshold低下はnoiseも拾う。gainなし/ありを分け、mixed backgroundのSNRを変えて確認。
- 300ms無音primeは旧試験でnoise false-positiveを起こしたので既定に戻さない。
- Qwen Toolkitの1.5s最小発話はlong-file API splitting用。相槌を守りたい会議workerへそのまま移植しない。

### C. ASR側のno-speech/出力状態

Official Qwen parserはauto languageの `language None<asr_text>` を、textも空ならno-speechとして扱う。現行workerは全requestで言語をforceする。**auto vs forced language**を同一音声・同一境界で比較する価値はあるが、autoが純無音を必ず判定できるとは限らない。言語force解除を採用決定はしない。

空/NaN入力拒否、音声なしの推論/flush禁止、発話間state reset、原duration保持は現行workerで維持する。別backendのmlx-audio issue #928も同種のempty/padding/言語carry問題を報告しているが、その修正をRust engineの効果証拠と混同しない。

repetition/near-token-limit/short ambiguousはraw保持 + review。EOS判定/生成終了理由を将来露出する。Whisperのno_speech_probのようなcalibrated指標がQwen Rustにあると仮定しない。LLMに文字列を書き直させて音声根拠を作らない。

### D. 二段認識/forced alignmentは補助

必要なら疑わしい区間だけ別の音響ASR/alignerで確認するが、常時二重推論はCPU・memory・終了遅延を増やす。Qwen ForcedAlignerは11言語（Japanese/Englishを含む、**Thaiは含まない**）。与えたtextを音声へ強制配置するので、timestampが付いたことは「その語が話された」証明ではない。万能な幻覚filterとして扱わない。

## 次のcontrolled比較（未実施、採用提案）

| 比較 | 固定するもの | 確認するもの |
|---|---|---|
| existing480 vs official互換512 vs pinned v6 | 元PCM、gain設定、quiet/short/noise fixture | speech recall/false-positive、境界、原時刻 |
| 同じworkerのfull PCM vs現行crop vs余白増加crop | モデル・6threads・session・言語・token budget | raw CER/WER、数字意味/固有名詞/挿入/末尾欠落 |
| auto language vs forced | 境界/PCM/engine | 純無音誤出力、言語誤判定、相槌保持 |
| 原音 / quiet / noise混合 | 内容・話者・正解語音 | 弱音声をgateで全捨てしていないか |

合格条件：negative suppressionとpositive保持を別々に判定し、どちらかだけgreenなら採用しない。構造変更は独立workerの比較に限定し、本体接続前にユーザーの選択を得る。Windows/two-hour/live captureは別acceptance。

## コマンド・失敗

```sh
# pinned shell、model推論を並列起動しない
nix-shell -I nixpkgs=/nix/store/8vdpwi2yls679v4dnjcp3wynsq77jwhl-source benchmarks/qwen3-asr/shell.nix
export RUSTFLAGS="-C target-cpu=znver3"
export CARGO_TARGET_DIR=/home/user/Code/meeting-assistant-research/runtime/qwen3-asr/worker-target
cargo build --release --locked --offline --example vad_frame_probe --manifest-path benchmarks/qwen3-asr/worker/Cargo.toml
"$CARGO_TARGET_DIR/release/examples/vad_frame_probe" ../runtime/qwen3-asr/worker-fixtures/{silence-before,white-0.002,white-0.1,tone-440,short-prefix,quiet-0.01}.wav
"$CARGO_TARGET_DIR/release/examples/vad_frame_probe" ../runtime/qwen3-asr/artifacts/fleurs-ja_jp-validation-0-1519.wav
```

最初のleading probeは `worker-fixtures/speech.wav` が存在せず失敗。original fixtureはartifactsにあることを確認して再実行成功。webでToolkit `text_tools.py` は404、issue#93はforce-language一般の問題で今回のno-speech根拠ではなく除外。logs：親`logs/qwen-vad-frame-research.log` / `qwen-vad-leading-research-corrected.log`。Raw artifact/weighted probabilityは診断のみ、ASRの品質/速度/会議受入結果ではない。

## 一次情報

- [Silero v4 official input validation/trained frame sizes](https://github.com/snakers4/silero-vad/blob/v4.0/utils_vad.py)
- [v4.0 ONNX blob metadata](https://api.github.com/repos/snakers4/silero-vad/contents/files/silero_vad.onnx?ref=v4.0)
- [actual pinned Rust implementation](https://github.com/emotechlab/silero-rs/blob/26a646003cd8532ae2dde424ccdab1b6cdf5d7b0/src/lib.rs)
- [current Silero ONNX context/state implementation](https://github.com/snakers4/silero-vad/blob/master/src/silero_vad/utils_vad.py)
- [latest release metadata as retrieved](https://api.github.com/repos/snakers4/silero-vad/releases/latest) / [v6.2.3 release](https://github.com/snakers4/silero-vad/releases/tag/v6.2.3)
- [official Qwen input/empty-language/repetition parser](https://github.com/QwenLM/Qwen3-ASR/blob/main/qwen_asr/inference/utils.py)
- [Qwen long-file API Toolkit splitting](https://github.com/QwenLM/Qwen3-ASR-Toolkit/blob/main/qwen3_asr_toolkit/audio_tools.py)
- [Qwen model/aligner language coverage](https://github.com/QwenLM/Qwen3-ASR)
- [mlx-audio #928 (different backend; supporting issue report only)](https://github.com/Blaizzy/mlx-audio/issues/928)
