# 独立worker：小声・合成noise・短語の追加評価

ユーザーの「ok」で本体統合前の追加評価を実施。**追加品質評価は不合格。本体統合・連続処理への移行は保留。** 前の512/full-input修正が限定30clipで基準を保った事実と矛盾しないが、それだけではnoise/quiet speech/短語を保証できない。

選択済み1.7B decoder-INT8（encoder/prefill F32）、6 threads、LP64、same v4/512、presence-only full original PCMの**同じbinary**を使用。gate/model/認識contextのpolicyは変更していない。public audio + deterministic synthetic noiseのみ、capture/playback/cloud推論なし。ASR workerは逐次実行。

## 実験範囲と採点

- FLEURS validation既存manifestの先頭JA2、EN1、TH1（計4clip）。clean、-20/-40dB、white noise SNR20/10/0dB、lowpass noise SNR10dB、burst noise SNR10dB：**32 speech requests**。
- white/lowpass/burst/非言語harmonic tones × nominal RMS .002/.01/.05/.1、speech前後のsilence：**18 negative requests**。同じnegative silenceの前後2回を別IDで記録。
- Noiseはseed5122026で生成。burstsは250msごとの指数減衰random pulseで、**実keyboard音ではない**。tonesも実musicではない。SNRはpauseを含むwhole-clip RMS、active-speech RMSではない。mixは共通gainでclipを防ぐ。各WAVのSHA256、quantized PCM RMS/peak/saturated samplesを保存。
- nominal RMS .1のnegative burstは200sampleが飽和し、実RMS .092685。原JA2にも82saturated sampleがあり、そのまま保持。PCM16量子化/飽和とスケーリングは記録しており、全条件を完全な線形振幅比較とは扱わない。false-positiveになった.002/.01 burstには飽和なし。
- 元のCER/WER定義を変更しない。no_speechになったspeechも全削除として採点し、review_requiredのraw textも残す。negative文字挿入はspeech CERに混ぜず別表示。missing/duplicate/metadata不一致をreporterが拒否。

## 発話なしでも「はい。」を生成

**negative 16/18は空/ASR0、2/18は失敗。** nominal RMS .002と.01のburstのみVADがspeechを誤検出し、ASR1、raw `はい。`、status `recognized`。両方ともreview理由は空だが、全recognitionの`confirmed:false`は維持。

新しいworkerを立てる追加2passでも両条件で同じ現象を再現（最初のpassを含め各3回）。.002のVAD gainは約2.879、.01は1.0。したがって「VAD gainを外せば全部治る」とは言えない。純合成noiseなのでこの`はい。`は発話根拠のない誤生成と判断できる。自然音中の`はい`を根拠なく削除することとは別。

## 小声・noise中のspeech

これは**4clipの探索値**で、前の各言語10件/full corpusの結果とは別。

|条件|JA CER（2clip）|EN WER（1clip）|TH CER（1clip）|
|---|---:|---:|---:|
|clean|2.36%|0.00%|0.00%|
|-20dB|1.57%|0.00%|0.00%|
|-40dB|2.36%|100.00%|3.49%|
|white SNR20dB|2.36%|0.00%|0.00%|
|white SNR10dB|6.30%|13.33%|0.00%|
|white SNR0dB|10.24%|100.00%|30.23%|
|lowpass SNR10dB|2.36%|0.00%|0.00%|
|bursts SNR10dB|3.15%|0.00%|0.00%|

ENの-40dBとwhite SNR0は**VAD未検出/ASR0/空**。追加fresh-worker2passでも再現。-40dBではgain上限20でも救えなかった。JA/THの検出成功を全languageのquiet speech保護と一般化しない。表記変動（例：二人→2人）もraw採点に含む。noise条件の認識誤りがあっても現heuristicでは`recognized`になり得る。statusは信頼度ではない。

## 正解メタデータのある人間の短語「いいえ」

