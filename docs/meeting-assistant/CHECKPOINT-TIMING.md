# 録音checkpointの継ぎ目：float32 WAVへ修正

ユーザーが時刻警告の切り分け／必要な修正と録音回帰を承認。診断後、保存方式の選択で **float32 WAV checkpointへ変更** を明示選択。中間容量増加を受け入れ、最終audio.mp4とASR入力／モデル／defaultを維持。実mic/desktop/会話・cloud/LLM/authや精度追試は行わない。

## 根因と旧方式の証跡

独立の合成信号（mono48k、長さ1440000／1440240／11777samples）を、実同梱FFmpeg7.0.2のAAC192k／AAC-LC／MP4 checkpointとして生成し、旧concat-demuxer＋`-c copy`で結合。

FFmpeg9.0.1 decodeでは個別checkpoint合計2892000samples、結合2896368samples（**4368追加、91ms**）。一意な波形の内部byte列から各chunk位置を測ると、最初のchunkにも1024samplesの先行paddingが追加。単なるlog表示の問題ではない。AACのcheckpointごとのencoder priming/paddingを一つのMP4へstream-copyしても、個別のgapless時間指定を保持できない。

また元11777samplesの端数tailが個別decodeで11760になった。MP4 movie edit-listの既定1000Hz時刻単位によるsub-ms丸めも別に存在。元sample数2892017との差17を隠さない。

log `../logs/meetily-aac-boundary-first.log`、artifacts `../runtime/aac-boundaries-hu7rdt6e`。その前の10分保存MP4にpacket PTS/DTS非増加0だったことも撤回しない：packet時刻が増加していてもcheckpoint間のPCM余剰を否定できない。

## 実装修正

- `audio/checkpoint_audio.rs`：IEEE float32 mono WAV（fact sample count含む）、元floatの全bitを保持。追加PCM16/24変換、gain、clipping、ASR入力加工なし。create-new／RIFF size bounds。
- `IncrementalAudioSaver`：30秒単位の `audio_chunk_NNN.wav` をprivate tempdirへ完全write→flush/fsync→hard-link no-clobber→directory syncでpublish。失敗時はbuffer／既存checkpointを保持。
- final merge：lossless WAVをconcat、sample countに基づく連続PTS（`asetpts=N/SR/TB`）、**最終AAC192k／AAC-LCを一回だけencode**、`+faststart`。従来のfinal temporary／fsync／atomic publishとmetadata commit後だけcleanupを維持。
- `encode_single_audio` とfinal mergeのMP4 movie timescaleをsample_rateへ設定し、端数sampleのedit-list丸めを避ける。
- recovery/discovery/has-audioはnew WAVとlegacy MP4を認識。数字indexのgaps／duplicates／codec混在は拒否し原filesを保持。
- legacy MP4のみの**明示**復旧は旧stream-copy互換を維持するが、status=`partial`／padding-timingが残り得るmessageを返す。既存audio.mp4は再encode／置換しない。新WAV復旧はAAC一回でoriginal WAVを保持。保存済み録音の自動変換や旧失敗の修復を主張しない。

一時容量はmono48k float32で約5.76MB/30秒、約11.52MB/分（final圧縮中は一時outputも必要）。停止時encodeのCPU／待ち時間が増える。失敗時のcheckpoint保持はdisk spaceを使い続ける。最終AACは従来通り非可逆であり、最終音声が元PCMとbit同一とは言わない。

## 境界回帰

新native example `checkpoint_boundary_smoke.rs` が実Saverへ同じ3サイズの合成PCMを送る。全WAV dataとsource.f32がbyte同一。final AACを、全sourceを直接一度encodeした参照AACと比較：

- 同梱FFmpeg7 decode：48k2892800samples／16k964267samples。
- 現Nix FFmpeg9 decode：48k**2892017samples（元と一致）**／16k964006samples。
- 両version・両rateでfinalとsingle-encode参照の**全decoded bytes一致**、warning0。

FFmpeg7は一つのAACの末尾paddingを783samplesまで返すためFFmpeg9と数が違う。これをcheckpoint重複や音質改善と混同しない。新方式が追加のcheckpoint delay/gapsを作らず、一回AAC encode相当になることを検証した。decoder差を丸めて同一とはしていない。

