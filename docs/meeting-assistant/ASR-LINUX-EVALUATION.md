# 日本語ASR / Linux内部音声の予備評価（2026-10-12）

## 境界

マイク・実会議・デスクトップ音声を収録していない。公開音声をダウンロードし、**Meetily本体と同じWhisperEngine → WhisperProvider**をCLI exampleから呼んだ。cloud ASR/LLM、認証ファイル、アプリDB、telemetryは使っていない。音声の再生もしていない。

このexampleはcapture/mixing/resampling/VAD/queue/保存を通さない。実時間pipeline、話者分離、録音復旧、2時間会議の検証ではない。

## 公開fixtureと権利

- [Google FLEURS dataset card](https://huggingface.co/datasets/google/fleurs): Conneau et al., *FLEURS: Few-shot Learning Evaluation of Universal Representations of Speech* (2022)。card metadataはCC-BY-4.0。
- `ja_jp / validation` のrow index **0, 1, 2**（ID 1519, 1608, 1607）を固定。計41.94秒、正規化後155文字。別に人工の5秒無音を追加。
- FLEURSは読み上げ音声。雑音、重複発話、会議の専門語・人名への一般化を主張しない。3件は正式なbenchmarkやモデル採用の十分条件ではない。
- 当初test splitを要求したがrows APIはHTTP 500、first-rowsはHTTP 501。配信側が541,644,467 bytesのParquet読み込みに対し300,000,000 bytesの上限超過を報告。大量downloadで回避せずvalidationの先頭3件に変更。ASRの失敗ではない。
- rows responseにはdataset Git revisionがないため `revision: null` と記録。モデルのrevisionと、取得WAV/変換PCMのSHA256、正解文、sample countを `evidence/asr/{fixture,models}.json` に保持。再取得時もPCM hash/正解文/model hashの一致を確認。
- Whisper weightsは[OpenAI Whisper](https://github.com/openai/whisper)のMITライセンス。[ライセンス](https://github.com/openai/whisper/blob/main/LICENSE)。変換版の取得元は[ggerganov/whisper.cpp](https://huggingface.co/ggerganov/whisper.cpp)。モデル計635,553,432 bytesはruntimeのみ、Gitには含めない。再配布パッケージのlicense auditは別工程。

## 条件と測定

Ryzen 5 PRO 5650GE、Linux x86_64、Meetily 0.4.1、upstream lock、debug native build、CPU-only（GPU/BLAS featureなし）。日本語hint `ja`、本体のadaptive beam/temperature/cleanupを変更していない。ffmpegで16kHz mono f32leに変換。model load/hash確認とfile readは推論elapsedから除外し、load timeは別項目。

Warm-upなし、各音声1回。baseの初回はexample再buildと時間が重なったので探索記録 `base-exploratory.json` に分離し、build完了後にbaseを再実行したものを主表に採用。smallはbuild完了後の逐次実行。ホスト全体の他アプリを停止・専有した試験ではなく、統計的latency分布やrelease性能ではない。

CER = Unicode文字単位Levenshtein距離 / 正解文字数。NFKC・小文字化・Unicode句読点/空白除去。`2人`と`二人`など表記の意味同一化はしない。speech集計は音声3件のみ。RTF = 推論秒 / 音声秒、低い方が速い。無音のraw出力は句読点だけでも別に非空と数え、CER正規化で失敗を隠さない。

| モデル | 音声CER | 音声の推論秒 | 音声RTF | 無音のraw出力 |
|---|---:|---:|---:|---|
| Whisper base | 24/155 = 15.48% | 35.57s | 0.848 | `。` |
| Whisper small | 9/155 = 5.81% | 135.50s | 3.231 | `お疲れ様でした。` |

明細: `evidence/asr/base.json`, `small.json`。smallはこの3件で誤りが少ないが、CPU版の低遅延用途で採用可能とはいえない。baseも短い発話ではRTF>1の例があり、全体RTF<1だけでリアルタイム性を保証しない。

**無音に対するsmallの文章生成は既知の失敗。** upstreamのVAD前段を通していないので、これだけでアプリが無音を送ると断定しない。一方、構造テストの成功や句読点除去を「幻覚なし」の証拠にしない。次はVADを含む人工fixtureと無音/雑音/短発話、release/GPU/threads、chunking/queueを分けて測る。

ソース点検では `transcribe_audio_with_confidence` のconfidenceは文字列長のheuristicで、確率ではない。またadaptive `max_threads`を読みながらWhisper paramsへ適用していない。本評価では両方を変更していない。

## Linux内部音声：診断結果、未実装

`asr_fixture --devices` はCPAL列挙のみでcapture streamを開始しない。ホストは`Alsa`、入力名は `pipewire`, `default`, `sysdefault:CARD=Generic_1` 等で、**monitor名は0件**。`pactl list short sources`には複数の `.monitor` が存在する。

`configure_linux_audio` はALSA/CPAL入力名に `monitor` が含まれるものしかsystem deviceとして追加しない。`AudioStream::create_with_backend` は非macOSでCPALへ進むため、実名修正だけではこの環境のmonitorを選べない。`pipewire`という名前があることを、任意のmonitorを取得可能という証拠にしない。環境全体のdefault source変更はmic/systemを混同し得るので行わない。

- Nix shellのFFmpegはPulse入力対応。
- アプリの既存bundle FFmpeg 7.0.2-staticは **Pulse入力なし**（`-devices`で確認）。そのままFFmpeg captureへ置換できない。
- 次の候補はLinux専用native PulseAudio client（PipeWireのPulse互換serverも対象）でmonitorを列挙・選択し、既存AudioCaptureへ流す方法。mic/WindowsのCPAL経路は維持する。新dependency/stream lifecycleが必要なためask_userで確認し、**ユーザーがこのnative PulseAudio方式を明示承認した**。このcheckpointではまだ導入していない。次工程でbounded buffer、stop/join、切断通知、monitorとmicの識別を維持して実装する。
- synthetic null sinkのtone captureによるchannel/rate/stop/reconnect検証を予定。実desktop/mic収録の許可とは別に扱う。

## 再現

```bash
cd meetily
nix-shell shell.nix
# artifactsはrepo外。ffmpeg/nodeはshell内。
node frontend/scripts/prepare-asr-fixture.mjs ../runtime/asr
# 初回はモデル約636MB。既存modelはSHA256一致なら再利用。
# rows APIの最新snapshotを再取得するので保存済みfixture/model hashesとも比較する。
export CARGO_TARGET_DIR=/home/user/Code/meeting-assistant-research/murmur/target
cargo build --locked -p meetily --example asr_fixture
for model in base small; do
  timeout 600 "$CARGO_TARGET_DIR/debug/examples/asr_fixture" ../runtime/asr "$model" \
    > "../runtime/asr/$model.jsonl" 2> "../logs/meetily-asr-$model.log"
  node frontend/scripts/asr-metrics.mjs "../runtime/asr/$model.jsonl"
done
"$CARGO_TARGET_DIR/debug/examples/asr_fixture" --devices
pactl list short sources
node --test frontend/tests/lib/asr-metrics.test.mjs frontend/tests/lib/tauri-sidecar.test.mjs
```

raw Cargoでのsidecar stageはBASELINE/PROGRESS参照。テスト8件成功（metrics 5 + sidecar 3）、example native build成功。repo内artifact出力を拒否するguardも期待通り失敗しdirectoryを作らなかった。UI/本体の実装は今回は変更していないためfrontend production buildを再実行していない。

ログ: `../logs/meetily-asr-{prepare,reprepare,build,final-check,final-metrics,base-repeat-summary}.log`, `meetily-audio-device-probe.log`, `meetily-linux-capture-capabilities.log`。モデル・WAV・PCM・raw実行ログはrepo外。
