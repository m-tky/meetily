# NVIDIA話者分離のリアルタイムCPU試験

## 依頼・結論

ユーザー「リアルタイムで使えるかを試してみてほしい」に対する**独立実験**。
Ryzen 5 PRO 5650GE上で、NVIDIA Nemotron 3 DiarizationをCPUのみで実測した。

- 標準設定は60秒で処理遅れが増大して **不合格**。短い12秒の成功を採用根拠にしない。
- 履歴量を減らした固定候補は60秒の実時間入力に追従。日本語2話者の短語反復を順／逆順のfresh processで処理し、匿名2clusterを維持。処理速度の候補にはなる。
- **自然な会議の精度、本体での録音＋Qwen同時処理、長時間の話者記憶は未検証。リアルタイム製品機能の完成／採用ではない。**
- Meetily/Murmurの実装・表示中アプリ・既存録音・ASR設定・DB・lockfileは変更していない。以前の保存後手動実行の選択を、この独立試験だけで自動／リアルタイム実行へ変更しない。

## 固定artifact

- NVIDIA公式[NeMo-Speech.cpp](https://github.com/NVIDIA/NeMo-Speech.cpp)、revision `083861c241061562b8ec29eb06d7f6cdc3af8775`。
- llama.cpp submodule `bd4f514db14d87fded667787a7a963bfbaa98e89`、公式patch適用。外部benchmarkを`cpu-diar`／Releaseでbuild。本体releaseを作成した意味ではない。
- [NVIDIA Nemotron 3 Diarization](https://huggingface.co/nvidia/Nemotron-3-Diarization) revision `f667ed73aee57d40cc39428eb768b4fd87a0a29e`、公式indexのQ8_0 GGUF、107012128bytes。
- SHA256 `08456d9e22cd9a323c0364d98375f3746d6e68507ebb705cd46438c534c7a3a1`。公式CLI pullでsize/hash検査成功。
- runtime Apache-2.0／モデルOpenMDW 1.1。再配布・本体採用のlicense/notice確認は別工程。
- standalone C ABI `nemo_speech_diar_*`、`gpu=-1`、upstream既定CPU4threads。モデルcapacity8、今回試した話者は2人。Pythonは計測・PCM供給用、推論はnative C++/ggml。
- sources／models／build／全frame履歴はrepo外 `../runtime/nvidia-diarization/`。音声のcloud upload／ネットワーク推論はなし。

## 測り方

`scripts/probe_nvidia_diarization.py`：事前取得済みの公開mono16k WAV／人工無音・toneをメモリで供給。**音声デバイスを開かず、物理再生しない。**

- 120msごとに元PCMを分割。paced試験はmonotonic時刻で音声末尾の到着予定時刻まで待つ。
- 推論が遅くてもsampleを捨てず、次の供給を遅らせてbacklogを可視化する。音声の全保持は実mic callback試験の代わりではない。
- RTF＝全push処理時間＋finish処理時間／音声秒数。モデルload・paced待機・Python結果取得は含めない。
- completion lateness＝各pushと結果取得の完了時刻−そのPCMの到着予定時刻。ゼロまで戻る一時的なblockingと、持続的なbacklog増大を分ける。
- 暫定label出力age＝最初にそのframeが公開されたwall時刻−そのframeの音声時刻。最終的に確率>=.5のspeakerを持つframeに限定。UI表示／ASRテキストへの付与／label確定の遅延ではない。
- labelは確率最大の匿名cluster（最大確率<.5なら0）。公式既定postprocessingの最終segmentsも保存。両者は同じものではない。
- 固定followup計画は `../runtime/nvidia-diarization/experiment-plan.json`。結果を見て追加retuningはしない。

## 結果（CPU-only）

| 設定／音声 | 秒数／供給 | RTF | wall秒（待機＋finish含む） | 最大push完了遅れ | 最大RSS MiB |
|---|---:|---:|---:|---:|---:|
| 既定：chunk13/cache264/FIFO80、FLEURS反復 | 60／paced | **1.681** | 109.219 | **47.496秒** | 318.9 |
| chunk4（320ms）・他は既定、FLEURS反復 | 24／非paced | **2.990** | 71.888 | 非pacedにつき評価なし | 306.7 |
| 固定候補：chunk13/cache96/FIFO40、FLEURS反復 | 60／paced | **0.626** | 60.627 | 0.930秒 | 291.0 |
| 同候補、日本語短語2話者 | 60／paced | **0.648** | 60.629 | 0.953秒 | 291.1 |
| 同候補、同2話者の順序反転 | 60／paced | **0.635** | 60.620 | 0.960秒 | 291.4 |
| 同候補、人工無音 | 12／非paced | 0.401 | 4.831 | 評価なし | 253.2 |
| 同候補、440Hz/.05人工tone | 12／非paced | 0.400 | 4.809 | 評価なし | 253.1 |

- 既定は15/30/45/60秒時点で完了遅れ0.849／10.953／29.686／46.897秒。初期12秒のRTF0.452だけなら問題を見逃す。
- 候補は同時点の遅れがFLEURS0.337／0.136／0.665／0.0002秒、2話者0.339／0.234／0.686／0.0002秒。順序反転も最終0.0002秒に戻る。1.04秒center chunk＋80ms右contextに対する最大推論pushは約0.95秒。buffering/blockingは残る。
- 候補の暫定speaker frame出力age：FLEURS中央値1.338／最大2.147秒、2話者中央値1.345／最大2.153秒、順序反転中央値1.349／最大2.111秒。2話者最初のlabelは開始後1.266／1.253秒。
- 2話者は両順序でcluster1/2のみ、既に公開されたthreshold labelの変更events0。これは限定fixtureでの観測であり、誤識別がない／将来の訂正が不要という保証ではない。
- 無音・toneはspeaker label／最終segmentとも0。実環境noise/musicは未評価。
- EOFはnative10ms gridで6001frames＝60.009999秒。入力は960000samplesちょうどであり、録音を延長したものではない。
- RSSはPython供給・計測・モデルhash照合用readを含むプロセスhigh-water mark、モデル単体／推論だけの定常メモリ量ではない。

## 公開音声・精度の限界

- [Google FLEURS](https://huggingface.co/datasets/google/fleurs)、CC BY 4.0、`fleurs-ja_jp-validation-0-1519.wav`（既存取得済み、日本語読み上げ、反復＋500ms無音）。SHA256 `725511728984c6eb4353a73aebe6cf77ed1914b1bd71e24a2dae517284e2d171`。
- [Lingua Libre：葵心「車」](https://commons.wikimedia.org/wiki/File:LL-Q5287_(jpn)-%E8%91%B5%E5%BF%83-%E8%BB%8A.wav)、CC0。
- [Lingua Libre：Fukujinzuke「本」](https://commons.wikimedia.org/wiki/File:LL-Q5287_(jpn)-Fukujinzuke-%E6%9C%AC.wav)、CC BY 4.0、speaker/recorder Fukujinzuke。今回の変換：既存mono16k変換音声を4回ずつ交互反復、各clip後500ms無音、60秒でsample切断。原録音・metadataは既存runtimeに保持。
- 上記変換PCM hashes：車 `23dbb51f080b9d84c8fff8a39b3d9eb1b56fde4adeb06608baff78f88e8aa2ed`、本 `d1780859bf2d8e3408dbb19280b7d50060844bd7c6f644aea17a1fec45c00209`。
- 10ms binsのRMS>.005に限定した**source identity proxy**は2話者順／逆とも100%（1850／1806bins）。匿名clusterの全体permutationを許し、無label／同一clusterへのcollapseはpenaltyにする。人間annotated DERではなく、声と単語が完全に対応した反復fixtureなので一般化しない。
- FLEURS同proxyは既定58.60%、候補61.07%にとどまる。RMSによる音声正解範囲は人手注釈ではなく、backgroundをspeechと数える可能性もあるため「39%のspeech loss」と断定しない。しかし2短語の100%から日本語会議全体の精度合格を主張してはいけない。
- cache264→96／FIFO80→40はAPI validationを通った非既定のgeometry。履歴縮小は処理速度を改善したが、離れて再登場する話者・8人・雑音・重複発話の品質を維持するとは限らない。
- 未検証：録音callback／UI／保存との接続、Qwen6threadsとのCPU競合、実会話・短い相槌・遠距離・multi-hour／speaker cache忘却・20分以降compaction・日本語会議DER。

## 再現

```bash
# 外部checkoutでのみ。アプリのshell/lock/defaultを変更しない。
cd ../runtime/nvidia-diarization/NeMo-Speech.cpp
git submodule update --init --depth 1 llama.cpp
nix-shell -p cmake ninja sentencepiece pkg-config python3 --run \
  'scripts/configure.sh cpu-diar -DNEMO_SPEECH_BUILD_MIC_CAPTURE=OFF -DNEMO_SPEECH_BUILD_EXAMPLES=ON && cmake --build --preset cpu-diar -j6'
NEMO_SPEECH_MODEL_DIR=../models build/cpu-diar/bin/nemo-speech pull nemotron-diar
cd ../../../meetily
nix-shell -p python3 --run 'python3 -m unittest discover -s scripts -p test_probe_nvidia_diarization.py -v'
# --source は公開fixtureのみ。実音声・資格情報は与えない。
nix-shell -p python3 --run 'python3 scripts/probe_nvidia_diarization.py \
  --library ../runtime/nvidia-diarization/NeMo-Speech.cpp/build/cpu-diar/bin/libnemo_speech_asr_c.so \
  --model ../runtime/nvidia-diarization/models/nvidia/Nemotron-3-Diarization/f667ed73aee57d40cc39428eb768b4fd87a0a29e/Nemotron-3-Diarization.q8_0.gguf \
  --source ../runtime/qwen3-asr/diagnostics/current-vad-validation-r2/lexical-converted-0.wav \
  --source ../runtime/qwen3-asr/diagnostics/current-vad-validation-r2/lexical-converted-1.wav \
  --fixture turns --seconds 60 --cache-frames 96 --fifo-frames 40 --pace \
  --output ../runtime/nvidia-diarization/bounded-turns-recheck.json'
```

Pure harness tests6 PASS（PCM16/float32/odd RIFF、rate/NaN拒否、sample保持・source identity、threshold、permutation／collapse／missing penalty、negative counter）。上流code・モデルは変更なし。

ログ：`../logs/nvidia-diarization-{clone,submodule,build,model-download,default-smoke,default-paced-60,followups,bounded-validation,repeat-and-tests}.log`。不合格default／短chunkの結果を保持。次に本体統合するなら別途方式を確認し、非blocking sidecar・有限queue・暫定表示／unknown・保存後再解析との関係を設計する。
