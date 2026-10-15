# Linux：10分の合成録音とNixOSローカル候補

ユーザーはvisible合成プレビューに「問題なさそう」と報告。その後、長時間／Linux配布確認の続行を承認し、配布対象は **まず手元のNixOS** を明示選択。これは限定操作のユーザー報告であり、全項目の実測・自然会議品質受入ではない。実mic/desktop/会話・cloud/LLM/auth、モデル/default/VAD/gain/補正変更は行わない。

## その後の修正（最新）

ユーザーが診断／修正を承認し、float32 WAV checkpointへ変更を明示選択。AAC copy-concatがPCM余剰を作ることを確認し、新録音はlossless float32 WAV→最終AAC一回へ修正。native境界比較／10分実Qwen／厳密decode警告0／NixOS候補の複数checkpoint回帰まで成功。旧AAC録音は変更せず、legacy復旧はpartialの注意を返す。詳細：[CHECKPOINT-TIMING.md](CHECKPOINT-TIMING.md)。以下の旧14警告・store path・数値は修正前の証跡として保持する。現在のGC-rootは更新済み候補を指す。

## 10分録音（開発native、本物Qwen）

`recording_native_smoke.py --soak-seconds 600` を追加。private Pulse/eSpeak英文の全再生と5秒休止を繰り返し、public録音／ASR失敗events、native/worker RSS、saved segments、private temporary WAV数を観測。ASRの文は固定／補正／採点しない。終了は最後の再生完了を待つため600秒を少し超える。decode検査はarray('f')へ変更し、長いPCMをPython float tupleへ展開する大量メモリ消費を避ける。

結果：`../runtime/recording-gui-775ach4q`、log `../logs/meetily-gui-soak-600.log`。

- 602.498秒／全再生40bursts、57raw segmentsを保存。event/raw disk/SQLite全text一致、未知confidence=-1。Stop tailはmanager除去後にも到着。
- worker PIDは1つのみ。private temporary input WAVは観測最大1（**処理queue長そのものではない**）。public transcription/recording error・save/transcription warning eventsなし。無効ALSA discoveryのexpected errorログは別に存在。
- Stop→recording-stoppedまで4.351秒。recognition/drain/saveを含む一観測値で一般の性能保証ではない。
- 保存音声をdecodeし、有限／nonzero／elapsed duration以上を確認。temporary WAV削除、worker終了、native再起動後の全SQLite ID/text/audio times・folder全files/checkpoints hash不変を確認。
- 635memory samples（各burst終了時も採取）。native RSS head60–120秒median368254KiB→最後60秒393504KiB、peak399000KiB。worker7011912→7011928KiB、peak7597680KiB。nativeは約25MiB増えた。worker plateauはほぼ一定だが、これだけでleakなしとはしない。
- RSSはnative executableとworkerだけ。WebKit/driver/helper・unique RAM・PSSは含めない。メモリ予算／多時間の合格基準ではない。

`soak_monitor.py` とparser／plateau／missing-nativeのunit3tests成功。`soak-samples.jsonl`／`soak-summary.json`／`real-asr-result.json` はrepo外profileに保持。

### 音声timestamp警告は残す

長いaudio.mp4から16k raw PCMへのFFmpeg decode出力muxerで14件のnon-monotonically-increasing DTS警告が出た。成功exitだけで警告を隠さない。追加read-only ffprobe auditでは保存MP4の28302packetsにPTS/DTS非増加0、packet duration重なり0、firstPTS0だった。log `../logs/meetily-soak-media-audit.log`、profile `media-audit.json`。

**元MP4のtimestamp破損と断定しないが、decode/resample時の警告の原因・checkpoint境界のsample連続性は未解決。** 原音やcodecを推測で書き換え／再encodeせず保持した。今回のpipeline smokeは成功だが、完全な長時間音声受入ではない。

## NixOSローカル候補（embedded frontend）

配布preflightでmain/helper/workerがNix ELF interpreter/RPATHを参照することを確認。そのままdeb/AppImageへ入れて一般Linuxで動くとは主張しない。log `../logs/meetily-linux-package-preflight.log`。ユーザー選択に従い、既存production `frontend/out` をembeddedするTauri custom-protocol buildを作成（**debug profile、release最適化版ではない**）。frontend rebuildは行わず、ユーザーが開いているdev server/.nextを壊さない。開発binaryは退避→artifact入力へcopy後に元を復元。

一回限りのbuild overlayで `beforeBuildCommand=""`、`createUpdaterArtifacts=false`、updater endpoints=[]。元tauri.conf/default/providerは変更しない。上流公式updateでこのローカル候補を置換しない。署名／外部配布の仕組みを完成したものではない。

`scripts/nixos-local.nix` は以下をNix storeへstageし、runtime closureとGC rootを作る：

