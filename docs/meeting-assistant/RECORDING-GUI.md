# Linux native録音フロー：合成GUI／障害回帰

## 範囲

`frontend/tests/gui/recording_native_smoke.py` で、専用Pulse serverの合成音声を実native pipelineから録音し、Homeの開始→停止→ファイル／SQLite保存→native再起動→Meeting Details再閲覧を確認。

- 新規HOME/XDG・専用D-Bus・analytics無効。Pulseは `-n` でnull sinkとprivate Unix protocolだけをloadし、sourceが `meetily_fixture.monitor` 一つであることを確認。ALSA defaultは存在しないcard999へ固定。hardware検出module・実mic・実desktop・既存Pulse serverは使用しない。
- eSpeakによる固定人工英文を、そのnull sinkへだけpaplayする。**本体の既存Silero VADは実行するが、ASR／LLMモデル推論はしない**。Qwen JSONL adapterの相手は固定応答fixture-workerで、model/ORT pathsは隔離された明示fake placeholders。実モデルの認識成功・導入成功・品質を意味しない。
- Start/Stopは実DOM `.click()`、設定準備は公開native commands。文字起こしeventを注入せず、本体VAD→Qwen adapter→テストworker→TranscriptSink→UI/SQLiteを通す。WebKitの物理座標クリックは別の未検証境界。
- worker応答を止めてStart後のrequestを確認。Stopで `get_meeting_folder_path == null`（global manager除去）になり、まだtranscript-updateがないことを確認してから応答を解放する。Stop後tailの保存をrace任せにしない。
- 保存audio.mp4をdecodeし、1秒超の有限・非zero音声を確認。transcripts.jsonの固定文、Stop payload、temporary input削除、worker PID終了を確認。SQLite Meeting Detailsへ移動した後でnative appを再起動し、同meeting再閲覧／元filesとcheckpointsのSHA256不変を確認。

## 三つのシナリオ

1. **通常**：Stop後tailがdisk/SQLiteに残り、`transcription_drained=true`／`save_succeeded=true`。再起動後も閲覧可能。
2. **worker-crash**：最初のrequestでfixture-workerをexit7。UIの継続警告が残り、native captureとStopは維持。新しい音声burstのrequestだけfresh PIDを起動し、Stop後にその応答を保存。失敗があったため `transcription_drained=false`（後続成功で過去の欠損を完了扱いしない）。`save_succeeded=true` はaudio/file保存の結果であって全文認識成功ではない。元の失敗音声を再送／補完しない。
3. **save-failure**：Stop前に元metadataをrepo外の同隔離profileへ退避し、metadata.jsonをdirectoryにしてcompletion atomic publicationを故障注入。`save_succeeded=false`／save_errorと **Recording files were not fully saved** の継続警告、audio checkpoint保持を確認。raw transcripts／audioは残る。**試験側で障害directoryを除去し元metadataをコピー復元**した後、保存済SQLite meetingを再起動で再閲覧。これは自動修復ではなく、disk-full全体／crash／power lossの証明でもない。

## 見つけた不具合と修正

- SettingsRepositoryがQwenのAPI key lookupを `Invalid provider: qwen` と拒否し、設定reload／native start validationが失敗。Qwenを既存Parakeetと同じlocal/no-key分岐へ追加。合成SQLite testでconfig roundtrip・Qwen key非保存・既存cloud key非変更・unknown provider拒否を確認。
- ALSAのphantom `default` micがCPALに存在するだけで選択され、usable inputがなくてもstream作成へ進んでsystem-only startを失敗させていた。default micのsupported input configsをpreflightし、なければ既存mic-unavailable/system-only経路へ進む。実マイク切断／Bluetooth／Windowsの受入試験ではない。
- RecordingControlsがactive ASR errorで `onRecordingStop(false)` を呼び、nativeは捕捉中なのにUIがpost-stopへ進みStop controlを隠していた。ASR失敗をcapture終了として扱わず、明示Stopまで録音を維持。useModalStateのactive errorをInfinity toastにし、欠損・再送なし・音声確認を説明する。関連async listenerの遅延登録cleanupも保護。
- icon-only Start/Pause/Resume/Stopにaria-labelを付け、DOM操作の対象を明確化。

## 結果・再現

三シナリオが全て成功し、Stop owner除去待ちを追加後も全て再成功。

- `../logs/meetily-gui-recording-tail-final.log`：最新3 GUI成功（過去 `meetily-gui-recording-final.log` も3成功）。
- `meetily-gui-recording-first.log`：Qwen config拒否の失敗。
- `meetily-gui-recording-second.log`：phantom mic start失敗。
- `meetily-gui-recording-worker-crash.log`：ASR failure後のStop control消失。
- `meetily-gui-recording-worker-crash-final.log`：警告が短時間toastのみで消失した失敗。persistent warning修正後成功。
- `meetily-qwen-settings-roundtrip.log`：追加native1＋native build成功。
- `meetily-gui-recording-native-regression.log`：transcription8／minutes17／recovery7／sink6／incremental5＝43passed、既存real Ollama1ignore。
- `meetily-gui-recording-frontend.log`：frontend24＋TypeScript成功。
- `meetily-recording-controls-errors.log`：独立hook harness2成功（errorでstop callbackなし、遅延listen cleanup）。React mockのためこのfileは独立Bun processで実行する。
- `meetily-gui-recording-production.log`：TypeScript／pnpm10 production build成功。Cargo/pnpm lock変更なし。

```sh
nix-shell shell.nix
export CARGO_TARGET_DIR=/home/user/Code/meeting-assistant-research/murmur/target
# staged sidecarsとlocalhost:3118 dev serverが必要。tests/gui/README.md参照。
for fault in none worker-crash save-failure; do
  python3 frontend/tests/gui/recording_native_smoke.py \
    --binary "$CARGO_TARGET_DIR/debug/meetily" \
    --bootstrap "$CARGO_TARGET_DIR/debug/examples/gui_fixture_bootstrap" \
    --driver "$(command -v WebKitWebDriver)" --espeak "$(command -v espeak-ng)" \
    --runtime ../runtime --fault "$fault" || break
done
# frontendから独立実行：
bun test tests/lib/recording-controls-errors.test.tsx
```

GUI artifacts／音声／DB／fake artifactsはrepo外の新規runtime profileだけに保持。専用server、playback、driverのowned process groupsをfinallyで終了する。production buildで.nextを置換した後はdev serverを再起動する。

## その後の実Qwen接続確認

ユーザー承認の続行で実worker／prepared1.7B weightsを直接使う `--real-*` modeを追加。全再生の合成英文でStop tail／event-disk-SQLite全segment一致／native再起動を3回確認。fixture側の3故障回帰も再実行成功。上記fixture回帰の範囲を実モデル品質合格へ読み替えない。詳細：[REAL-QWEN-GUI.md](REAL-QWEN-GUI.md)。

## 未検証の境界

実会議の実Qwen品質／長い30秒境界・精度／長時間・負荷欠損／real mic/desktop／cloud/final minutes意味品質／hard kill/power loss／corrupt DB repair／worker長timeoutの実GUI／clean-host package／Windowsは未検証。認識不完了の警告は今はUI toast／Stop-session状態であり、app再起動後の全会議一覧に欠損badgeを永続再表示する機能はない。失敗speechや未checkpoint RAMを復元できると主張しない。Linux MVP全体の完成チェックはまだ更新しない。
