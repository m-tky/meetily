# Linux実用版：精度研究から製品完成へ

## 最新のユーザー決定

「いったん、精度はいいので、完成形にするべきでは？」に対して、ask_userで **「MeetilyのLinux実用版を完成（推奨）」** を明示選択。

これまでの「品質受入まで本体統合を保留」を、**Linux・選択式Qwenの実用版実装を許可する**方針に変更する。品質の合格を意味しない。追加のASR精度比較・holdout実験は一旦止め、動作する製品フローを優先する。既存Whisper/Parakeetを残し、無断でdefault/provider/modelを切り替えない。

## 完成条件（まだ未達）

- [x] 選択式1.7B decoder-INT8 Qwenを本体の音声入力・表示・保存に接続。float32 mono16k入力、導入／起動失敗UI、private sink合成英文→実weights推論→Stop tail保存→SQLite／native再起動の接続確認済み。精度受入・全言語・長時間・clean-host導入の保証ではない。
- [x] Linux native PulseAudio/PipeWire互換monitor入力を実装し、専用synthetic sinkで取得／stop／source削除／明示reconnectを確認。mic/Windowsの既存CPAL経路を維持する。実録音全体・server restart・配布版は未検証。
- [x] 録音中の継続議事録と、停止後の末尾を含む最終更新を接続。録音停止はLLM成功を待たない。synthetic orchestration／制御テスト成功。実GUI全体・意味品質・長時間incrementalは未検証。
- [x] raw transcript・根拠・草案・変更／ユーザー編集履歴を保持。編集をAI応答検証前／commit時に保護、revision conflictは拒否。保存済meeting ID経由の再閲覧とJSON/Markdown出力を接続しsynthetic tests成功。合成GUIの明示再登録→編集→再起動→実JSON/Markdown downloadは確認済み（DOMボタン操作）。録音GUI全体は未検証。
- [ ] worker timeout/crash/restart、保存失敗・再起動の復旧を、失敗を隠さず扱う。
- [ ] インストール／起動手順を整え、公開fixture・専用synthetic sinkで一連の操作を確認する。最低限の欠損・停止・再起動回帰は行うが、精度最適化の追試に戻らない。

Windows完成・自動ASR補正は今回の選択範囲外。音声はローカル、文字起こし／議事録のcloud送信はUIで明示選択した場合のみ。実会話／実desktop／microphoneの収録は別途許可が必要。未検証の話者を人名と断定しない。既知の短語・否定誤認識は撤回せず、AI生成物を自動確定会議録にしない。

## 実装開始：停止時のtranscript保存

本体を確認したところ、停止時にglobal RecordingManagerをtakeし、さらにtranscript-update listenerを解除してから残りASRを待っていた。旧listenerはglobal managerを参照して保存するため、stop tailの発言がbackendのtranscripts.jsonに反映されない経路があった。startup時もtask開始後にlistenerを登録していた。

修正：

- `RecordingSaver::TranscriptSink`をsessionのsegments＋folderに固定。startupでglobal manager公開**前**に捕捉しworkerへ渡す。
- workerがUI event発行**前**に直接upsert／incremental保存する。UI listenerの有無やStopのglobal manager移動に依存しない。
- 古いworkerの遅延結果が新しい会議folderへ保存されない。
- upsertとdisk snapshotを同じlockで直列化。unique tempfile＋fsync＋atomic replacement、Unix directory syncを使用し、古いsnapshotによる上書きを防ぐ。
- disk保存失敗はwarningを表示し、正常lock下ではsegmentをmemoryに保持して次回保存で再試行できる。
- 旧event listenerによる二重保存を削除。transcripts.jsonの既存version1.0/schemaとsegment IDsを維持。

3synthetic tests（owner除去後のtail／別sessionへの非混入、upsert・atomic output、disk失敗後のmemory保持・再保存）が成功。録音・再生・ASR/LLM推論・cloud requestなし。これは低水準保存回帰であり、録音停止全体／最終議事録／crash復旧の完成を意味しない。

