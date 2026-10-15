# Linux system audio: native Pulse monitor

## 実装

`audio/capture/pulse.rs` は公開libpulse C APIでPulseAudio／PipeWire Pulse互換serverへ接続。`pactl` / `parec` / FFmpegは本体captureで使わない。既存Cargo/pnpm locks、mic/Windows CPAL、macOS backendは維持。

- source metadataの `monitor_of_sink != PA_INVALID_INDEX` を確認してmonitorだけをOutputとして列挙。device identityは `pulse-monitor:<exact-source-name>`。表示suffixをsource nameへ渡さない。
- `DeviceSelection` のSystem Audioでこのmonitorを選択。既存Default System Audioはdefault **sink**のmonitorへ解決し、default input/microphoneは使わない。serverが使えなければmic-onlyの既存動作が残るので、内部音声が必要な場合はmonitorを明示選択する。
- ALSA playback deviceをsystem capture候補に追加する旧discovery経路をLinuxで除外。既存ALSA monitor inputsは引き続き使用可能。
- `AudioStream::create` からnative threadへ接続し、mono48k float32を既存 `AudioCapture` → RecordingState channel → pipelineへ送る。録音とASRの後続経路は変更しない。
- owner threadだけがC pointers／mainloop／operation callbacksを操作。source/server callback userdataはRefCellでinterior mutableとし、operationをcancel/unrefしてから解放。public structのstable prefixだけを読む。ABI layout testあり。
- context／operation／stream startupは各5秒上限。nonblocking mainloopを5msごとにpollし、Stop/dropはstop flag＋thread joinで全native resourcesを解放。ready failureではcaptureを返さない。
- `PA_CONTEXT_NOAUTOSPAWN` でserver自動起動なし。`PA_STREAM_DONT_MOVE` で別sourceへ移動させない。Pulse holeは同じ長さの無音として扱い、nonfinite／不正bufferはエラー。
- server/stream failureとsource消失をエラー通知。PipeWireではsource削除後にstreamがREADYかつ無音のまま残ったため、1秒ごとにsource存在を確認する。失敗は既存RecordingState error callbackから `recording-error` として通知する。勝手なreconnectやsource fallbackはしない。元sourceを復旧／再選択して新規startする。

## 必要なruntime

Linux appは `libpulse.so.0` を必要とする。developmentにはlibpulse headers／link library、実行時にはPulseAudioまたはPipeWireのPulse serverが必要。Nix `shell.nix` は既存pulseaudio packageに加えlibraryをLD_LIBRARY_PATHへ追加した。Debian/Ubuntuなら通常 `libpulse-dev`（build）と `libpulse0`（runtime）。Tauri Debian bundleの追加依存に `libpulse0` を宣言。実deb/AppImage packaging・clean-host installは未検証。

## Synthetic-only検証

```sh
cd /home/user/Code/meeting-assistant-research/meetily
nix-shell shell.nix
export CARGO_TARGET_DIR=/home/user/Code/meeting-assistant-research/murmur/target
export MEETILY_DISABLE_ANALYTICS=1
cargo test --locked --offline -p meetily --lib audio::capture::pulse
cargo run --locked --offline -p meetily --example pulse_monitor_smoke
```

smoke exampleのみ `pactl` / `paplay` を使用する。unique-name `module-null-sink` を作り、そのmonitor以外を開かず、生成440Hz／0.1peak float32を **そのsinkへだけ** 再生する。default device変更・マイク／実desktop capture・ASR／cloudは行わない。raw fileはtempdir、moduleはRAII unloadで削除。

検証結果（このPipeWire 1.6.8環境）:

- native monitor discovery、default sink monitor introspection（captureなし）成功。
- `AudioStream` → `AudioCapture` → RecordingState channelでSystem/48k metadataと有限samplesを確認。
- 同一monitorをstart/stop2回、pauseでchunk送信停止、resume、1秒以内のstop成功。
- monitor削除をエラー検出し、消失sourceへのstartは拒否（fallbackなし）。同じ専用sinkを明示再作成し、再接続・tone取得成功。
- synthetic module cleanup成功。pulse2 tests、既存transcription7／sink3 tests、cargo check成功。
- 最初のsmokeはsource削除を検出できず失敗。周期source確認追加後に再実行して成功。失敗をsilence成功として扱わない。

logs: `../logs/meetily-pulse-check.log`, `meetily-pulse-synthetic-smoke.log`, `meetily-pulse-final.log`。

## native録音GUIへの接続確認

新しいprivate Pulse serverをnull sink＋private Unix protocolだけで起動し、eSpeak人工英文を専用sinkへ再生。本体capture→VAD→固定応答fixture-worker→Stop tail→audio／transcript／SQLite保存→native再起動／再閲覧が成功。worker crash／completion metadata失敗のGUIも確認。ALSAが存在だけ返すphantom default micはsupported input configsでpreflightし、system-only開始を妨げないよう修正。実ASR weights／LLM／mic／desktopなし。詳細：[RECORDING-GUI.md](RECORDING-GUI.md)。実モデルを通すend-to-end／長時間・server restart・配布版は未検証。

## 未検証／次工程

上記はsynthetic sourceでのcapture/channel lifecycle検証。GUI録音全体・mixed WAV保存・文字起こし＋議事録までのend-to-end、server自体の再起動、長時間・負荷時の欠損、実会議品質、clean-host package／Windows検証を意味しない。stream failure後の録音manager cleanup／recovery全体は `LINUX-MVP.md` の未完条件。音声入力のlevel previewは既存CPAL実装のため、native monitorの録音前previewは別途接続が必要。

次はStop tailを含む最終議事録と、ユーザー編集保護／出力・再閲覧・復旧を進める。実会話／実desktop／micの収録やcloud requestによる検証は引き続き別途許可が必要。
