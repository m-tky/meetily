# 固定VAD候補の追加検証：不合格、既定/本体は変更しない

後続の承認された1・2では公式wrapper実行parityを確認し、gain/追加PCM16量子化を分離した：[QWEN3-ASR-VAD-INPUT.md](QWEN3-ASR-VAD-INPUT.md)。本記録の失敗は保持し、ASR短語誤りの解消や本体採用には読み替えない。

ユーザーの`continue`で前工程の候補 **公式現行ONNX / peak20 / positive .7 / negative .55 / min speech >90ms** を先に固定した。planを推論前に保存し、結果を見た再調整・fixture除外・基準緩和は行っていない。既存v4/.5と現行ONNX/.5をcontrolとした。採用/統合は未承認・未実施。

## 対象と限界

- **254windows = speech220 + negative34**。
- FLEURSの前回noise変形で使っていない残り26clips ×8条件 =208speech。clean / -20dB / -40dB / white SNR20/10/0 / lowpass SNR10 / bursts SNR10。new seeds7212027+source index。26clean clipsは元30clip gridでも見ているので、**完全な音源holdoutとは呼ばない**。未使用の変形条件での検証。
- Commons/Lingua Libreの新しい2speakerの短語 ×6条件 =12speech。clean / 前後1秒zero padding / -20/-40dB / white SNR10/0。相槌ではなく単語発音。
- 2new seeds × white/lowpass/bursts/new-frequency harmonic tones ×4nominal RMS (.002/.01/.05/.1) +前後silence =34negative。tonesは従来110/220/440Hzをコピーせず、83/166/332Hzと149/298/596Hz、phase π/7。syntheticでありreal keyboard/music/環境音ではない。
- SNRはpause込み全clip RMS。PCM16後のRMS/peak/saturationを推論前に保存。clean5clipsとRMS .1 burst2clipsにsaturationがあるが削除せず記録。取りこぼしたquiet6clipsとshort-word failureにはsaturationなし。
- モデルhash/512new+64context/state[2,1,128]/scalar int64 srは前工程と同じ。official Python/Torch wrapperの独立実行parityはまだ未確認。全window presenceでありannotated boundary recallではない。

## 固定候補は未検出を増やした

|設定（gain上限20、min >90ms）|speech未検出 /220|negative誤検出 /34|
|---|---:|---:|
|既存v4 / positive .5|1|3|
|公式現行probe / positive .5|5|0|
|公式現行probe / **固定positive .7**|**6**|**0**|

候補の未検出はすべて-40dB：EN validation indices1/3/4/5/7、TH index1。これら6clipはv4/.5ならすべて検出。新wrapper p_maxはそれぞれ.541/.322/.447/.075/.416/.266（全VAD gain20）。EN index1はthreshold .5なら検出、.7に上げることで追加未検出。残る5clipはthresholdを元の.5へ戻すだけでも救えない。

v4はTH index7白色SNR0を未検出、新seedburst3条件を誤検出した。新版でnoise false-positiveを減らせても、静かな実speechのwindow presenceにtrade-offが出る。前工程の89windowsでゼロerrorだったことは、一般化を示さなかった。**候補を採用しない。ここではgridを再探索しない。**

全254inputs ×3neural gain policiesのprobability JSONLは、両modelとも逐次2passでbyte一致。比較/scoringは事前指定peak20だけを使用し、未使用gain出力を新候補選びへ流用していない。

## 新speaker短語：検出とASRを分離