Qwen providerの選択／worker接続は下記まで実装。Linux monitor captureとsynthetic sink試験、最終議事録接続も実装・synthetic tests成功（下記）。編集保護・出力・再閲覧も接続・synthetic tests成功（下記）。次は保存失敗・再起動復旧／全体操作。長時間の増分議事録、停止timeout後のtask lifecycle、保存失敗後の再閲覧／再起動復旧等は引き続き完成条件に含まれる。旧benchmarksと失敗証跡は保持し、採点を変更しない。

検証：新3tests＋既存worker2tests＋議事録6tests成功（実Ollama1testは既存ignoreのまま）。全native backendのcargo checkとgit diff check成功。ログ：`../logs/meetily-linux-mvp-transcript-sink.log`、`meetily-linux-mvp-first-check.log`。依存lockfile変更なし、既存pnpm10/Nix shell、offline/locked Cargo、analytics無効、外部targetを使用。

## 継ぎ目のPCM余剰を修正（新録音）

ユーザー選択でfloat32 WAV checkpointへ変更。旧AAC copy-concatのpriming/padding余剰を合成試験で確認し、新録音は元float bit保持→sample PTS→最終AAC一回／MP4 sample timescaleへ修正。3checkpointはsingle-encode参照と全decoded bytes一致・warning0。実Qwen10分Stop/save/restartとupdated NixOS候補71秒／strict decode回帰も成功。既存MP4は不変、legacy復旧はpartial注意、混在／duplicate／gapはaudio merge拒否・原files保持。容量／Stop encodeの費用は増える。release／多時間／clean hostは未達。[CHECKPOINT-TIMING.md](CHECKPOINT-TIMING.md)。

## 10分／NixOS standalone候補（部分確認）

ユーザーの操作確認「問題なさそう」の後、続行／まずNixOS向けを明示承認。実Qwen・40合成burstsで602.498秒の録音→57raw segments→Stop tail／保存→native restartまで確認。worker同PID、temporary input最大1。worker RSS plateauはほぼ一定、nativeは約25MiB増加（leak不在の保証ではない）。decode DTS警告14件は未解決、元MP4のpacket timestamp auditでは非増加／重なり0。

embedded frontendのローカルNix候補をstageし、開発library環境を除去、tauri://localhost origin必須で編集・exports・restartと実Qwen録音・restartを確認。nix-shell／Nextなしのsynthetic launcherも用意。debug profile／external model／同NixOS host限定で、release／clean host／一般Linux配布は未完。全体完成条件はまだ未達。[LINUX-STABILITY-PACKAGING.md](LINUX-STABILITY-PACKAGING.md)。

## 実Qwenを使う合成native録音GUI（接続確認）

private Pulse/eSpeak固定英文の全再生を実1.7B decoder-INT8へ直接渡し、pending推論中Stop→manager除去後tail→raw event/disk/SQLite全segment一致→native再起動／再閲覧を隔離profile3回で確認。worker終了／temporary float WAV削除、音声decode／長さ、録音files/checkpoints hashとSQLite ID/text/audio時刻不変を検証。model/default/VAD/correction変更なし。raw誤認識／segmentation差は保持し品質合格としない。fixture故障3回帰も成功。詳細：[REAL-QWEN-GUI.md](REAL-QWEN-GUI.md)。実mic/desktop／会話／LLM/cloud/authなし。長時間・30秒境界・hard crash/power loss・package／clean-host導入は引き続き未検証。

## 合成native録音GUI／障害（部分確認）

専用private Pulse server＋eSpeak人工英文、ASR固定応答fixture-workerでHome開始→本体VAD→Stopでmanager除去後tail→実audio／raw transcript／SQLite保存→native再起動→再閲覧を確認。通常／worker exit7後のfresh PID／metadata completion保存失敗の3シナリオ成功。ASR errorでもcapture/Stopを維持し継続警告、認識不完了は `transcription_drained=false`。保存失敗は `save_succeeded=false` とcheckpoint保全（故障先の修復は試験側手動）。Qwen configのAPI key拒否とphantom default mic startも修正。実ASRモデル／LLM/cloud／mic/desktopは使わない。[RECORDING-GUI.md](RECORDING-GUI.md)、log `../logs/meetily-gui-recording-tail-final.log`。fixtureの接続確認であり、real-Qwen録音全体・長時間・crash/power loss・packageの完成ではない。