- embedded-native、隣接FFmpeg/helper、templates、optional real Qwen worker。
- wrapperのPulse/ALSA/OpenSSL/appindicator等。raw copied ELFのRPATH参照は自動保持されないため、**LP64 OpenBLAS（blas64=false）とORTを明示closureへ追加**。ILP64を代用しない。
- optional private synthetic-preview launcher、Python／WebDriver／Pulse／D-Bus／eSpeakとmigration bootstrap。ユーザーがStartするまで録音しない。previewのnavigationは現在のoriginを使い、localhost:3118へ戻さない。
- モデル／credentials／音声／個人DBは同梱しない。Qwen modelとORT pathは明示指定、normal appのASR defaultsは変えない。

初回版のGC-root link（その後更新）：`../runtime/nixos-local-candidate` → `/nix/store/szlzn38sq780rrqkxv5972lvdxbb43ff-meetily-local-candidate`。closure3305968016bytes（約3.08GiB、debug native/bootstrapとWebKit等を含む）。小さいrelease installerではない。NixOS現ホスト用で、nixpkgs pin・別NixOS host／CPU・新規OSの検証ではない。

### 検証

`--clean-native-env` でapp起動時の開発LD_LIBRARY_PATH/GTK/GI/GSettings/font/XDG_DATA_DIRSを除去し、PATHはhost systemだけ。candidate wrapper自身がruntimeを供給。`--expected-origin tauri://localhost` でembedded UIを必須にして確認：

- 人工minutes：再登録→編集revision2→native再起動→JSON/Markdown実download、元raw不変。
- 実Qwen合成録音：Start/Stop→manager除去後tail→全event/raw/SQLite一致→worker/tmp cleanup→native再起動とhash／SQLite不変。
- 最終closure修正後の両回帰成功：`../logs/meetily-nixos-local-final-regression.log`。
- packaged previewを `nix-shell` なし、開発library環境なしでheadless :110に直接起動し、人工minutes表示／READYを確認。録音は開始せずowned helperとXvfbだけ終了。log `meetily-nixos-packaged-preview.log`（closure修正前の同一native/worker binaries）、最終版のblank-environment CLI helpも成功。user-visibleの既存previewは閉じていない。
- この試験はNixOS現ホストのstore依存を使う。host dev serverは利用者用に残したが、candidateのUI originはembeddedであり開発URLではない。オフライン機全体・第三者clean hostを再現した試験ではない。

build log `meetily-nixos-standalone-build.log`、Nix log `meetily-nixos-local-package-closure-fixed.log`。app/worker SHA256はprofile manifestsとbuild inputsに保持。Cargo/pnpm lock変更なし。

## 手元で同じ合成確認を開く（nix-shell／Next不要）

```sh
cd /home/user/Code/meeting-assistant-research
runtime/nixos-local-candidate/bin/meetily-synthetic-preview \
  --runtime "$PWD/runtime" \
  --model "$PWD/runtime/qwen3-asr/artifacts/model-1.7B" \
  --ort /nix/store/x46rnziynpalr4phm6yh0x46s42xq5kw-onnxruntime-1.27.1/lib/libonnxruntime.so \
  --port 4450
```

新しい隔離profileを作り、人工minutesを表示。ユーザーがHome→Startした後だけ合成英文を一度private sinkへ再生（speakerには聞こえない）。Stop／保存完了を待って閉じる。再開時は表示されたprofileを `--profile` で指定。モデルはdownloadしない。normal `bin/meetily` は通常のuser profileを使うため、現在の「合成のみ」確認には上のlauncherを使う。

## 再stageの要点

repo Nix shellで事前にdev binaryを保全してから：

```sh
export CARGO_TARGET_DIR=/home/user/Code/meeting-assistant-research/murmur/target
cd frontend
pnpm exec tauri build --debug --no-bundle --ci \
  --config '{"build":{"beforeBuildCommand":""},"bundle":{"createUpdaterArtifacts":false},"plugins":{"updater":{"endpoints":[]}}}' \
  -- --locked --offline
# production binary/sidecars/templates/optional worker/bootstrapを新規inputsへcopy。
# dev binaryを保全した場合は元へ復元し、稼働中user appを止めない。
cd ..
nix-build scripts/nixos-local.nix --arg inputs /absolute/path/to/inputs \
  --out-link /absolute/path/to/local-candidate
```

`inputs/meetily` がdevUrl版ではないことを必ずexpected-origin回帰で確認する。FFmpegとモデル／third-party license確認前に再配布しない。

## 未完

decode timestamp警告／sample境界、数時間・連続30秒超ASR burst／resource limits、hard kill/power loss・破損DB修復、restart後認識欠損badge、release-size/optimized build、clean-host model導入、nixpkgs pin／一般Linux・Windows、署名・update設計と再配布license。完成条件全体をチェックしない。