|published source|speaker/recorder|license|音源timestamp|変換後|
|---|---|---|---|---:|
|[車](https://commons.wikimedia.org/wiki/File:LL-Q5287_(jpn)-葵心-車.wav)|葵心|CC0|2024-10-24|0.9453125s|
|[本](https://commons.wikimedia.org/wiki/File:LL-Q5287_(jpn)-Fukujinzuke-本.wav)|Fukujinzuke|CC BY4.0|2026-10-05|0.776s|

public APIのfile revision/音源SHA1/converted SHA256/Artist等全文metadataを保持し、published SHA1をdownload bytesと照合した。mono16k PCM16、zero padding、attenuation、synthetic white noiseを派生物として記録。referenceはpublished transcriptionで、ASR生成や独立listenではない。Commonsの`はい`/`うん`/`いいえ`検索も保存。新しいlicense-backed `はい`/`うん`は見つからず、相槌未検証を他の単語で合格扱いにしない。

3VAD設定とも短語は検出12/12。ASRは**新VADを組み込まず、既存v4/full-original-input workerのcontrol**として12speech +2silenceを処理。前後silence空/ASR0、全出力confirmed=false。

- 車：6/6条件 `車。`
- 本：clean/padded/-20/-40dB/white SNR10は`本。`、**white SNR0は`ほう。`**。
- 既存のNFKC/punctuation/whitespace除去CERで一致11/12、2character errors /12reference chars =16.67%。語辞書/意味補正/失敗除外なし。raw text保持。
- 全12speechはreview_required/short_speech_uncertain。fresh worker2passでtext/status/gate intervals/review reasons/ASR-call counts再現（timingは比較から分離）。

新短語の正解が増えても、以前の`いいえ`→`家。`/`いい。`失敗は撤回しない。VAD検出12/12は認識の正しさや自然な短い相槌の保護を保証しない。

## 実行・証拠

`validate_current_vad.py`はplanを先に保存し、prepare/measure/short-ASR-controlのphaseを分離。新ONNXも既存probeもhash固定、manifestはmissing/extra/duplicate/reference mismatchを拒否。worker/core/default/app/ASR model/locksを一切変更していない。

```sh
# meetily、既存worker READMEのpinned Nix shell内
python3 benchmarks/qwen3-asr/validate_current_vad.py ../runtime/qwen3-asr --tag current-vad-validation-r4 --prepare-only
python3 benchmarks/qwen3-asr/validate_current_vad.py ../runtime/qwen3-asr --tag current-vad-validation-r4 --measure-only
python3 benchmarks/qwen3-asr/validate_current_vad.py ../runtime/qwen3-asr --tag current-vad-validation-r4 --short-asr-only
# repeatはfresh dirへmanifest/planだけコピーしてmeasure/short-ASR-only。
# audioは元のrepo外pathsのまま、NN passesは逐次実行。
node benchmarks/qwen3-asr/summarize_current_validation.mjs MANIFEST FIRST_SHORT_JSONL SECOND_SHORT_JSONL OUTPUT
```

runtime：`../runtime/qwen3-asr/diagnostics/current-vad-validation-r2/`、r3（JSON-only repeat）、source searchは`short-speech-sources/search-validation-*.json`。初回r1はlicenseをBY-SA4限定していたため、車の**CC0**でprepare段階で拒否（推論なし）。metadataを確認してCC0/BY4/SA4を許容し、**同じsource/同じ候補**をfresh r2へ準備。r1/traceback/partial audioを残した。これは測定失敗clip除外ではない。

`evidence/qwen3-asr/worker/current-vad-validation/`にplan、manifest（全PCM hashes/metadata）、report（全gate詳細）、short ASR raw2pass/summary、source metadata、tests、provenanceを保存。audio/weights/large probability arraysはGit外。

最終Rust15+現行frame helper2 / Python12 / Node14成功、306 endpoint-reference cases・fmt・Python compile成功。new Python3/Node3はplan/coverage/references/duplicate/normalization/repeat fail-closed checks。各model/blob/probe/runtime/全254WAV hashとworker binary/core source不変を確認。

## 次の境界

固定候補をfailとして保持。次に進むなら、quiet speechの入力scaleとgate confidenceを**別の比較**として扱い、失敗を消さずprecision/recall trade-offを測る。新しい候補はまた検証前に固定し、独立した音源/変形で確認する。公式wrapperとの独立runtime parity、真の会話相槌、ASR noise/context、live/long-running/Windowsは未確認。**本体統合・既定VAD replacement・会議品質受入は引き続き禁止。**
