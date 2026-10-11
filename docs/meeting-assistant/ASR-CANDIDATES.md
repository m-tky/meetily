# ローカル日本語ASR候補（2026-10-12）

ユーザーの「別のモデルの候補は？」への一次資料調査。モデルを追加downloadしたり、provider/backendを切り替えたりしていない。Linux native PulseAudio対応の承認は維持。以下は採用決定ではなく評価優先度。

前提: Windows/Linux、Ryzen 5 PRO 5650GE CPUを基準、音声をcloud送信しない、日本語会議、低遅延。現行CPU debugのWhisper base/small予備値だけから他モデルの速度・精度を予測しない。

## 第一群

1. **ReazonSpeech k2-v2 / int8 / sherpa-onnx**
   - 日本語専用159.34M、Zipformer/RNN-T。公式model card: Apache-2.0、約30秒以下の音声clip。
   - 公式APIはCPUとint8/int8-fp32を提供し、OfflineRecognizerを利用。CPU低遅延候補の最初に測定したい。ただし本ホストの速度・会議品質は未測定。
   - Meetilyへ新ASR providerが必要。RNNTというだけでこの配布物を真のstreaming対応と扱わず、まずVAD区切りのoffline推論を評価。
   - [card](https://huggingface.co/reazon-research/reazonspeech-k2-v2), [official API](https://research.reazon.jp/projects/ReazonSpeech/api/reazonspeech.k2.asr.html)
2. **Kotoba-Whisper v2.0 / q5_0**
   - 日本語向けlarge-v3蒸留。Apache-2.0。公式GGML/量子化weightsあり、既存Whisper.cpp engineを再利用しやすい。
   - 公式のlarge-v3比6.3倍は本CPUでの予測値ではない。large-v3 encoderを保持するため、smallより速いとも限らない。ASRだけの比較にはv2.0を使用し、v2.1/v2.2の追加pipelineと区別する。
   - [card](https://huggingface.co/kotoba-tech/kotoba-whisper-v2.0), [official GGML/q5_0](https://huggingface.co/kotoba-tech/kotoba-whisper-v2.0-ggml)
3. **Whisper large-v3-turbo / 量子化**
   - MIT、既存engine系で比較できる多言語候補。turboはlarge-v3を高速化したもので、base/smallより速いという意味ではない。CPU速度・量子化の精度低下を測る必要あり。
   - [official README](https://github.com/openai/whisper), [model](https://huggingface.co/openai/whisper-large-v3-turbo)

## 追加候補と注意

- **Qwen3-ASR-0.6B / 1.7B**: 日本語対応、Apache-2.0。LLM型のASR。まず小さい0.6Bを評価候補とする。公式のstreaming inferenceは現在vLLM backend限定で、CPU主体のRust desktopへ容易に組み込めると仮定しない。一般のqwen3:1.7b議事録fixture失敗とは別のモデル・タスク。[card](https://huggingface.co/Qwen/Qwen3-ASR-0.6B)
- **Fun-ASR-Nano-2512**: 800M、日本語対応、cardはApache-2.0、CPU指定とGGUF/native runtimeの案内あり。選択artifactの実際のlicense、runtime、CPU速度とtimestampの品質を先に確認する。tagにstreamingがあるだけで、日本語native CPUの低遅延streamingが完成していると判断しない。[card](https://huggingface.co/FunAudioLLM/Fun-ASR-Nano-2512)
- **SenseVoiceSmall**: 日本語対応、非自己回帰、ONNX exportあり。CPU速度を評価する価値がある。ただし現行official cardは`license: other`でFunASR MODEL_LICENSEへリンク。第三者記事の「Apache-2.0」をモデルweightsの根拠にしない。再配布/商用利用の審査が必要。[card](https://huggingface.co/FunAudioLLM/SenseVoiceSmall), [model license](https://github.com/modelscope/FunASR/blob/main/MODEL_LICENSE)
- **faster-whisper int8**は別モデルではなくWhisperのCTranslate2実装。CPU/int8で同じsmallを比較し、モデル差とruntime差を分離する価値がある。ただし既存Rust engineの差し替えやPython sidecar導入は別の設計判断。[official implementation](https://github.com/SYSTRAN/faster-whisper)
- stock **Parakeet TDT 0.6B v3**は公式cardの25言語に日本語が含まれない。Meetilyの既存Parakeet経路を日本語用としてそのまま採用しない。日本語向け派生weightsは別途card/品質/licenseを確認する必要がある。[official card](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3)

## 評価の境界

次に測る順序の提案は **ReazonSpeech int8 → Kotoba q5_0 → turbo量子化**。共通の公開fixtureを拡張し、無音、雑音、短発話、人名、数字、日英混在を追加。CER、raw無音誤生成、load time、speech RTF、chunk-endから確定までの時間、peak memoryを分離する。runtimeはrelease/threadsを揃える。3件の読み上げや他人のGPU throughput値だけでモデル採用を決めない。

今回の検索では二次資料にParakeetの日本語対応やSenseVoiceのライセンスについてofficial cardと一致しない記述があったため、上記は一次資料に限定した。ReazonSpeech GitHub READMEの推測raw URLは404だったが、official model cardとAPI docsを直接確認できた。
