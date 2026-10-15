# 日本語公開fixture＋隔離仮想音声での自動確認

## その後のPC正規化

PCの上限付き自動正規化が別途明示承認され、新候補の公開fixture元音量／−20dB／virtual dual−20dB確認も成功。最新候補・変更点・追加script argumentsは [PC-AUDIO-NORMALIZATION.md](PC-AUDIO-NORMALIZATION.md)。以下はその前のraw PC baseline記録で、比較用に保持する。

## 最新のユーザー承認

ユーザーは自分のmic録音ではなく「前にテストで使った音声」で試すよう指定し、使用中の実micを避けて仮想スピーカーを希望。decision gateで以下を明示選択：

1. 以前使用した日本語公開FLEURS fixture。
2. **隔離環境の仮想入力のみ**でagentによるStart／再生／Stop／保存監査を自動実行してよい。

この範囲に限って、以前の「操作はユーザーのみ」制約を変更する。使用中の実mic／実speaker／host desktop音／通常profile／private実mic録音／cloud・auth／releaseは許可しない。今後のこの診断ではユーザーの7.2秒mic録音を使わない。

## 実装・隔離

新script `scripts/test_public_virtual_audio.py`：

- sourceは以前の `../runtime/qwen3-asr/artifacts/fleurs-ja_jp-validation-0-1519.wav` に固定。11.1秒mono16k、SHA256 `725511728984c6eb4353a73aebe6cf77ed1914b1bd71e24a2dae517284e2d171`。30秒以内のguard。ユーザー音声を選ぶargument／globを持たない。
- fresh `public-virtual-*` HOME／XDG／SQLite／D-Bus／Xvfb。使用中のvisible appにattachしない。
- **private Pulse serverを `-n` で起動**。読み込むmoduleは専用Unix protocol＋null sinkだけ。`pactl`でsource名の完全一致とmodule allowlistを検証し、hardware moduleを使わない。
- PC-only：`fixture_pc.monitor`のみ。ALSAはinvalid card999に固定して実micをopenできなくする。micは最終metadataでもnullを要求。
- dual：別null sink `fixture_mic` を追加。private ALSA Pulse alias `fixture_mic` を **private socketのfixture_mic.monitor** に明示bind。そこへ45秒のPCM zeroをprivate paplayで流し、人工mic callbackを作る。mic発話／hardware captureはしない。PC側だけに公開日本語音声を流す。
- `paplay --device=fixture_pc` の終了を待ってStop。local Qwen1.7B decoder-INT8／ja、model／gain／VAD／resampler／capture DSPは変更しない。
- native `recording-stopped`後にfrontendのmeeting-details遷移を待ち、raw／events／SQLiteの本文一致を**ローカルbooleanのみ**で監査。発話text／titles／audio／native log linesをtoolへ出さない。non-content logにsegment count／保存音声duration・RMS／warning counts／cleanupを記録。
- cleanupは当該runがspawnしたprocess groupsだけ。userのreal micや他appのdevice設定／global Pulse defaultsを変更しない。

使用候補は `../runtime/nixos-dual-sync-candidate`（store `v907pl178by5zqbkl357fhjrhngr61fx-meetily-local-candidate`）。**既知のtimeout後degraded復帰バグはまだこのcandidateに残る**。

## 結果

### PC-only

- profile `../runtime/public-virtual-bcee4lba`、DISPLAY:113／port4463。
- 11.1秒公開fixtureを全再生、保存音声11.792875秒、ASR queued2／saved2 nonempty。
- completion／save succeeded／drained／event本文＝raw＝SQLite／selected virtual PC＋mic null／decode warning0／temporary input削除／checkpoint cleanupすべて正常。
- stall0／late discard0、error events0、helpers終了確認、`passed=true`。
- mixed保存AAC decoded16k RMS0.054769。元ASR入力のbit exact／歪みなし／単語精度の証明ではない。

### dual（仮想micは無音）

- profile `../runtime/public-virtual-6j9vzk2j`、DISPLAY:114／port4464。
- 同じ公開fixtureを全再生、保存音声12.020375秒、ASR queued3／saved3 nonempty。
- 両private virtual device保存、completion／save／drained／event・raw・SQLite本文一致、decode warning0、temporary input／checkpoint cleanup正常。
- stall0／late discard0、error events0、helpers終了確認、`passed=true`。
- 保存AAC decoded16k RMS0.054516。

log：`../logs/meetily-public-virtual-single.log`／`meetily-public-virtual-dual.log`。各profileの `public-virtual-result.json`もaggregate／boolのみ。

**公開clipの文字起こしと保存が両routeで動いたという限定確認。意味精度／CER／実mic＋AEC＋相手voice／同時発話／長時間／restartは未測定。** segment数2対3は同じsegmentationを保証しない。実機testでの空ASRや既知degraded復帰バグの因果を否定しない。これら2runsではバグを誘発するcallback順にならなかった。

## 初回失敗も保持

- PC-onlyの最初の2runsは、hardware禁止ALSAで`get_audio_devices`（CPAL enumeration）を呼んで失敗。native録音前に停止。既存recording_native_smoke同様にsystem-onlyではexplicit Pulse monitorをStart時に解決し、ALSAを有効にして回避しない。private Pulse module検証＋最終metadata mic null確認は維持。
- 次のPC-only runはASR2／audio保存正常だが、native stopped直後にSQLiteを読み、frontend meeting登録前にharnessが終了した。停止eventとDB登録完了は別boundary。meeting-detailsまで待つharness修正で最終監査成功。失敗runのnativeファイルは保持。
- dual初回はprivate ALSAのnamehint／ctl.hw定義不足でdevice enumeration失敗。実micへfallbackせず、既存private mic launcherのhint構成を使ってprivate aliasを列挙し、最終run成功。

失敗log：`meetily-public-virtual-single-initial.log`／`-enumeration-failure.log`／`-save-race.log`、`meetily-public-virtual-dual-enumeration-failure.log`。これらをアプリ認識精度の失敗／最終成功として混同しない。

## 再現

repoの `nix-shell shell.nix`（python3／pulseaudio／ffmpeg／Xvfb／D-Bus）から：

```sh
python3 scripts/test_public_virtual_audio.py \
  --binary ../runtime/nixos-dual-sync-candidate/bin/meetily \
  --bootstrap ../murmur/target/debug/examples/gui_fixture_bootstrap \
  --driver /nix/store/iv6vjwxnifbi9zcwpglh5pd8p8mwj5w6-webkitgtk-2.52.6+abi=4.1/bin/WebKitWebDriver \
  --worker ../runtime/nixos-dual-sync-candidate/libexec/qwen-speech-worker \
  --model ../runtime/qwen3-asr/artifacts/model-1.7B \
  --ort /nix/store/x46rnziynpalr4phm6yh0x46s42xq5kw-onnxruntime-1.27.1/lib/libonnxruntime.so \
  --alsa-plugin-dir /nix/store/pkiq4qah654jaja30iwk3cqx6spfgiqb-alsa-plugins-1.2.12/lib/alsa-lib \
  --display :113 --port 4463
```

dualは `--dual --display :114 --port 4464`。occupied display／portのexistingプロセスへattach／killしない。Nix store pathはこのhostのもの。

次の検証もこの公開fixture＋private仮想音声を使う。復帰bug修正／人工の遅延到着順回帰／必要ならprivate仮想入力へのdelay注入を優先し、ユーザーに実mic録音や大音量speaker再生を要求しない。処理方式／モデル／gain／VAD／releaseの新しい変更は別decision gateを通す。