新規sourceは[Wikimedia Commons / Lingua Libre：LL-Q5287 (jpn)-Zsrtrgh-いいえ.wav](https://commons.wikimedia.org/wiki/File:LL-Q5287_(jpn)-Zsrtrgh-%E3%81%84%E3%81%84%E3%81%88.wav)。Speaker/recorder **Zsrtrgh**、2025-11-03、[CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/)。published transcriptionは`いいえ`。page revision/audio timestamp/SHA1/SHA256/extmetadataを保存。16k mono PCM16に変換し、0.8666875s/13867 samples。音源はGit外。変換/zero pad/attenuation/noiseをmanifestに明記。

これは**1speakerの単語発音**で、自然会話の相槌ではない。こちらで独立聴取してverified referenceとしたものでもなく、公開転記に基づくprobe。`はい`/`うん`の適切な音源は今回確保できず未評価。以前の500ms unaligned cropを正解不明の相槌として採点することはしていない。

|条件|raw text|検出/ASR|結果（既存正規化で比較）|
|---|---|---|---|
|原音|`いいえ。`|検出/1|一致|
|前後1秒zero pad|`家。`|検出/1|不一致|
|-20dB|`いいえ。`|検出/1|一致|
|-40dB|`いい。`|検出/1|不一致|
|white SNR10dB|`いいえ。`|検出/1|一致|
|white SNR0dB|`いい。`|検出/1|不一致|

**検出6/6、転記一致3/6**。全て`review_required/short_speech_uncertain`、raw保持。前後silenceは2/2で空/ASR0。clean/padded/-40dBはfresh-worker追加2passでも同じtext（各3回）。full PCMで文脈を保持する方式でも、短語の前後無音に対するASRの不安定さは残る。VADの未検出と認識の語の誤りは別障害として扱う。

## 再現・保存・検証

`evidence/qwen3-asr/worker/robustness/`にmanifest、全raw results、report、source metadata、failure recheck14rows、hash provenanceを保存。wave/model/buildは`../runtime/qwen3-asr`。既存fixed/履歴は上書きしない。初回noise runnerが後続silenceのIDだけmanifestへ入れ忘れた点は、結果/PCMを変えずscoring前に追記しnoteを残した。現runnerは推論前に追記する。PCM measurement metadataも初回は後計測で、同じhashのWAVに基づくことを明記。

```sh
# meetilyから、worker READMEのpinned Nix shell内で実行。
# tagごとの新しいdirを要求し、既存測定を上書きしない。
python3 benchmarks/qwen3-asr/evaluate_robustness.py ../runtime/qwen3-asr --tag synthetic-r2
python3 benchmarks/qwen3-asr/evaluate_short_speech.py ../runtime/qwen3-asr --tag lexical-r2
node benchmarks/qwen3-asr/summarize_robustness.mjs ../runtime/qwen3-asr/robustness/synthetic-r2
node benchmarks/qwen3-asr/summarize_robustness.mjs ../runtime/qwen3-asr/robustness/lexical-r2
python3 benchmarks/qwen3-asr/recheck_robustness.py ../runtime/qwen3-asr --noise-tag synthetic-r2 --short-tag lexical-r2 --output-tag failure-recheck-r2
python3 -m unittest discover -s benchmarks/qwen3-asr -p test_robustness.py -v
node --test benchmarks/qwen3-asr/test-robustness-report.mjs benchmarks/qwen3-asr/test-metrics.mjs
```

Noise generator2 tests、reporter3 tests、既存supervisor4/metrics5成功、Python compile成功。前工程のRust15/reference306の成功は維持し、今回Rustは変更していない。logs：`../logs/qwen-worker-robustness-r1.log`、`qwen-worker-short-speech-r1.log`、`qwen-worker-robustness-summary-recheck.log`、`qwen-worker-robustness-check.log`。

## 後続診断

ユーザーの「続けて」でsame-v4設定gridとASR-only/full-crop比較を実施。[QWEN3-ASR-DIAGNOSTICS.md](QWEN3-ASR-DIAGNOSTICS.md)に最新結果を保存。単純なthreshold/gain/cropを採用できる結果ではなく、本体統合禁止を維持。以下はその前に定めた診断境界。

## 次の修正境界

1. burst false-positiveとquiet EN false-negativeを同じ固定fixtureでgate比較し、precision/recallのtrade-offを測る。閾値を上げて小声を消す、RMS-only fallbackでnoiseを通すなどの一方向の対処を先に採用しない。
2. 短語のclean/padded/quietを同じモデルでcontrolled context比較し、VADとASRを切り分ける。clean単語一件の成功でshort-ack品質を合格としない。会話の`はい`/`うん`/否定語・別speakerを追加する。
3. 上記不合格を解消してからlive時間窓/終了遅延/長時間連続処理、本体統合へ進む。official現行VAD等の新dependency/model選択は別途承認・検証が必要。

語のblacklist、LLMによる発話根拠の捏造、数詞等の正規化緩和、失敗clipの削除で合格させない。実keyboard/music/環境音、自然会話、混在language、Windows、二時間運転は引き続き未検証。
