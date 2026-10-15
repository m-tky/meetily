# 実マイク確認：EchoCancel_Mic／日本語（短時間保存を確認）

最新の指示：**まだreleaseは作らない。micで使えるかを確認する。** ユーザーは入力としてOS既定の `EchoCancel_Mic`、言語として日本語を明示選択。限定的なテスト発話のマイク確認を許可したもので、実会議／周囲の会話／desktop音声／cloud／ログインの許可へ拡張しない。自動Startは行わず、ユーザー自身がStart→発話→Stopする。

## ユーザー実行後の確認（最新）

ユーザーがStart→日本語テスト発話→Stopを実行し「できた」と報告。実音声／発話textをtoolで読み上げず、`scripts/audit_microphone_save.py` が同じ承認済みprofileだけをread-onlyで調べ、non-content数値／一致boolを出力：

- completion metadata committed、microphone alias保存、system audio=null。
- audio.mp4 168576bytes、mono48kHz、保存／decodeとも7.2秒、有限／nonzero。FFmpeg warning-level stderr0。
- raw transcript1segment、非空。SQLite folder binding1つ、全textとaudio_start/end/durationがraw diskと一致。
- private ASR input削除、checkpoint正常cleanup完了。
- 認識文の内容や正誤は評価／表示しない。event-drained flag／restart-reopenの新実測ではない。

log `../logs/meetily-mic-save-audit.log`、profile `mic-save-audit.json`。`passed=true` はこの限定storage/capture audit。選択した実mic→ローカルQwen→Stop→audio/raw/SQLite保存まで短時間で動作した確認であり、自然会議・全mic・長時間・意味品質の合格ではない。

参考：AAC decode peak1.321069、RMS0.129405。float decode peak>1はcodec overshoot等でも生じるため、元mic入力がclipしたと断定せず、音質良好とも判定しない。gain/VAD/correctionを変更して数値を隠さない。音声内容はローカルに保持。

release/build/packageは追加していない。

## 準備

`scripts/preview_microphone.py` を追加し、既存のWAV checkpoint修正済みNixOS **debug候補**をそのまま使用。release build／新package stageはしていない。新規HOME/XDG/DB、private D-Bus、analytics無効、資格情報コピーなし。実Qwen1.7B decoder-INT8、明示ja、prepared model/ORTをそのprofileだけに設定。合成発話・人工transcript・人工minutes・自動録音は一切追加しない。

今回のmicrophone routeは、既存本体のCPAL→ALSA入力経路を使う：

- private ALSA configに `default` と `meetily_mic` の二つのinput aliasだけを用意。ALSA Pulse pluginに**host Unix socketとEchoCancel_Micを明示**し、他sourceへdefault fallbackしない。物理cardの直接録音は設定しない。
- native libpulseのsystem monitor discovery/capture用 `PULSE_SERVER` は**存在しないprivate socket**へ向ける。ALSA pluginのexplicit serverとは分離し、hostのspeaker/HDMI/Bluetooth/desktop monitorを選べない。
- stock `ctl.hw` の最小control metadata定義とnamehint defaultsを追加。card名の列挙は音声録音ではない。他のPCM hardware definitions／user ALSA configは読み込まない。
- 実機のOS設定／EchoCancel volume・AEC設定は変更しない。マイクは当然周囲の音を拾い得るので、静かな場所でテスト発話を行う。

ユーザー画面 `DISPLAY=:0` に開いたwindow title：**Meetily — マイク確認 / EchoCancel_Mic / 日本語**。

profile：`../runtime/mic-preview-a6j9i_qd`、helper log `../logs/meetily-mic-preview-ready.log`。native device enumerationはInput `default`／`meetily_mic` のみ、Outputなし（profile `input-routing.json`）。READY時のis_recording=falseと、当該native PIDのhost Pulse source-outputが0であることをmetadataだけで確認。自動Start／実音声保存をしていない。録音開始後のusable samples／認識／保存は**まだ未確認**で、ユーザー発話を待つ。

## 操作

1. 上記「マイク確認」のwindowで **Start recording**。
2. 例：「今日はマイクの録音と文字起こしのテストです。」と日本語で話す。
3. **Stop**し、認識・保存の完了を待つ。初回model load／final encodeには待ち時間がある。
4. エラー表示／文字起こし／保存済みmeetingを確認。成功でも自然会議の品質合格とはしない。Stop完了を待ってから閉じる。

実マイクデータはrepo外・owner-only profileに保持。**音声file・発話text・native logをtoolでそのまま読み上げ／cloud contextへ送らない。** 必要な確認はユーザー報告と、ローカルで計算したduration／nonzero／segment数／event-file-SQLite一致のboolなど非内容metricsだけに限定する。ユーザーが別途明示しない限り、実発話のAI議事録生成／cloud送信や自動uploadは行わない。

## 再開用（既存候補、release不要）

repo Nix shell内で：

```sh
python3 scripts/preview_microphone.py \
  --binary ../runtime/nixos-local-candidate/bin/meetily \
  --bootstrap ../murmur/target/debug/examples/gui_fixture_bootstrap \
  --driver "$(command -v WebKitWebDriver)" --runtime ../runtime \
  --worker ../runtime/nixos-local-candidate/libexec/qwen-speech-worker \
  --model ../runtime/qwen3-asr/artifacts/model-1.7B \
  --ort /nix/store/x46rnziynpalr4phm6yh0x46s42xq5kw-onnxruntime-1.27.1/lib/libonnxruntime.so \
  --alsa-plugin-dir /nix/store/pkiq4qah654jaja30iwk3cqx6spfgiqb-alsa-plugins-1.2.12/lib/alsa-lib \
  --pulse-server unix:/run/user/1000/pulse/native --mic-source EchoCancel_Mic \
  --language ja --display :0
# Stop／保存してwindowを閉じた後のみ --profile ../runtime/mic-preview-a6j9i_qd を追加して再開。
```

bootstrapはschema migrationのみ。必要なsynthetic markerを一時作成し、native app起動前に除去する。実mic profileをsynthetic-only artifactとして扱わない。

初回準備はwrapper command名の誤り（list_audio_devicesではなくget_audio_devices）で失敗、次は最小ALSA configにctl.hw/namehint defaultsがなくenumeration失敗。両方script側で修正しREADY確認。前者を本体mic故障、後者を録音成功として扱わない。logs `meetily-mic-preview.log`／`meetily-mic-preview-fixed.log`も保持。既存synthetic visible appは閉じていない。

## 未完

実mic保存データのnative restart/reopen、mic reconnect、直接USB mic、長時間実mic、レベル／clipの入力側診断、他OS、品質・release完成。今回の7.2秒capture/storage auditだけでそれらを証明しない。