## 合成native GUIの再登録・編集・再起動・出力

Linux/Xvfb、隔離HOME/XDG、専用D-Busで、Homeのdisk orphan明示再登録→Meeting Detailsの本文編集→revision2保存→native再起動→編集保持→DownloadsへのJSON／Markdown実保存を確認。元item/evidence ID・before/after履歴保持、raw metadata/transcripts不変。ボタンはWebKit座標問題を避ける実DOM `.click()`、IPC/SQLite/files/downloadは実native。録音・ASR/LLM・authなし。test `frontend/tests/gui/minutes_native_smoke.py`、log `../logs/meetily-gui-minutes-final.log`。物理ポインター・競合UI・録音Stop全体・hard crash/power loss・長時間・配布版は未検証。全体完成条件はまだチェックしない。

## 再起動のdisk orphan検出／明示再登録（部分完了）

Homeのread-only bounded scanと手動DB registrationを接続。metadata/transcriptsが有効なfolderだけ、fingerprint確認＋transactionで登録。既存folder／繰返しnative recoveryを上書きせず、元音声／raw transcript／canonical／user edits／archivesは不変。SQLite再接続後のmeeting-ID再閲覧とuser edits保持を合成確認。旧復旧のfolder推測／checkpoint削除／age-based purge、DB error時のWAL/SHM削除retryを停止。

native44passed＋real Ollama1ignore、frontend24、check／TS／production build成功。実録音・推論・cloudなし。[RECORDING-RECOVERY.md](RECORDING-RECOVERY.md)。native全GUI・実crash／power loss・破損DB修復・audio-only orphan・disk/IndexedDB差分統合は未完／未検証。製品全体の復旧完成ではない。

## 録音保存の失敗表示／checkpoint保全（部分完了）

保存先初期化失敗はcapture前に拒否。UUID folderで同名・同時刻の別録音を保護し、停止時はaccumulator queueを最後までawaitする。checkpoint／結合audio／completion metadataをtemporary保存で保護し、transcriptsとmetadataのcommit前には正常終了checkpointsを消さない。Stop完了とファイル保存成功を別payloadで通知し、frontendのpersistent warningへ接続。

合成queue／FS故障・修復／元音声保全／path特殊文字、native34passed＋real Ollama1ignore、frontend18、check／TS／production build成功。実録音・推論・cloudなし。[RECORDING-SAVING.md](RECORDING-SAVING.md)、log ../logs/meetily-save-drain-final.log。再起動orphan検出／DB import・全体GUI復旧・長時間障害はまだ未完なので復旧完成条件はチェックしない。

## 選択式Qwen接続（部分完了）

- 本体のtrait provider／engine選択／start前model・language検証／Stop shutdown／Tauri commandsを接続。
- 設定UIでlocal worker・prepared model・ORT libraryを明示指定。JA/EN/THのみ、ready成功後の選択、失敗表示。既存ASR/defaultは維持。
- mono16k float32のprivate temporary WAV、終了／失敗／キャンセルで削除。30秒超burstは全sampleを連続bounded requestsで処理。追加量子化／ASR gain／補正なし。
- ready30s／request180s timeout、bounded framing・ID検証、kill-on-drop、次requestでfresh起動。失敗audioは再送せず、既存warning/errorで知らせる。
- 画面にQwenのAI草案・短語／否定語誤認識の注意を表示。品質合格／自動確定とは扱わない。
- Rust adapter5＋worker2＋sink3＋minutes6、frontend routing5、cargo check、TypeScript、production build成功。実workerから生成無音JA/EN/THの空結果・temporary cleanupを確認。古いworkerはreadyで拒否し、再build後成功。