logs `../logs/meetily-wav-boundary-reference.log`／`meetily-wav-boundary-verification.log`。artifacts `../runtime/wav-boundary-native-final`（WAV／source.f32／reference／verification JSON）。repoに音声を追加しない。

Native tests：float bits/no-overwrite/parser2、incremental既存5＋boundary/recovery4、sink保存6、read-only recovery8＝**25 passed**。旧AAC保存済みfile保全／partial、WAV復旧／再実行non-clobber、mixed/duplicate拒否、WAV/MP4 scan counts・partial temp除外・byte不変を含む。logs `meetily-wav-checkpoint-native-final.log`（sink filterは0testで、後続reference logの6testを使用）、`meetily-wav-recovery-scan-final.log`。初回generic castのtest compile不備はLE bytes比較へ修正（`meetily-wav-checkpoint-native-first.log`）、upstream欠陥ではない。

## native GUI／長時間再試験

- fixture通常／worker exit7/fresh-worker／completion metadata故障→checkpoint保全の3回帰成功。新WAV checkpointを要求。log `../logs/meetily-wav-checkpoint-gui-regression.log`。
- 実Qwen、private Pulse/eSpeakのみの602.585秒／40complete bursts／47raw segments：Stop tail→全event/disk/SQLite一致→native restart／SQLite ID/text/timesと全folder hashes不変。worker同PID、temporary WAV最大1。Stop待ち9.923秒（load/recognition/drain/final encode/saveを含む一観測値）。保存PCM603秒。
- long decodeのFFmpeg9 warning-level audit：16k9648000samples、**警告0**。旧14警告を黙って抑制せず、新生成fileで消えたことを確認。rawASRは補正・scoreしない。segment countの旧57→今回47は認識／VAD timing依存で品質向上とは言わない。
- 長いrunのnative RSS head60–120秒median367380KiB→最後60秒391116KiB、worker6984724→6975948KiB。native約23MiB増、workerほぼplateau。WebKit/unique-RAM未計測、leak-free受入ではない。
- GUI harnessはdecodeをwarning-levelで実行し、**stderrが一文字でもあれば失敗**する回帰gateを追加。

logs `meetily-wav-checkpoint-soak-600.log`／`meetily-wav-strict-decode-regression.log`、profile `../runtime/recording-gui-viodoopp`。

## NixOS候補も更新

custom-protocol **debug**候補を再build／stageし、開発binaryを復元、ユーザーが開いているapp／dev server/.nextは止めない。GC-root `../runtime/nixos-local-candidate` は現在 `/nix/store/qbbn2prbvil8xknalmkiqi2x0dkplwl8-meetily-local-candidate`、closure3305606888bytes。

最終候補でclean native env／tauri://localhost必須、実Qwen・5bursts／71秒の複数WAV checkpoint→Stop/save/restartを確認、warning-level decode0。log `meetily-wav-nixos-real-regression-fixed.log`。最初のpackaged soakはmonitorがshell wrapperをnative exeと誤比較しRSS0で失敗（`meetily-wav-nixos-real-regression.log`）。owned-group制限を維持してbin→libexec companionを識別し、unit4tests／同候補再実行成功。アプリの録音エラーではないが、そのrunを成功扱いしない。

起動commandは [LINUX-STABILITY-PACKAGING.md](LINUX-STABILITY-PACKAGING.md) と同じ。新録音はWAV方式。旧MP4 recording自体・credentials/models/default/VAD/correction/locksは変更しない。

## 再現と未完

```sh
# repo Nix shell; devices/network/modelを使わない境界試験
export CARGO_TARGET_DIR=/home/user/Code/meeting-assistant-research/murmur/target
export PATH="$CARGO_TARGET_DIR/debug:$PATH" # 同梱FFmpeg7がstage済み
cargo test --locked --offline -p meetily --lib audio::incremental_saver:: -- --test-threads=1
cargo build --locked --offline -p meetily --example checkpoint_boundary_smoke
"$CARGO_TARGET_DIR/debug/examples/checkpoint_boundary_smoke" /new/synthetic/output/directory
```

今回の1・2は接続／保存の修正・回帰であり、自然会議品質／全decoder・全OSの受入ではない。legacy AACのpaddingを自動修復しない。数時間／disk exhaustion／hard kill/power loss／破損DB・clean-host model導入／release最適化・配布licenseはまだ未完。次は手元NixOSのrelease版と開発環境なしでの操作確認（別工程）。
