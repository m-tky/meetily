# 実Qwenと合成音声のnative GUI接続確認

ユーザーが「テスト用workerを実Qwenに置き換え、合成音声で同じ一連操作」の継続を承認。追加の精度研究ではなく、選択済Linux 1.7B decoder-INT8を使う製品フローの確認。実mic/desktop/会話、cloud/LLM/authは使わない。既存ASR/default、VAD/gain/補正policy、Cargo/pnpm locks、worker/modelを変更しない。

## 試験

`frontend/tests/gui/recording_native_smoke.py` に全て明示必須の `--real-worker` / `--real-model` / `--real-ort` を追加。省略時は従来fixtureモード。realモードと故障注入は同時指定できない。

- 新規HOME/XDG・private D-Bus・無効ALSA hardware・private Pulse server（null sinkとUnix protocolのみ）。eSpeak固定英文を専用sinkへだけ再生。外部serverの実音声は開かない。
- realモードは**worker直接起動**。protocol proxy／応答の保持・生成・補正なし。app-owned float32 mono16k WAVの完成済RIFF headerと、owned native process group内の実worker PIDだけを観測する。実binary hash、model file size/mtime、ORT pathは隔離profileへ記録。
- Start/Stopは実DOM `.click()`、初期の選択設定は公開native commands。設定はその隔離profileだけに保存する。物理ポインター／設定画面の全入力操作の受入ではない。
- 合成音声の**全再生完了を待つ**。pending ASR inputを確認してStopし、global manager除去後にも追加の本物transcriptが届くことを要求。先行live segmentがあっても、末尾segmentの到着を独立確認する。
- `transcription_drained=true`／`save_succeeded=true`／ASR error eventなし。raw transcriptとnative eventの全sequence/text一致、未知confidence=-1を確認。
- audio.mp4をdecodeし、有限・非zero、元合成音声の長さ以上（100ms許容）の保存音声を確認。これは短時間の接続smokeであり全sample同一や負荷欠損の保証ではない。
- SQLiteのfolder binding／全transcript textを確認。native app再起動後、同meetingを再閲覧し、SQLite全行のID/text/audio時刻・録音folder全files/checkpoints hash不変を確認。
- Stop後、実worker PID終了とprivate Qwen input WAV削除を確認。model filesのsize/mtime不変も確認（再hashで全weightsのbyte不変を証明する試験ではない）。

## 結果

全再生版を新しい隔離profileで3回実行し、全て成功。原稿は8.655646秒、保存audioは10.221313／10.221313／9.621313秒（capture開始／停止の前後無音を含む）。Stopボタンからrecording-stoppedまで6.157／6.230／4.538秒。**これらはpipeline drain／モデルload・認識／保存を含む観測値で、ASR単独latency／一般の性能保証ではない。**

raw認識は実際に変動した。1回目は冒頭が `Set recording for the recording test.`、2回目は原稿の冒頭から出力、3回目は2segmentsで最初が `Synthetic audio for the recording test. The proposal is pending.`。差を削除／補正／採点正規化せず保存。今回の合格条件は各runのraw出力がevent/disk/SQLite/restartで保持されること。自然会議・日本語／タイ語・否定語等の品質合格や再現性を主張しない。既存の短語／noise誤認識失敗は撤回しない。

logs（repo外）：

- `../logs/meetily-gui-real-qwen-full-playback.log`：全再生1回目。
- `meetily-gui-real-qwen-full-repeat.log`：全再生2回目。
- `meetily-gui-real-qwen-verified.log`：完成済float WAV観測＋全SQLite照合の最終real成功、続けてfixture通常回帰成功。
- `meetily-gui-real-qwen-fixture-regression-fixed.log`：fixture通常／exit7／metadata保存故障の3回帰成功。
- 先行 `meetily-gui-real-qwen-first.log`／`meetily-gui-real-qwen-final.log` は最初のASR requestで停止する版。特に後者は再生途中にStopしており、原稿全再生の確認として使わない。16.686／3.619秒を全入力での性能比較に使わない。
- `meetily-gui-real-qwen-fixture-regression.log` は試験スクリプトのfixture生成indent不備で失敗。textwrap.dedent＋生成sourceのcompile検査で修正。アプリ／実workerの欠陥ではない。

今回はGUI試験script/docsのみ更新。既存アプリを再buildして変えた成功ではない。Python compile／git diff check成功。音声／DB／model manifests／screenshots／result JSONはrepo外の隔離runtimeに保持。

## 再現

準備済model/worker/ORTは [QWEN3-ASR-SETUP.md](QWEN3-ASR-SETUP.md)、native binary/sidecar/bootstrapとdev serverは `frontend/tests/gui/README.md` を参照。prepared artifactsがなければこのtestはdownloadしない。

```sh
cd /home/user/Code/meeting-assistant-research/meetily
nix-shell shell.nix
python3 frontend/tests/gui/recording_native_smoke.py \
  --binary ../murmur/target/debug/meetily \
  --bootstrap ../murmur/target/debug/examples/gui_fixture_bootstrap \
  --driver "$(command -v WebKitWebDriver)" --espeak "$(command -v espeak-ng)" \
  --runtime ../runtime \
  --real-worker ../runtime/qwen3-asr/worker-target/release/qwen-speech-worker \
  --real-model ../runtime/qwen3-asr/artifacts/model-1.7B \
  --real-ort /nix/store/x46rnziynpalr4phm6yh0x46s42xq5kw-onnxruntime-1.27.1/lib/libonnxruntime.so
```

## 次の境界

長時間／連続burstでのqueue・メモリ・遅延／30秒分割／worker timeoutとhard kill／crash後tmp cleanup／再起動後の認識欠損badge／clean-host Linux packageとmodel導入UX／Windowsは未検証・未完。cloud/final minutesの意味確認や実会話は今回の許可に含めない。Linux MVP全体完成ではない。
