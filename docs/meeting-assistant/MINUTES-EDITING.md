# 継続議事録のユーザー編集・出力・再閲覧

## 接続した操作

- LiveMinutesPanelの同じsession内で保存済み草案を読込／再読込し、項目ごとに本文・状態・担当・期限を編集する。
- Save時に `expected_revision` を確認。競合は `MINUTES_REVISION_CONFLICT` で拒否し、古い版を無断で上書きしない。UIは入力を保持し、再読込後に**最新の版にこの入力を再適用**を明示操作できる。
- 本文を編集してもID・分類・元のevidence IDを変更しない。担当／期限はユーザー入力として明示し、speakerを人物に捏造しない。ユーザーの変更文が元発言で裏付けられると自動判定しない。
- 保存した項目は**項目全体**を後続AI更新から保護する。AIが書き換えたり省略してもlatest user editを復元する。モデルpayloadには保護IDを渡し、応答の構造検証／repairの前にも復元する（保護項目の省略だけで不要なrepair要求をしない）。新発言による撤回／置換も自動適用しないため、保護した内容・状態は人間が見直す必要がある。
- `user_edits` に編集時刻・base revision・変更前／後の完全Itemを追記する。以前のAI文・根拠・ユーザー文は消さない。現時点で保護解除／編集履歴削除のUIはない。
- JSON／Markdown出力は保存済canonical revisionを使い、未保存の入力を含めない。JSONはuser historyを保持、Markdownはuser-edited markerとbefore/after historyを含む。
- Meeting DetailsのSummary側に **保存済み継続議事録（ローカル閲覧・編集・出力）** を追加。保存済meeting ID→SQLite metadataのfolder_pathから読込むため、録音session registryを再構築しなくても保存された会議を再閲覧／再編集できる。従来のsummary editorと別のcanonical documentであり、一方をもう一方へ置換しない。

新しいread/edit/export commandsはsession IDまたは保存済meeting IDを受け取り、任意のfilesystem pathは受け取らない。これらの操作でASR／LLM／cloudを呼ばない。録音／継続クラウド更新を別途明示有効化していれば、その選択済providerへ後続の文字起こし・議事録更新が送られる既存動作は継続する。

## 保存／並行更新

`live_minutes/document.rs` のwriter lockは短いfilesystem commitだけに使用し、ネットワーク推論中は保持しない。従来のAI request lockは別。遅いAI応答のcommit時にcanonicalを再読込し、**推論中に保存されたuser edits**を保護してから検証・次revisionへ保存する。cached fingerprintでMarkdownだけ修復する場合もlatestをwriter lock下で読むため、古いprojectionでユーザー文を上書きしない。

- `live-minutes.json` がcanonical、`live-minutes.md` は派生projection。
- revision archiveはimmutable/no-clobber。失敗で残ったarchive candidateがあればrevision番号を飛ばし、そのファイルを上書きしない。
- archive→canonical→Markdownを各atomic/fsyncで保存。canonical commit後のMarkdown失敗は `MINUTES_PROJECTION_FAILED: canonical revision ... was saved` と表示する。再読込／exportでprojectionを復旧でき、同じ編集を黙って二重適用しない。
- orphan archiveは自動でcanonicalへ昇格させない。ファイル破損／未知schema／サイズ超過ではfail-closedにして既存ファイルを消さない。read/historyは8MiB上限、revision directoryは100k entries上限。
- 旧schema_version1（user_editsなし）は空historyとして読める。同versionにoptional fieldを追加し、raw transcript／item IDs／existing archive layoutは維持。
- writer lockは同一app process内。通常のsingle-instance guardは維持しているが、外部editor／複数processからの同時ファイル書換えは新しく保証しない。

## UI／出力について

同じtargetを表示している限り、AI snapshot propsやrevision更新で未保存draftを再初期化しない。遅延したAI／read応答はより新しいrevisionの表示を巻き戻さない（functional state updateで検証）。route/session identityが変わった後の古いasync結果はcomponent lifecycleで抑止する。browser reload/closeにはbeforeunload warningを登録するが、Tauri native close／SPA navigation全体の未保存確認は未検証。**Save前の入力はdiskには保存されない**。

ファイル出力UIは既存app summary exportと同じBlob download方式。backendが一つのcanonical revisionからJSON/Markdown本文を作り、既存meeting folderのMarkdownも修復する。mock downloadではfilename／body／URL cleanupを検証。Linux/Xvfb native WebViewのJSON／Markdown Blob downloadは、合成GUIフローで隔離Downloadsへの実保存を確認済み。filechooser／任意保存先／他platformは未確認。出力先を任意pathとしてbackendに渡してcanonical/audioを上書きするcommandは追加していない。

## 検証

- native document5 tests: edits保護（AI rewrite／omission）、reload/export/history、stale revision／無効patch／unknown ID非保存、immutable archive collision保持、projection失敗後のcanonical保持／export復旧、old-schema互換／corrupt拒否、synthetic SQLite saved-meeting resolution（sessionなし）。
- orchestration追加2tests: protected項目の省略をvalidation/repair前に復元しcall1回、 live modelが旧revisionを読んだ後でuser editを保存→応答commitはuserの本文・撤回状態を保持しrevision3になる。
- frontend3 tests: draft/base revision独立、patchはID／sourcesを変更できない、SSRでuser provenance／escaped text／controls表示、mock JSON/Markdown downloadのbody／revision／cleanup。
- 既存minutes/session/final testsとtranscription/sink/control testsも実行。minutes計17＋既存transcription8／sink3、frontend計16tests成功（既存実Ollama1はignore）。全native check、TypeScript、pnpm10 production build成功。

logs: `../logs/meetily-minutes-edit-final.log`, `meetily-minutes-edit-final-frontend.log`（最新UI単調revision回帰＋TS/build）, `meetily-minutes-edit-check.log`。testsはtemporary folders／人工文／in-memory SQLite／SSR／mock DOMだけ。実会話／mic／desktop capture、ASR精度追試、LLM/cloud推論、資格情報アクセスなし。Cargo/pnpm lock変更なし。

## native GUI合成フロー

Homeの未登録folder再登録→Meeting Detailsで本文編集→revision2保存→native app再起動→同meetingの編集保持→JSON／Markdown実downloadが成功。item `gui-item-1`／evidence `seg_gui_1`、before/after履歴を確認し、JSON出力はcanonicalと完全一致。raw metadata/transcriptsはbyte不変。

`frontend/tests/gui/minutes_native_smoke.py`、log `../logs/meetily-gui-minutes-final.log`。新規HOME/XDG・専用D-Bus・無効Pulse/null ALSA・analytics無効、人工文のみ。ボタンはWebKitの座標クリック問題を避ける実DOM `.click()`、本文入力はWebDriver、IPC/SQLite/files/downloadは実native。物理ポインター／競合UI／native-close警告／実録音フローの証明ではない。初回dev serverのCSS/JS 404は再起動で解消し、再起動待機の旧本文依存も修正。スクリーンショット・DB・出力はrepo外の隔離runtimeに保持。

## 次の未完条件

実GUIの編集競合・物理ポインター操作、録音保存の失敗通知／再起動復旧・orphan recovery、長時間incremental minutes、clean-host packageは未検証／未完。既存summary側の別機能による生成は今回のdocument操作と区別する。`LINUX-MVP.md` の全体完成条件は引き続き未達。