log: `../logs/meetily-qwen-integration-final.log`, `meetily-qwen-adapter-smoke.log`, `meetily-qwen-worker-build.log`。導入：[QWEN3-ASR-SETUP.md](QWEN3-ASR-SETUP.md)。実speech／全GUI録音フロー／長時間・新しい30秒境界の品質は未検証で、完成条件の最初の項目はまだチェックしない。batch Qwenやapp crash後temporary cleanupも未対応。Linux monitorのsynthetic sink検証も下記まで完了。次は最終議事録・編集export/reopen・復旧へ進む。

## Linux monitor入力（native / synthetic-only確認）

`audio/capture/pulse.rs` の公開libpulse APIをdevice discovery／AudioStreamへ接続。monitor metadataとexact identity、default sink monitor、mono48k float32、source移動禁止、bounded startup／Stop、source/server error通知。既存mic/Windows CPALは維持。追加Cargo dependencies／lock変更なし。Linux Tauri deb追加依存はlibpulse0。

専用null sinkの生成440Hzをapp AudioCapture/channelまで取得。start/stop2回、pause/resume、source削除エラー、missing monitor拒否、同名専用sink明示再作成後のreconnectとcleanup成功。初回はPipeWireが削除済streamをREADYのまま保持して検出が失敗し、周期source確認で修正。pulse2・既存transcription7・sink3tests＋cargo check成功。実mic/desktop／ASR/cloudなし。詳細：[LINUX-SYSTEM-AUDIO.md](LINUX-SYSTEM-AUDIO.md)、log `../logs/meetily-pulse-final.log`。録音全体のend-to-end、server restart、長時間・配布版は未検証。

## 停止後の最終AI草案（接続／合成回帰確認）

session UUID＋TranscriptSinkを保持して、Stopのmanager除去後も同じ末尾履歴から更新。明示有効化したsessionだけ、drained event後に継続要求と直列にfinal pass。provider/model/accountを保持し、StopはLLM成功に依存しない。final失敗はpause／手動再試行、incomplete tailはmodel要求前に拒否。新sessionへ古い結果を保存しない。

timeout時のdispatcher detachをabort＋await／owned JoinSetへ修正。start／Stopとsettings変更をengine lifecycle lockで直列化。未知confidenceは-1 sentinelとして割合表示しない。

minutes10／transcription8、frontend runner7＋confidence1＋routing5、cargo check・TS・production build成功（実Ollama1ignore）。synthetic Stop tail撤回をrevision2／JSON／Markdown／archiveに保存し、同一evidenceのprojection再保存は追加model callなし。録音・推論・cloudなし。詳細：[FINAL-MINUTES.md](FINAL-MINUTES.md)、log `../logs/meetily-final-minutes-final.log`。実GUI stop全体／長時間incremental／再起動復旧は未完。manual edit保護／保存済みmeeting再閲覧は下記まで接続。

## ユーザー編集・ローカル出力・再閲覧（接続／合成回帰確認）

項目本文／状態／担当／期限のユーザー編集、全Item保護とbefore/after履歴、revision競合拒否、保存済meeting IDからの再閲覧、JSON/Markdown出力を追加。ID／分類／元evidenceは維持。AI応答検証前と最新canonicalを再読込するcommitで保護を復元し、推論中の編集も上書きしない。snapshot updatesやread refreshで入力中draftを初期化しない。

immutable/no-clobber archiveとcanonical保存、projection失敗の明示、export repair、schema1互換／corrupt拒否を確認。minutes17＋既存transcription8／sink3、frontend16tests、native check・TS・production build成功（実Ollama1ignore）。保存済み会議はsession registryなしでsynthetic SQLite経由に再読込。実録音・cloudなし。詳細：[MINUTES-EDITING.md](MINUTES-EDITING.md)、log `../logs/meetily-minutes-edit-final.log`。native download先／実GUI end-to-end／未保存SPA/native-close guard／再起動復旧・orphan recovery／長時間incremental・配布版は未検証／未完。
