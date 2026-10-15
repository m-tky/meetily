# Stop tail / 最終AI草案

## 接続したフロー

1. 録音startupで `live_minutes/session.rs` のsession registryへUUIDとsession-bound TranscriptSinkを登録する。folderとevidenceをglobal RecordingManagerから別々に読む旧経路を撤去。
2. ユーザーがLiveMinutesPanelでprovider/model（cloudの場合はaccountと送信説明）を明示選択し、継続更新を開始する。そのsession IDと設定をrunnerに固定。登録／録音開始だけではモデルを呼ばない。
3. 継続更新は単一実行。停止してmanagerがglobalからtakeされても、同じsinkの末尾発言を読む。新しい録音のUUIDが異なる場合はモデル要求前／結果保存前に拒否する。
4. Stopは音声停止→文字起こしdrain→録音保存を行う。議事録モデルはStop関数から呼ばず、LLM成功に停止を依存させない。
5. `recording-stopped` に `minutes_session_id` と `transcription_drained` を追加。有効化された**同じ会議**だけrunnerが最終passをqueueする。実行中の継続passがあれば待ち、並行モデル要求をしない。ID不一致や未有効化では呼ばない。
6. 最終passはended＋drainedをbackendでも検証してから全available evidenceをreconcileする。raw transcript、stable item ID、撤回・変更evidence／revision archiveを維持。同じfingerprintなら追加LLM callなしでMarkdown projectionだけ再保存する。
7. 最終pass失敗時は草案を残して画面にエラー表示。usage/policy/permission/configは自動pause、temporary errorは最大3回のbounded backoff。provider/account/modelの無断fallbackなし。**最終草案を更新／再試行**ボタンはユーザーの明示操作で同じ停止済sessionを再確認する。

停止しただけで草案を自動確定しない。UIの「最終」はavailable Stop-tail evidenceでpassを終えた意味で、正確な会議録／人間の確認／全音声の認識成功を保証しない。元録音や既知の短語・否定語誤認識を確認する。

## Stop timeout / failure

従来は `timeout(task_handle)` に所有権を渡し、timeoutでJoinHandleがdropされてdispatcherがdetachされた。またdispatcher内のworker JoinHandlesもdetachされ得た。

- `finish_task` はowned handleをmutable borrowして待ち、timeout時はabort＋await。
- dispatcherはowned `JoinSet` を使い、drop時にworkerもabortする。Qwenのin-flight child／temporary WAVは既存kill-on-drop cleanupへ進む。native engine内部で開始済みblocking inferenceの即時停止までは保証しないが、取消workerはその結果をtranscriptへ保存しない。
- dequeue/completed countとは別に、engine初期化／認識失敗／worker panic／transcript disk保存失敗をcomplete=falseとして返す。timeout／failureでは警告し、automatic final passを禁止。既存AudioTooShort skip/no-speechは引き続き処理方針を変えない。
- start/Stopは同じengine lifecycle lockで直列化。startupはdispatcher handleの登録・started eventまでguardを保持するため、Stopが未登録taskを取り逃がさない。Stop中のsettings/model変更・次startも待機する。

継続更新のopt-out/unmount後は新規モデル要求と最終passを止め、古いUI結果は表示しない。既に実行中／送信済みの要求はbackendで完了・同じ会議folderへ保存される場合がある。cloudへ送信済みのデータを取り消す機能ではない。

## 検証（録音／モデル要求なし）

- Rust minutes10 tests成功、既存実Ollama1はignore。追加session2: manager消失後tail保持、new sessionへの非混入、incomplete final拒否。
- 追加orchestration2: synthetic live→Stop-tail撤回→revision2/JSON/Markdown/archive保存、同じfingerprintの再試行がモデルを呼ばずprojection復旧、incomplete tailは要求前拒否、slow old resultをnew sessionに保存しない。
- transcription8 tests成功（Qwen5＋worker3）。追加timeout testはdispatcherのowned worker resourceが確実にdropすることとcomplete true/falseを確認。
- frontend runner7: 未opt-in／stale Stopは要求0、in-flight後に最終1回、incomplete tail／cancelで新規要求と古い表示停止、usage limit pause、temporary retries3、manual finalのended/drained guard。
- frontend confidence1＋provider routing5 tests、cargo check、TypeScript、pnpm10 production build成功。
- scoreを持たないproviderに旧workerが85%を捏造していた点を修正。既存numeric schemaを保ち `-1` をunknown sentinelとして保存し、ConfidenceIndicatorは不明／nonfinite／範囲外に割合を表示しない。モデルが返した既存scoreは維持する。

log: `../logs/meetily-final-minutes-final.log`（final suite）、`meetily-final-minutes-check.log`（sink回帰を含む前段）。Cargo/pnpm locksは変更なし。synthetic stringsとtemporary folders、mock Promise/timersだけ。mic／desktop recording、ASR精度追試、Ollama／cloud推論・credentialアクセスなし。

## 未完

- 本物のGUI stop→tail→最終草案のend-to-endやモデル意味品質、長時間／server failure／crash recovery全体は未検証。
- contextは既存48,000 UTF-8 bytes上限。超過ではモデルを呼ばずエラーを表示する。長時間のincremental reconciliationは未実装であり、勝手にevidenceを切り捨てない。
- 後続工程でユーザー編集保護・editing/export/reopening UIを接続した：[MINUTES-EDITING.md](MINUTES-EDITING.md)。saved meeting ID→SQLite folder_pathでsession registryなしに閲覧可能。app再起動後のactive recording session再構築はまだ未実装。session registryは最新1会議のin-memory handleだけで、再起動で失われる。
- 録音保存のdisk failure通知／再起動復旧、native blocking inference cancellation、配布package／全体運用検証は `LINUX-MVP.md` の残項目。
