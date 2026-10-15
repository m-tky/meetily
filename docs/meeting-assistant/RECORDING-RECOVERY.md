# ディスク会議の検出・明示再登録（部分完了）

再起動でmemory session registryやWebViewの保存処理が失われても、native保存folderに残るmetadata/transcriptsをSQLite一覧へ再登録する経路を追加した。**全体GUI・実収録・power loss・破損DB修復の完成ではない。**

## 利用フロー

Homeの「Saved recording recovery」で、実際のRecordingSaverが使うplatform default recordings rootをread-only scanする。root未作成なら空結果、アクセス失敗はerror。初回scanは自動だが、SQLiteへの追加は「Add saved data to meeting list」を利用者が押した場合だけ。

- active recording／停止／保存／開始中のUIをdisable。native commandもengine lifecycle lockとrecording flagで同時captureを拒否する。
- metadata・transcriptsのversion、日時、segment ID/sequence重複、非有限／不正な音声時刻等を検証。無効・読めないraw保存は登録しない。missing speechを推論で埋めない。
- original recording statusとpartial warning、保存segment数、音声fileの存在（品質未検証）、checkpoint数、canonical minutes fileの存在と読込警告を表示。
- opaque candidate ID＋閲覧時fingerprintのみを送る。commandでdefault rootから再検出／再読込し、metadata/transcript/canonical bytesが変わっていればRECOVERY_CHANGEDで拒否してrescanを求める。任意filesystem pathをimport commandに渡さない。
- SQLite meeting＋transcriptsを一つのtransactionで追加。既存folder_pathはcanonicalized pathで照合し、既存meetingがあれば上書きせずそのIDを返す。同じnative recoveryの同時／繰返し実行も同じIDに収束する。
- DB projection IDはfolder由来のnamespace＋元segment ID。別会議のseg_1が衝突しない。**元JSON／canonical evidence・item IDs／revision／user_edits／historyは一byteも変更しない。** 歴史的SQLite IDが失われた場合にそれを復元する機能ではない。
- sidebarを更新してMeeting Detailsへ移動し、既存typed meeting-ID minutes commandsからcanonicalとユーザー編集を再閲覧できる。

音声・transcripts・metadata・canonical minutes・revision archivesを削除／上書きしない。checkpointを結合する操作でもない。canonicalが壊れていても有効なraw transcriptsは明示再登録できるが、壊れたcanonicalを修復したり未採用archiveを昇格したりしない。notes readは引き続きcorrupt errorを返す。

## 保全した既存復旧経路

- IndexedDBの古い会議に現在の録音folderを推測で割り当てない。checkpointがない／確認できないだけで保存済folder bindingを消さない。
- folder pathはaudioの品質・可用性の証明ではないため、旧modalのAudio available／No audio表示をpath retained／missingに変更。
- 旧recovery終了後にcheckpointを削除しない。DB登録は成功しても音声結合に失敗しているケースがあった。
- 旧音声結合は既存audio.mp4を置換しない（存在を返すだけ、品質未検証）。新規結合もunique temporary output→sync→no-clobber publication、120秒timeout／kill-on-drop。regular numbered filesのみ、numeric order、番号gap/duplicateは拒否。失敗でpartial audio.mp4を公開しない。
- Home startupの7日／24時間自動IndexedDB削除を停止し、未保存会議の7日表示制限も撤去。年齢を保存成功の証拠にしない。これはcacheの保存期間を延ばす変更であり、disk使用量／利用者による明示cache管理は今後のUI課題。
- DB startupのmalformed/corrupt文字列を理由にSQLite WAL/SHMを削除してretryする処理を撤去。WALにはmain DBへ未反映のcommitted dataがあり得る。元errorを返し、明示的なbackup／repairは別工程。SQLiteライブラリ自身の通常journal処理まで無変更と保証するものではない。

## 検証

人工文・tmpdirs・SQLiteのみ（音声codec失敗fixture／先行zeros encodeを含む）。実mic/desktop capture、ASR/LLM/cloud、資格情報の読み出しなし。

native recovery7：

1. explicit registration→SQLite接続close/reopen→folder／raw text／canonical user edit／revision再読込。元全file byte一致。
2. stale preview／interrupted transcript JSONの登録拒否・非上書き。
3. SQLite INSERT failure trigger→meeting/transcripts rollback→retry成功、元全file一致。
4. concurrent retryのidempotencyと別会議の同一segment ID非衝突。
5. corrupt canonicalと未採用archiveの非昇格／非上書き。
6. 101folder pagination、8MiB超file拒否、未知／path状candidate ID拒否。
7. symlink folder／source file非追跡。

incremental5（既存audio／失敗時partial非公開の旧recovery2を追加）、sink6、minutes17、transcription8、DB open-error policy1成功。合計44passed／既存real Ollama1ignore。frontend新recovery4＋旧hook保全2＋既存18成功、cargo check／TS／pnpm10 production build／git diff check成功。Mock/SSRはnative操作全体の証明ではない。

logs: ../logs/meetily-recovery-final.log、meetily-recovery-frontend-preservation.log、meetily-recovery-preservation-final.log。locked/offline Cargo、pnpm10/Nix、analytics／Next telemetry無効、lock変更なし。

## native GUI合成確認

Homeのscan／明示再登録→Meeting Details→ユーザー編集→native app再起動→保持されたrevision2再閲覧→JSON／Markdownの実downloadを確認。新規HOME/XDGと専用D-Bus、人工文のみ、ASR/LLM/auth/captureなし。raw metadata/transcriptsは不変。WebKitの座標クリック問題によりボタンは実DOM `.click()`、IPC/SQLite/filesは実native。log `../logs/meetily-gui-minutes-final.log`、test `frontend/tests/gui/minutes_native_smoke.py`。hard crash/power loss試験ではない。

## 残る境界

- root4096entries、100folders/page、8MiB/file、32MiB/scan、50000segments/file、10000registered foldersのbound。limit/errorを通知し、切捨てたデータを回復済みとはしない。現在のactual default rootだけを扱い、外部／変更済rootの選択UIはない。
- 有効なmetadata/transcriptsが両方必要。音声-only、corrupt metadata、未checkpoint RAM、失われたspeechは自動復元しない。
- DBを正常に開けることが前提。実DB corruption／journal／power loss／hard process kill／Windowsの検証は未完。
- native disk recoveryと旧IndexedDB recoveryは別source。前者は後者をsaved扱いにせず、追加cached speechを捨てない。二つのsource間の比較／差分統合／旧save APIとのduplicate-aware reconciliationは未実装。両経路を別々に実行するとduplicate登録があり得るので、一つを選んで内容を確認する。
- unaccepted minutes archivesやcorrupt canonicalのrepairは明示workflowが未実装。任意download先／SPA/native-close draft guard、長時間incremental minutes、clean-host packagingも未完。
- symlink検査は通常の自分のrecording folders向け。攻撃者がparent directoryを同時置換するfilesystem sandboxや、blocking filesystem I/Oの強制中断を保証する設計ではない。
