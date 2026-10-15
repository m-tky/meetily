# Linux opt-in Qwen ASR setup

本体録音用の選択式adapter。既存Whisper/Parakeet/defaultを置換しない。追加の精度研究ではない。**AI草案・要音声確認**。短語・否定語・noiseの既知誤認識は未解決。品質受入・会議／長時間安定性・配布版完成を意味しない。

## ローカル準備

このcheckoutのstandalone workerを使用する。古いPCM16-only workerやcrop/VAD-only workerはready handshakeで拒否する。モデルや共有libraryはアプリへ同梱／自動downloadしない。

1. `benchmarks/qwen3-asr/PLAN.md` と `worker/README.md` のpinned source/official 1.7B重み・LP64 OpenBLAS準備を使う。必要ファイルは `config.json`, `model-00001-of-00002.safetensors`, `model-00002-of-00002.safetensors`, `qwen-asr-int8.sidecar`, `vocab.json`, `merges.txt`。0.6B/他モデルを選ばない。decoderだけINT8、encoder/prefillはF32。
2. frozen standalone worker lockでbuildする。既存app lockfileは変更しない。

```sh
cd /home/user/Code/meeting-assistant-research/meetily
nix-shell -I nixpkgs=/nix/store/8vdpwi2yls679v4dnjcp3wynsq77jwhl-source benchmarks/qwen3-asr/shell.nix
export CARGO_TARGET_DIR=/home/user/Code/meeting-assistant-research/runtime/qwen3-asr/worker-target
# この5650GE用。別CPUへ配る場合はtarget-cpuを変更する。
export RUSTFLAGS='-C target-cpu=znver3'
cargo build --release --locked --offline --manifest-path benchmarks/qwen3-asr/worker/Cargo.toml
printf '%s\n' "$ORT_DYLIB_PATH"
```

現在のローカルprepared paths:

- worker: `/home/user/Code/meeting-assistant-research/runtime/qwen3-asr/worker-target/release/qwen-speech-worker`
- model directory: `/home/user/Code/meeting-assistant-research/runtime/qwen3-asr/artifacts/model-1.7B`
- ORT: shellの `$ORT_DYLIB_PATH`。今回 `/nix/store/x46rnziynpalr4phm6yh0x46s42xq5kw-onnxruntime-1.27.1/lib/libonnxruntime.so`。別machineでは同じpathが存在する保証なし。

Nix buildのworkerはLP64 libraryのstore RPATHを持つ。この環境でapp shellから起動確認済み。一般LinuxではworkerとORTのruntime dependenciesを別途揃える。ILP64／6以外のthreadsはworkerが拒否する。library不足のとき別ASRへfallbackしない。

## UIで選択

1. 録音を停止し、Transcript Modelの設定で **Qwen3-ASR 1.7B INT8 (Linux / experimental)** を開く。
2. worker executable／model directory／ORT shared libraryの絶対pathを入力。
3. Japanese / English / Thaiを明示選択。これは既存の共通language設定を変更する。auto／英語翻訳は今回未対応。
4. **Validate and select Qwen**。設定ファイルを原子的保存し、worker ready handshake成功後だけproviderを選択する。設定はapp configの `qwen-asr.json` に保持。provider選択は既存transcript config経由。失敗は画面表示する。
5. 録音開始前にbackendでmodel/language/handshakeを再確認する。実microphone/desktop収録のテストは別途許可が必要。

音声はmono16k float32 WAVでprivate cacheへ一時保存し、request終了／失敗／キャンセルで削除。追加PCM16化・gain・自動ASR補正なし。既存app VADで抽出されたspeech burstを渡し、worker内は同じv4 presence-only/full-input policy。worker上限を超えたburstは連続30秒以下のrequestに分け、全sampleを保持してraw textを改行連結する。新しい30秒境界の品質やpipelineの長いburstの遅延は未検証。subrequest途中で失敗した場合、部分成功を確定扱いせずburst全体をエラーにする（元録音の保存は独立）。

stdout framing bound／request ID／confirmed:falseを検証する。ready30秒、request180秒timeout。失敗／timeout／キャンセルはchildをkill-on-dropし、次requestだけfresh workerを起動。失敗した音声は自動再送しない。1000requestsでprocessを更新。StopでQwen shutdown。warning/errorは既存文字起こしeventを通す。確度数値や話者を捏造しない。

## 録音なしの検証

```sh
# app shellへ戻り、WORKER/MODEL/ORTには上記の絶対pathを設定する
nix-shell shell.nix
export CARGO_TARGET_DIR=/home/user/Code/meeting-assistant-research/murmur/target
export MEETILY_DISABLE_ANALYTICS=1
cargo test --locked --offline -p meetily --lib audio::transcription::qwen_provider
cargo run --locked --offline -p meetily --example qwen_adapter_smoke -- "$WORKER" "$MODEL" "$ORT"
```

smokeは生成5秒無音をJA/EN/THで処理。device capture、再生、cloud、資格情報、ASR精度追試なし。weightsはspeechでlazy-loadするため、このsmokeは実speech推論／モデル全tensor integrity／録音全体の試験ではない。

## GUI接続／障害の合成回帰

private Pulse＋eSpeak人工音声と固定応答fixture-worker（実weightsなし）で、開始→VAD→Stopでmanager除去後のtail保存→SQLite／再起動→再閲覧、exit7後の次burstでfresh PID、completion保存失敗の継続警告／checkpoint保持を確認。Qwenのconfig reloadでAPI key lookupがInvalid providerだった不具合をlocal/no-keyとして修正。ASR失敗でUIだけ停止する不具合も修正し、録音/Stopを維持。実Qwen speech推論の成功ではない。詳細：[RECORDING-GUI.md](RECORDING-GUI.md)。

## 実weightsを使う合成GUI接続確認

その後ユーザー承認で、実worker／既存prepared1.7B weights／ORTを直接起動し、private sinkのeSpeak英文全再生→推論中Stop→manager除去後tail→raw event/disk/SQLite保存→native再起動／同meeting再閲覧を隔離profile3回で確認。追加量子化なしのmono16k float32完成済WAVを観測、worker終了・temporary cleanupも成功。raw文／segmentationは変動し誤認識を補正せず保持。自然会議／日本語・タイ語品質／長時間・配布版の保証ではない。新download、model/default/VAD/gain/correction変更、実mic/desktop/cloud/LLM/authなし。[REAL-QWEN-GUI.md](REAL-QWEN-GUI.md)。再現用explicit `--real-*` flagsはtests/gui/README.md参照。

## 残る製品作業

Linux monitor capture、停止後tailの最終議事録、編集保護、export/reopen、stop timeout後のtask lifecycleとcrash復旧、配布package／モデル導入UX、全体操作のsynthetic sink試験は `LINUX-MVP.md` の未完項目。batch import/retranscriptionは現時点で既存Whisper/Parakeet専用であり、Qwenをbatchの選択肢に追加していない。app crashで残った一時音声のcleanupは未実装。実録音・cloud試験は未実施。
