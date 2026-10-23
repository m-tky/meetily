# 開発状況（2026-10-12）

## 最新：録音済みAACのimport、両ファイル×両モデル4通りPASS（精度は未合格）

ユーザーが『録音済み音声のimport』と明確化。別実装のimport.rsにも既定Music保存先を使う不具合を発見し、ユーザー承認で設定save_folder snapshot→worker初期化→absolute選択先の新規folder作成へ最小修正、native2tests／source guard／Windows import gate追加。基準fd91a5d、patch8d33ca6…、candidate SHA256=985ba233…をWindowsでbuild／実行。前試験で実際に保存したAAC/MP4 2件（10.752／10.624s）を1.7B／0.6B各々で実UI importし、4新meeting保存／指定private先／原音とcopy SHA一致／原recording全hash不変／raw-JSON/SQLite一致／unknown source保持／global Large不変／worker/tmp cleanup／nativePID7512→14296再起動後4件表示・hash保持PASS。native import durationは10.773333／10.645333s（FFmpeg decode比約21.33ms差、正確なgapless同期は未検証）。新録音・再生・実mic・cloudなし、test prefs復元／owned app-debug閉鎖、元cache実行物hash一致復元。Windows native97unique（SIWC14含む、import1ignore）、Linux import16（+1ignore）、Node両OS48、PS parser0errors。初回harnessの日本語JSON console文字化けで停止した失敗を保持し、UTF8直接file読書きへtest側のみ修正して全4通り再実行成功。**認識精度は問題なしではない**：1.7Bも録音Aで価値→ワカシ等、Small両件も誤りあり。録音BのLargeだけはこの短い参照文に句読点以外一致。原因はこの試験だけでは特定せず、音声処理／model／VAD／gain／補正は未変更。通常app／installer反映・release・既存データ移動なし。詳細：[RECORDED-AAC-IMPORT.md](RECORDED-AAC-IMPORT.md)、Git外 `../runtime/windows-qwen-only-retest/evidence/recorded-aac-import/final-summary.json`。

## 最新：保存先・Windows同期修正、両実モデルの仮想録音通し試験PASS

ユーザーが保存先不具合の修正と仮想限定再試験を承認、さらにWindows同期エラーの最小修正を別途承認。両start経路からsave_folderのsnapshotをmanager→saverへ必須伝達、absolute／無効先は開始拒否・state rollback・既定Music/Documentsへfallbackなし。checkpoint writer自身のwrite handleでflush＋sync、AACはNamedTempFileのwrite handleをsync（同期省略や音声形式変更なし）。Linux保存24 unique／Windows native95 unique（SIWC人工14含む、既存import ignore1）／Node両OS47／PS packaging parser0errors成功。Windows専用buildにも保存regression gate＋verified FFmpeg PATHを追加、installer実行なし。同一修正candidate SHA256=1c53897…で0.6Bと1.7B双方：実DOM選択→仮想PWEC＋無接続emulated Line In→実capture/VAD/Qwen→manager除去後Stop-tail各1→指定private folderへ保存→event/raw/SQLite一致→worker/tmp cleanup→新native PID再起動・本文／filehash保持PASS。保存音声10.752／10.624s、decode警告0。owned console切断・SPICE全channelなしをwatchdog178／316回確認、実mic／host音声／conversation／cloud／実authなし。AAC peak>1／Small誤認識は残り品質acceptanceではない。元cache top-level本体／worker／DLL復元hash照合、通常app／profile／ログイン保持、Calibre削除不要で未実施。VM起動維持、owned app/workers/debug停止、consoleは音声隔離のため切断。初回folder失敗・harness cleanup不備・PowerShell配列／telemetry失敗・folder-only時のWin sync失敗とAPI再現は全保持。前の『本体未修正／録音未検証』は履歴。詳細：[RECORDING-SAVE-FOLDER.md](RECORDING-SAVE-FOLDER.md)、Git外 `../runtime/windows-qwen-only-retest/evidence/save-folder-fix/final-summary.json`。

## 最新の実不具合：録音が設定済み保存先を無視する

ユーザーが仮想音声だけの試験を明示承認。Windowsの仮想PWEC出力／エミュレートLine Inを実UIで選択・reload保持確認し、owned virt-manager接続を切断、host watchdogがSPICE全channelなし・物理passthroughなしを確認してStart。**公開音声再生前にFAIL**：native preferencesの専用save_folderではなく既定Music/meetily-recordingsに試験folderを作成。`recording_saver.rs::initialize_meeting_folder` が常にget_default_recordings_folderを使う実装を確認（共通コード、Windows限定とはしない）。本体未修正。初回harnessのabort cleanupは必須stop_recording.args漏れと誤ったcleanup_complete表示を含んだため、別helperで正しいargs・PID/exe照合→停止・owned app/listener閉鎖を確認。今回だけの正確なfolderを専用領域へMove/hash照合、telemetry集計ミス後の独立検証も成功（metadata1件456bytesのみ、音声／transcript保存なし）。既存録音・Calibre未削除／未読取。以前のfile-based実モデルbatch検証PASSは維持するが、録音／custom-folder正常動作とは絶対に扱わない。詳細・全失敗：[QWEN-ONLY-WINDOWS-RETEST.md](QWEN-ONLY-WINDOWS-RETEST.md)。次は保存先伝達の修正承認と回帰／仮想限定再試験。

## 最新：Qwen-only版をWindows VMで実行検証

ユーザーのvirt-manager検証依頼でwin11を起動・許可されたログイン。初回debug領域は容量安全guardで停止し、ユーザーが既存releaseキャッシュ再利用を明示承認。現source `9d37569` を専用source／identifier／WebView／録音先でbuild、Node46・worker19・native57 unique（既存import ignore1）・SIWC人工14・TS／frontend export／native build／clean-PATH startup/refusal6条件成功。両サイズの実weights＋公開JA音声で、実GUI onboarding選択→import→retranscription→raw/SQLite一致→native PID変更再起動／Small保持／本文保全成功。両サイズの実Rust adapterもfloat32無音3言語／公開JA repeat／Unicode temporary cleanup成功。モデルは既存公式manifest全SHA確認済み専用CoWコピー（Windows FileTime用にコピーだけmtime／cache identity調整、payload不変）、新規download／Windows cold初期化は行わず。元cacheのtop-level本体／worker／DLLはhash一致復元、通常app／profileは保持。Calibre削除許可も得たが削除不要で未実施。録音・再生・実mic／PC取得・実会議・実アカウントauth／cloud・NSIS生成／公開は未実施。**Windowsの録音→Stop-tail→保存という一連の動作は今回まだ未検証**。初回source archiveの.github欠落によるNode失敗を保持・修正し全gate再成功。詳細：[QWEN-ONLY-WINDOWS-RETEST.md](QWEN-ONLY-WINDOWS-RETEST.md)、Git外証拠 `../runtime/windows-qwen-only-retest/evidence/`。

## 最新：Qwen-only checkpointの実行再検証

ユーザーが実テストとWindows同等機能の確認を依頼。`3187df6` を現在のLinux hostで再build／再実行し、Node46・native59（既存ignore1）・worker19・実startup probe6・TS・隔離Next export・embedded debug build成功。両sizeのfixture batch/live GUI（import／再文字起こし／失敗・取消保全／Stop-tail／保存／native再起動）成功。既存両size公式weightsをmanifest全SHA照合→専用reflink copy、実workerで公開JA2音声＋無音＋repeat／shutdown成功。既存metricsで1.7Bは0/74文字誤り、0.6Bは9/74（12.16%）：Smallは実際に誤認識し、品質同等ではない。0.6B／1.7Bそれぞれの実モデル＋private virtual人工音声live GUIもStop-tail／保存／cleanup／再起動成功。debug速度は性能評価に使わない。Windowsは共通実装と専用installerの両manifest同梱を確認したが、VM停止中・起動せず、最新Qwen-only版のWindows実行受入は未実施。旧汎用Windows workflowのQwen未同梱・専用CIのbatch/Small検証不足・Windows device resolverの部分一致／default fallbackを未解消の確認事項として記録。実mic／host desktop／物理再生／ユーザー本文・資格情報／cloud／表示中app反映／releaseなし。詳細と初回runner失敗：[QWEN-ONLY-RETEST.md](QWEN-ONLY-RETEST.md)、log `../logs/qwen-only-retest/`。

## エージェント引き継ぎcheckpoint

ユーザー依頼に従い、Qwen-onlyの実装＋両サイズfixture batch/live GUI回帰完了を区切りに [HANDOFF-QWEN-ONLY.md](HANDOFF-QWEN-ONLY.md) を作成。現在地、コード入口、最終log、再開順、未検証項目と承認境界を集約した。引き継ぎsessionでは最終logを再確認し、Node46/46とdiff checkを再実行成功。native／GUIは前session最終結果であり今回再実行とは区別する。source／tests／manifest／文書だけを `feat/live-minutes` にcommit／pushする。tag／release／workflow dispatch／表示中app更新は行わない。

## 最新：ASR全経路をQwen3-ASR 1.7B／0.6Bの二択へ置換

ユーザーが「元のASR経路を削除し、Qwenの大／小だけにし、音声インポートも経路全体を置換」と依頼。小型は**0.6B**と確認し、ask_userで **旧実装・依存まで削除** を明示選択。既定1.7B維持、Whisper／Parakeet／cloud-ASR選択・native登録コマンド・provider・model管理UI・旧onboarding download・不要な旧STT/backupを撤去。直接依存と不要な9 packagesをCargo lockから削除し、残存package versions／pnpm lock／worker lockは不変。ONNXは既存Silero VAD用に残す。録音・import・retranscriptionを同じQwen adapterへ接続し、両モデルを独立した設定／manifest／保存先で管理する。旧providerだけ条件付きSQLで1.7Bへ移行し、明示的Small選択や旧key／filesを上書きしない。DB schema／録音・ASR前処理／言語・gain・VAD／Luna認証・要約経路は変更しない。

Batchはjob所有worker・固定model/language・録音／他batch／setupとのnative排他・実行中取消・temporary float WAV cleanup、失敗／空認識で旧本文を保持。unknown Qwen IDはfallbackなしで拒否。既存float32/full-input 1.7B workerの2引数CLIを維持し、model-bound handshakeでSmall誤接続を拒否。新workerは0.6B/1.7Bをconfigから識別、実推論時tensor shapeも照合する。明示`--model`はweightsなしruntime probe用にも使えるがmodel_loaded=falseを保持。Small setupはverified official weightsからstagingでcacheを新規生成して既存cacheと比較、無断置換しない。WelcomeはQwen-only、ローカル要約model不要。Nix二manifest／Small準備app／Home Manager `qwenModelSize`と個別backup、Windows manual `-ModelSize`を追加。

**検証：Native59PASS＋既存import fixture1ignore、worker19PASS、実debug worker起動6条件PASS、Node46＋isolated Bun65＝frontend111PASS、TS／隔離Next export／locked offline embedded-debug build／Nix derivation評価／Python compile／diff check PASS。** Nativeは設定移行2・transcription23・retranscription17・import14・onboarding3。fake ASR＋人工eSpeakファイルのfresh HOME/XDG/private D-Bus/Xvfb GUIで、両modelのSettings選択→他model overrideでimport→選択modelで再文字起こし→JSON/SQLite一致・unknown origin→異常／実行中cancelで本文保全→native旧PID退出・新PID再起動保持PASS。held inference中のmodel/path/setup変更と録音preflight拒否もPASS。さらに各size独立private Pulse/null sink・実capture/VAD・fixture ASRでStart→manager除去後Stop-tail→音声／event/raw/DB保存→worker/tmp cleanup→native restart/reopen PASS。**認識器は意図的なfake worker、実weightsなし。実0.6B推論／認識品質の合格ではない。**

初回のcold cache/Git・jni metadata欠如、不要なchannel toolchain取得、Nix vendor取得中断、helper sidecar欠如、開発shellのBLAS link失敗を保持・修正。公開metadataとchecksum照合済み固定crate archivesだけbuild cacheへ取得し、model weightsなし。初回TSの旧provider比較／新model prop漏れ、Windows source guardの旧qwen_init期待も修正。GUI harnessのselector引用符／HSTSをDBと誤認／Settings anchor誤認／閉じかけRadix操作はtest側のみ修正し、全失敗logを保持。最後に旧1.7B worker CLI互換も固定し、最終compat build＋全GUIを再実行成功。

最新log `/home/user/logs/meetily-qwen-only/{native-tests-r4,worker-tests-r2,worker-probes-r1,node-complete,compat-native-gui,nix-eval-final}.log`。本体debug exeは`target/debug/meetily`、frontendは`/tmp/meetily-qwen-only-frontend-iMMhiW/frontend/out`をembed、worker debugは`/tmp/meetily-qwen-worker-target/debug`。**通常／表示中アプリの置換・再起動、Home Manager switch、実mic／host desktop／physical speaker、実本文・資格情報読取、cloud推論、release／公開なし。** 本番profileの移行実行、実0.6B speech／新規HTTPS model download完走・初期化、Windows／clean-host／長時間は未検証。反映は保存・閉鎖確認と別承認後。詳細・再現・失敗：[QWEN-ONLY-ASR.md](QWEN-ONLY-ASR.md)。過去のWhisper/Parakeet維持・Qwen batch未対応という記録はこの決定でsupersede。

## 最新：LinuxはQwen3-ASRだけで初期設定完了・再起動を確認

ユーザーが「Qwen3.5 4BではなくQwen-ASRだけ」と明確化して続行承認。既存の修正済Nix debug候補 `pr7b0kc…` をfresh HOME/XDG/private D-Bus/Xvfb/null ALSA/unavailable Pulseで検証。実HTTPS進捗→UI Cancel→未選択・未完了・partial cleanup PASS。その後は重複約4.7GB転送を避け、既存公開モデルのmanifest9filesだけcopyして明示retry（cache/settings/completion flagsはseedせず）。本体checksum検証→fresh INT8 cache初期化→worker検証・Qwen選択→Qwen-only onboarding完了→native旧PID終了確認→新processで設定・完了状態保持／録音画面到達 PASS。Parakeet／summary download flagsはnot_downloaded、Qwen3.5や別の大型model追加なし、録音なし。完全新規HTTPS download完走とは区別し未検証を維持。

追加Node13／TS／Python compile／diff PASS。初回転送中断・unsupported sessions404・背景SIGINT無効によるowned環境限定cleanup・port/display衝突・cancel domain errorをprotocol errorと誤認したharness失敗・r4 timeoutを保持。harnessのerror envelope／signal handling／startup guard／実button aria-labelを修正、最終r5全成功。`QWEN3-ASR-SETUP.md`の通常手順をアプリ内ASR-onlyへ更新、旧manual準備は開発者用へ分離。本体source／通常profile／実音声・再生・ASR/cloud推論・auth・releaseは変更／実行なし。その後ユーザーの保存済み・閉鎖／反映承認を受け、same `dual-preview-4p61rc4k` :0/4454を `runtime/nixos-asr-only-candidate` → `pr7b0kc…` でpreserve-settings/no-screenshot再開。旧helper不在／port free／profile process0を事前確認。DOM metadataで既存ログイン・Lunaのreload保持／Qwen・JA／自動setupあり同意unchecked／jobなし／未録音を確認し、Settings Transcriptionを開いた。新native executable・owned host capture0も確認。直URL遷移／Radix click／wrapperと実exeの識別・孤立test processによるunique判定の初期検証失敗を保持し、実link・mousedown・exact profile scopeへ検証を修正して成功。timeout試験のowned isolated leftoversをcleanup、旧4453／通常app保持。詳細：[QWEN-LINUX-ONBOARDING.md](QWEN-LINUX-ONBOARDING.md)、log `../logs/meetily-linux-qwen-asr-only-retry-r5.log`。

## 前工程：LinuxのQwen自動導入をWindowsと共通化

ユーザーがLinuxにもWindowsと同じQwen自動導入を要求し、Qwen3.5 4B等は不要と明示。Windows限定だったsetup availability／download job／Qwen-only onboardingをLinuxへ接続。Welcome／Settingsの同意付き共通UIでQwen3-ASR 1.7B decoder-INT8をdownload→checksum検証→初期化→worker検証→明示選択し、Parakeet／local summary modelなしで完了できる。既存任意catalog／設定／filesは削除しない。Nix appへ共通initializer・ORT link・CA pathを同梱、manual preparationと同一derivationを共有。native23＋onboarding3（重複1）、Node23／Bun5／TS／Nix評価／initializer build PASS。full Nix debug app build／isolated export成功。初回packageはGUI表示成功でもORT dlopen FAIL（GAppsが.soをwrapper化）を保持、postFixupでlink公開＋実dlopen build gateへ修正。修正候補`/nix/store/pr7b0kc0pf8b88hga8gjycvk6ji5z2h6-meetily-local-0.4.1-local`は再build／実ORT load／fresh native Welcome＋browser reload（同意unchecked・jobなし・model未作成・未録音）PASS。実HTTPS download／準備後選択・native再起動は未検証。モデルdownload／録音再生／推論／auth／通常profile／表示中app変更／releaseなし。詳細：[QWEN-LINUX-ONBOARDING.md](QWEN-LINUX-ONBOARDING.md)。

## 最新：録音後の要約も共有Luna設定へ接続・反映

ユーザーが別のOllama設定欄を指摘し「Lunaのやつもちゃんと設定して」「continue」。共有ChatGPT/model設定をsaved-meeting Generate/Regenerate Summaryへも接続。旧Ollama設定欄はplan選択時非表示。Nativeはjob作成前にaccount/plan/catalogを検証し固定、既存SIWC Responsesへroute、API keyや他providerへfallbackしない。Cloudのpost-Stop自動生成は明示操作とみなさず二段でblock、手動Generateでのみ開始（明示opt-inのlive minutes Stop-finalは維持）。Native78／frontend36 PASS、TS／isolated frontend export／locked offline debug build／diff check成功。初回Rust/TS列挙漏れcheck失敗は保持・修正。ユーザー再起動承認でidle確認→今回4454だけ再開、同profile・ログイン・Luna保持。新候補 `../runtime/nixos-luna-summary-candidate` → `/nix/store/cvh8dnm02rdyqg07dxfcw4lg0qgmd50x-meetily-local-candidate`。DOM/metadataで設定reload保持／旧欄なし確認、Settings Summaryを開いた。実Generate／推論／本文・音声・native録音log読取／screenshotなし。実アカウントpost-summary推論はユーザー操作待ちで、過去の人工文成功とは別。詳細：[LUNA-SUMMARY.md](LUNA-SUMMARY.md)。

## 前工程：Luna認証・モデルをSettingsへ集約、英語UIと補足文削減を反映

ユーザーがLuna接続・人工文テスト成功を報告後、認証／model選択をMeetily設定へ移動、UI英語化・補足文削除を依頼。Settings → Summary → Meeting minutesに集約し、録音画面は選択値＋Settingsリンク＋明示更新操作のみ。既存preferences.jsonへprovider/modelだけ保存、資格情報とnative SIWC方式は変更なし。更新同意は保存／自動有効化しない。account/plan/model catalog検証と未知モデルのfail-closedを維持。32frontend PASS／TS・isolated frontend export・debug native locked/offline build・diff check成功。ユーザーが反映再起動を承認、idle確認して4454の試験helperだけ再起動。同じprivate profile／既存ログイン／録音・ASR言語を保持。新GC-root `../runtime/nixos-settings-ui-candidate` → `/nix/store/w4490m37x7k87b1nr06inbzdbsqv5rx5-meetily-local-candidate`、タイトルMeetilyでSettings Summaryを開いた。DOM/metadataのみでログイン維持／Luna選択／reload後の設定永続化／録音画面からauth選択UI除去を確認。実録音・本文・native録音log・screenshotの読取なし、ログイン／推論／録音／再生の実行なし。旧候補・通常profile・dev executable/.next/out保持、releaseなし。詳細：[SETTINGS-UI.md](SETTINGS-UI.md)。

## 前工程：入力別ASRはユーザー試用成功報告、Lunaのログイン準備済み

ユーザー「できるようになった。画面は閉じた」。入力別ASRのユーザー成功報告として記録（実本文・録音をtoolで読んだ検証ではなく、精度／長期運用acceptanceとは分離）。次にMeetily議事録用Lunaを選択。既存アプリ専用公式ChatGPTログイン、まず人工文だけ、実録音・本文送信はまだしない。保存済み／再起動可も明示確認し、今回4454のowned helperだけ終了→同じprivate `dual-preview-4p61rc4k` を再開。タイトル「Meetily — Luna接続準備 / Mic・PC / 日本語」、議事録provider chatgpt_planを選択しContinue with ChatGPT表示まで準備成功。未録音・origin・継続更新OFF確認、ログインや人工文テストや実推論のクリックなし。現在profileの認証／モデル一覧／人工文実推論はユーザー操作待ち。旧profile／通常data／既存Codex/pi資格情報には触れない。再開時のscreenshotなし、実本文・音声・native recording log読取なし。UI準備の初回引用符ミス／次のDOM change反映失敗を保持しReact onChangeへ修正して成功。helper py_compile／diff check成功。本体auth/provider実装や候補binaryは変更していない。詳細はSOURCE-TRANSCRIPTION.md／ADR-002-OFFICIAL-SIWC.md。

## 前工程：文字起こしのみを選択、Murmur型の入力別ASRをMeetilyへ実装

**次の承認で試用画面を開いた：** ユーザー「試してみたい」→マイク＋PCを選択。元`.next`／`out`／旧画面は維持し、sourceだけを外部build directoryへcopyして新frontendをexport、新UIをembeddedしたdebug／custom-protocol／no-bundle build・Nix stage成功。元dev executable復元hash一致、stage=input hash一致、worker旧版byte一致、lock変更なし。GC-root `../runtime/nixos-source-transcription-candidate` → `/nix/store/wnimgmc798andrlssvrymyp80n981fr0-meetily-local-candidate`。新隔離profile `../runtime/dual-preview-4p61rc4k`／DISPLAY:0／port4454、タイトル「Meetily — 入力別文字起こし / Mic・PC / 日本語」でREADY。旧4453画面・通常profileには触れない。READY時の未録音／embedded origin／EchoCancel両入力の列挙・新native processを確認、automatic_recording=false。最初のdriver path typoはprofile作成前失敗、訂正後READY（失敗log保持）。ユーザーのStart→mic／PC各別発話→Stop→保存確認待ち。toolsによるStart/Stop/実再生／実本文や録音/native recording logsの読取なし。実認識品質・保存acceptance・releaseなし。詳細はSOURCE-TRANSCRIPTION.mdの試用候補節。

ユーザー「文字起こしだけ」「話者分離はなし」「murmurの方針で実装して」でdiar／ForcedAligner採用を取り下げ。liveは共通120ms sample窓から選択mic／PCそれぞれの独立VAD→共有ASR1worker直列へ変更。本文と取得元mic/systemだけを保持し、人物識別・話者境界分割なし。録音mixer／WAV checkpoint／最終AAC、model／language／VAD設定／capture gainは維持。既存nullable speaker列を使い、JSON／イベント／履歴／SQLite／pagination／IndexedDB＋orphan復旧／live・保存後UI／copyへ取得元を通す。Mic／PC表示のみ、旧混合やimportはunknownのまま。同じ文・時刻の2入力を消さず、遅れて完了する入力は音声開始順へ整列。

Rust55PASS（pipeline20／worker metadata1／sink7／recovery8／source SQL2／minutes17、実Ollama1は既存ignore）、frontend8PASS（badge SSR含む）、TS／locked offline example check／diff check成功。synthetic2入力の実VAD後PCM・時刻が各独立referenceと一致、保存mix全sample一致・Stop短tail保持・2回flush非重複。real migrationの隔離SQLiteで同時刻同文の両取得元を保存→close→reopen→paginationして保持、legacy非推定も確認。初回DeviceType enum取り違えのcompile失敗は保持して修正。新経路の実ASR品質／実capture／GUI全体／長時間／連続dual負荷／echoは未検証、既存unbounded queueも残る。初回実装checkpointでは候補反映／stageなし（その後の試用承認は上記）。releaseなし。ユーザーdata／実音声／cloud不使用、旧diar実験は履歴保存。詳細：[SOURCE-TRANSCRIPTION.md](SOURCE-TRANSCRIPTION.md)。

## 最新：Linux実用版の完成を優先、本体への選択式Qwen統合を承認

ユーザー「いったん、精度はいいので、完成形にするべきでは？」を受け、ask_userで **MeetilyのLinux実用版を完成** を明示選択。精度追加実験を停止し、Qwen選択式接続／Linux内部音声／保存・表示／継続＋最終議事録／編集出力／復旧へ移る。過去の本体統合保留をこの範囲で変更する。品質合格・無断default変更・実会話録音／cloud検証・自動ASR補正の承認ではない。

最初にStop中の保存経路を修正：managerをtake・event listener解除後のASR tailを旧保存listenerが拾えない問題を、session-bound TranscriptSink＋worker直接保存で解消。startup前に保存先を捕捉、UI event前にatomic upsert。別会議への非混入・tail保持・保存失敗再試行のsynthetic3tests成功。完成版全体はまだ未達。詳細／固定完成条件：[LINUX-MVP.md](LINUX-MVP.md)。

## 前工程：ForcedAligner任意worker接続・公式比較まで実測、最新判断で採用取り下げ

ユーザーは日本語文字時刻／10〜30秒の話者別本文／相槌・重なり・誤文／3モデル負荷の4試験を承認し、悪化時は公式実装も試すよう追加依頼。`--aligner-dir`／任意`align:true`／独立alignを別buildへ接続、既定ASR・full original input・30秒上限を維持。公式0.6B固定revisionを取得しGit/LFS照合、全unconfirmed／認識未検証。文字単位版で失敗し、同じraw文を公式qwen-asr0.0.6／nagisa0.2.11で比較。短語20秒はASR文字誤り0でも話者付き代理誤り12/14→7/14、完全文22秒逆順は33/78→16/78。公式も不合格。順／30秒のASR文脱落32文字はalignmentで復元しない。誤った山に妥当そうな時刻、公式は明示無音＋はいにも0〜2秒を返す。重なりのCPP見落としで誤帰属し得る。

Native15fixture×fresh3でtext／unit一致、公式33条件×fresh2でtext／status／unit一致。Rust17／Python35／fmt／locked offline build、232artifact hash。既定alignなしの公開文＋無音は旧workerのtext／status／callsと一致。本体用binaryは据置、試験binaryは別保存。3モデルpacedは公式合算RSS11.37〜11.69GiB／各RTF<1だがcpp暫定label最大3.65〜9.11秒、3者同時計算は0。別offline backlogで23.49秒重複、RSS11.70GiB／平均8.59論理CPU相当／label12.07秒。対照も120ms最終push目標を満たさず、低遅延会議合格ではない。旧drainのCPP除外と誤命名したpaced runをaggregateで明示訂正、rawは保持。入力lane＋混合1系統diar方針を維持し、diar必須ASR gate／自動確定／公式語から文字時刻の捏造はしない。実capture／再生／cloud／app更新／DB／defaults／locks／releaseなし。詳細：[QWEN3-FORCED-ALIGNMENT.md](QWEN3-FORCED-ALIGNMENT.md)、`evidence/diarization/forced-alignment.json`。人手文字境界／自然会議／長時間／GUI全体は未検証。

## 前工程：入力別音声保持＋混合1系統diarを選択、第1段階public prototype

ユーザーは話者分離の区間をASRへ渡す構成を提案し、入力別音声を保持して混合1系統で話者分離を行う方式を明示選択。まず既存VAD／文脈を維持した隔離試験。公開2話者turns60を人工mic／PC laneへ割当、lane保存／合算元PCM全bit一致。cppQ8 low4threads＋現在realQwen6threadsでRTF0.683／0.573、最終cpp pushlag0.25ms／ASR開始待ち0.09ms未満、6realASRcall／非空／error0、cleanup成功。cpp最大暫定label遅延3.503秒で即時受入ではない。offline trace replayで既存full-input10秒windowへのspeaker候補setを付与、全6窓が2話者でsingle=null。元text／context不変、不明／重複を保持、word alignmentやASR前diar区間切出しはまだ未実装。VAD paddingのlabel不明部分をspeech missと数えない。pure23PASS。実入力capture／本体／DB／録音保存／defaults／OS／cloud／releaseは不変。製品の入力別保持時間・永続保存・区間文脈・revision／backpressure／Stop／長時間は未定／未検証。詳細：[MIXED-DIARIZATION-PIPELINE.md](MIXED-DIARIZATION-PIPELINE.md)、`evidence/diarization/mixed-pipeline-stage1.json`。

## 前工程：C++候補を選択、Qwen同時実行は60秒throughput追従・遅延増加

ユーザーは組み込み候補をC++へ変更し、「同時に実行できるかテスト」を承認。本体統合ではなく公開FLEURS反復60秒／CPUのみで単独対照＋同時fresh2runs。cpp最適化Q8 low4threads＋現在のlocal Qwen1.7B decoder-INT8 float32 JA6threads、cpp120ms供給／Qwen10秒窓6件。cpp RTF0.641／0.673、Qwen request RTF0.554／0.581（単独0.433／0.422）、cpp最終pushlag約0.3ms／Qwen最大開始待ち0.11ms未満で追従。ただしcpp最大一時遅れ2.150／2.243秒、暫定label age最大3.230／3.323秒へ増加、即時label受入ではない。実計算overlap23.58／25.88秒、raw ASR6texts／cpp segmentsは単独一致、全ASR6calls／非空／error0。sampled合算RSS約7.4GiB（PSS／leak証明ではない）。Qwen初回warmup別記録、model/cache不変、pure16PASS、owned process終了／tempWAV削除。自然会議・長時間・本体可変VAD schedule／GUI保存・minutes同時負荷は未検証。実mic／物理再生／cloud／newmodel／app統合・更新・releaseなし、保存後手動の既方針維持。詳細：[CPP-QWEN-CONCURRENCY.md](CPP-QWEN-CONCURRENCY.md)、`evidence/diarization/cpp-qwen-concurrency.json`。

## 前工程：cpp遅延の主因はNixのnative指定除外、最適化後はCPU実時間供給に追従

ユーザー「parakeet.cppが極端に遅い理由を調査してほしい」に対応。旧buildはGGML_NATIVE=ONでもNix GCC wrapperが`-march=native`を除外し、実DLLのAVX2/FMA/F16C=0だった。元build／失敗を保持し、別buildへ明示Zen3指定。共通公開PCM／同model／同low preset／CPU4threadsの60秒pacedでQ8 RTF0.536／fresh0.561／逆順0.566、F16 RTF0.462、最終push遅れ約0。Q8計算約5.2〜5.4倍、同12秒F16 prefixは約25.8倍高速化。実推論のQ8 tinyBLAS内でAVX2 `vpmaddubsw ymm`を1命令実行までGDB確認。

前回の「cppはCPU速度不合格、rsだけが候補」を訂正。Q8順proxy98.70%／再実行raw完全一致、逆順100%。F16全60秒proxy91.35%で品質合格ではなく、人手DER／自然会議／長時間／Qwen競合も未検証。旧→新Q8は4bins異なりbit-exactではない。最適化Q8の無音／tone12秒はlabel0、guard追加含むpure13tests PASS（比較7＋NVIDIA6、再開後も再確認。旧19件表記は二重計上として訂正）。harnessにcompiled feature記録／任意`--require-simd`を追加、旧build拒否を確認。本体／表示中app／DB／録音／ASR／lock／OS音声設定不変、保存後手動の既選択を維持。詳細：[PARAKEET-CPP-CPU-DIAGNOSIS.md](PARAKEET-CPP-CPU-DIAGNOSIS.md)、`evidence/diarization/cpp-cpu-diagnosis.json`。

## 前工程：CPU向け候補2engineを初回実測（cpp汎用buildの速度評価は上記で訂正）

ユーザー「それぞれ試して」を、直後の「前２つだけでよい」でparakeet.cpp／parakeet-rsに限定。共通公開float32 PCM、CPU4threads、120ms実時間供給、同じlow preset（9+4/cache264/FIFO264）で比較。本体／表示中appは変更しない。

cpp Q8は60秒RTF2.895／最大遅れ116.070秒でFAIL。F16の60秒は500秒timeout、別prefix12秒は40.125秒／RTF3.343で完了、失敗を保持。rs FP32＋CPU ORT1.23.2は60秒順／逆／公開FLEURSでRTF0.538〜0.599、最終push遅れ約0、暫定label age中央値1.08〜1.16秒／最大1.87〜2.69秒。ただし日本語短語2話者の順方向はproxy78.49%／話者2の385binsを話者1へ混同、fresh runで全segments・frame setsが一致して失敗再現。逆順98.67%で旧失敗を置換せず、人手DER／会議精度合格にはしない。

両engineの人工無音／tone12秒はlabel0、共通pure tests12PASS、独立Rust driver＋専用lockのbuild --locked成功。速度だけならrsが候補だが**本体採用／精度合格ではない**。INT8 v3／Qwen競合／実会議／長時間は未検証、保存後手動の既選択を維持。実mic／物理再生／cloud推論／app releaseなし。詳細：[DIARIZATION-ENGINE-COMPARISON.md](DIARIZATION-ENGINE-COMPARISON.md)、機械可読結果：`evidence/diarization/engine-comparison.json`。

## 前工程：NVIDIA話者分離のCPUリアルタイム可否を独立実測

ユーザー「リアルタイムで使えるかを試してみてほしい」で、公式NeMo-Speech.cpp／Nemotron3 Q8_0を5650GE CPUのみで隔離検証。120msずつ公開音声を実時間供給した標準設定は60秒でRTF1.681／最大backlog47.496秒でFAIL、短chunk320msもRTF2.990でFAILを保持。事前固定の履歴縮小候補（chunk13/cache96/FIFO40）は60秒の日本語公開音声でRTF0.626〜0.648、最終push遅れ約0、暫定frame出力age中央値約1.35秒／最大2.15秒、RSS約291MiB。

公開2話者短語を4回ずつ交互反復した順／逆順fresh試験では2匿名cluster維持、観測label変更0。source identity proxy100%は人手DERではなく、声／単語が対応した限定fixtureのみ。FLEURS proxy61.07%とRMS正解の限界も記録し、自然会議の精度受入にしない。人工無音／toneは各12秒label・segment0、pure harness6PASS。Qwen同時動作／録音・UI・保存接続／長時間・重複発話・話者忘却は未検証。

**本体は変更せず、保存後手動の既選択をリアルタイム採用へ変更していない。** 表示中app／実mic／物理再生／cloud推論／releaseなし。詳細・pins・license・全失敗：[NVIDIA-DIARIZATION-REALTIME.md](NVIDIA-DIARIZATION-REALTIME.md)。

## 前工程：120ms処理窓の隔離回帰成功、CPAL同期は部分修正・未合格

ユーザーは未完3／5の実行を依頼し、120msと有効backend時刻の不連続保持・復帰を明示選択。live二mixerを120ms／5760samplesに設定し、buffer4.8秒／wait1.2秒／exact Stopを維持。native共有人工信号60秒30/30bursts／loss・stall全0／約10ms位置差、公開FLEURS＋実local Qwen GUI PC-only／native dual／従来CPAL dualの3回帰が120ms／5760blocks／ASR・保存・DB・reopen・cleanup・warning0で成功。unit72／metrics4／TS、source20＋既知DSP2expectedFAILを確認。

CPAL callbackは時刻情報を捨てていたため接続を修正し、連続時計の不連続復帰／resamplerのunknown永久伝播も修正。しかしprivate共有信号の最終CPAL診断は全10bursts保持でもinvalid3／gap24908／位置差初回300ms→30msでFAILを保持。本体のCPAL timedは有効化せず、旧sample-orderを維持。上記旧CPAL GUI成功を同期合格としない。**5は実装済み、3は未完**。

debug/no-bundle候補`runtime/nixos-window-120ms-candidate`（`r993k…`）stage／binary一致・dev復元確認。表示中appは不変、実mic／物理再生／cloud／releaseなし。次のCPAL課題・全失敗・確認範囲：[CPAL-WINDOW-120MS.md](CPAL-WINDOW-120MS.md)。

## 前工程：選択式native Pulseマイク＋収録時刻配置、隔離60秒／GUI回帰成功

修正の継続で、CPAL／既定入力を残して`pulse-input:<source>`を明示選択式で追加。native選択時だけbackend read-head時刻をセッションsample位置へ配置し、起動未収録範囲はsilenceとして保持。unknown時刻をarrivalへ置換せず、600ms窓／1.2秒wait／late discard／exact Stopを維持。CPAL timestamp診断のFAIL（unknown625/653/617）は保持し、live CPALをtimed mixerへ自動移行しない。

一つのprivate人工再生を両native入力で取得：20秒10/10bursts、60秒30/30bursts、late/gap/overlap/invalid/overflow/stall全0、位置差約10ms／60秒で観測増加なし。入力remove/error/no fallback/明示reconnect/timed callback/StopもPASS。初期lifecycle harness FAIL（JSON index欠落、idle source）を保持して試験側修正。

公開FLEURS−20dB＋private zero mic＋実local Qwenの本体GUIはPC-onlyとfresh native dual3runsでPASS。UI選択保存／browser reload／native再起動、ASR非空、event/raw/SQLite一致、completion／cleanup、stall0／late0／error-events0。native66／metrics4／picker6／TS成功、source18PASS＋既定DSP2expectedFAILを維持。精度・実二入力・multi-hour／hardware clock同期の合格ではない。

embedded debug候補`runtime/nixos-timed-sync-candidate`（`gal613…`）をstage済み、built/staged一致とdev復元hash一致を再確認。表示中5wh previewは不変。更新は保存完了・閉鎖確認を得てからで、nativeマイク選択は明示的に行う。実mic／物理再生／cloud／releaseなし。詳細・失敗・未検証境界：[DUAL-LATENCY-REPAIR.md](DUAL-LATENCY-REPAIR.md)。

## 前工程：PCの秒単位配送遅延を改善、残る起動時Mic供給差を特定（二入力全体は未完）

ユーザーが入力UI成功を報告後、二入力遅延の計測・比較計画を承認。private人工burstの同一native A/Bで、従来PC配送gap最大約1.89秒／欠落1burst／stallを再現。ADJUST_LATENCY＋100msでは20秒＋60秒3回、全bursts保持／PC gap最大102.71ms／stall0／late0、位置差−80msは60秒で増加なし。private pause/resume／source削除／明示reconnectも成功。通常Pulse startをこの設定へ変更し、native47＋metrics4等PASS。

ただし実Qwen＋公開fixture＋private GUIではPC-only PASSに対し、dualは3runsともstall1／late4978・8879・2749でFAILを保持。PC buffer latencyは541〜708µsまで低下。source別numeric計測で初回handoffがPC0.663秒／Mic1.917秒、初回mix2.465秒だけdegraded、遅着2749は全Micと判明。handoff＝収録時刻とは断定せず、起動時のsource時刻モデル修正が残る。保存／UI再起動／ASR非空／DB一致／cleanupは成功だが全体合格ではない。

新embedded debug候補2つをstage、dev binary復元。表示中の既存appへは未反映。実mic／物理再生／cloud／releaseなし。次の具体的時刻契約／startup録音範囲は設計decision gateへ。詳細：[DUAL-LATENCY-REPAIR.md](DUAL-LATENCY-REPAIR.md)。

## 前工程：PC取得fragment100ms指定、仮想dual3回警告0・遅延診断はFAIL保持

ユーザー明示承認でPCのみfragsize19200bytes（mono48k float32／100ms）を指定。他属性server既定、ADJUST_LATENCYなし、Mic／OS source latency／1.2秒wait／ASRは変更なし。native46PASS、source18PASS＋既定DSP2expectedFAIL。UI＋norm＋recovery＋fragmentのembedded debug候補`runtime/nixos-input-fragment-candidate`（`5whvry…`）をstageしdev binary復元。

private公開FLEURS−20dBのPC-only＋独立dual3runsでfragment実値19200、stall0／late0／error events0、UI保存・再起動／ASR非空／保存・DB一致／cleanup成功。ただし再生2秒後buffer latencyは1.621秒のままで、新diagnostic<1.2秒条件は全4FAIL（exit1）を保持。fragment一致と短時間warning0を確認したが、遅延低下／精度／実二入力安定は未受入。その後ユーザーの保存完了／閉鎖確認を得て同じpreviewへ新候補を反映。選択済みEchoCancel機器がOS側に不在のため初回strict起動は安全停止し、missing表示＋Start disabledを要求する明示UI-only modeで再起動。新binary／embedded／recording=false／owned capture0確認、別機器へのfallback／OS変更／自動録音再生／releaseなし。詳細：[PC-CAPTURE-FRAGMENT.md](PC-CAPTURE-FRAGMENT.md)。

## 前工程：復帰の永久破棄を修正、PC側の既定取得buffer遅延を確認

ユーザーが復帰修正を先に選択。degradedが将来区間の待機をbypassするbugを修正し、区間毎1.2秒wait／episode毎warning／late破棄／exact Stopを保持。追加3production testsは旧guard復元で3FAIL→修正3PASS、native43PASS、source18PASS＋既定DSP2expectedFAIL。UI＋PC norm＋fixのdebug候補`runtime/nixos-input-recovery-candidate`（`kdwnlh…`）をstage、dev binary復元。

public private PC-only PASS、dualも一度PASS。ただし独立repeatはstall3／late16899でFAIL、別diagnosisも警告が残る。再生中のprivate serverでPC buffer latency1.620937秒（Mic0.000240秒）を計測、1.2秒wait超。PC Pulse APIはbuffer attrs=NULLでserver既定に依存。paplayだけ50msにしたcontrolもFAIL、結果を隠さない。PC取得fragment指定は未変更・次のdecision gate。UI保存／再起動、ASR非空／保存／SQLite／cleanupは成功だが同期安定／精度合格ではない。表示中preview未変更。詳細：[INPUT-RECOVERY-FIX.md](INPUT-RECOVERY-FIX.md)。

## 前工程：録音画面に入力機器選択を常設、UI保存／再起動成功・二入力同期は未合格

ユーザーはマイク／スピーカーのUI選択を依頼し「録音画面に常時表示」を明示選択。日本語の選択欄、設定を保持する直列自動保存、録音／処理中ロック、missing選択の非自動変更とStart guardを追加。スピーカーはPC音声capture対象でOS再生先を変えない。LinuxはALSA mic一覧失敗時もPulse monitor一覧を返す。TypeScript／隔離frontend export／debug candidate／frontend9＋native40tests成功。

private公開FLEURS−20dBでDOM選択→保存→browser reload→Start中lock→Stop→native再起動のUI検証はPC-only／dualともPASS。ただしdual全体はrecording-error1／stall1／late discard420018samplesでFAIL（初回も418095samples）。保存・ASR非空・event/raw/DB・cleanupは成功だが同期／精度合格とはしない。既知degraded recovery問題と整合、破棄sourceは不明。

debug候補`runtime/nixos-input-ui-candidate`（`z24swrr…`）をstageしdev binary復元確認。表示中の古いsync previewは未変更。実mic／物理再生／cloud／releaseなし。known recoveryの修正優先かUIのみ反映するか、ユーザー決定を待つ。詳細：[INPUT-DEVICE-UI.md](INPUT-DEVICE-UI.md)。

## 前工程：PCにも上限付き自動正規化、公開fixtureの元音量／−20dB検証成功

ユーザーがPC正規化を依頼、方式選択で−23LUFS／増幅上限＋20dB／smooth／zero保全／sample peak制限を明示承認。新pc_normalizer.rsをcapture mix前に導入しmic処理は維持。native10＋pipeline12＝22PASS。新debug candidateをbuild/stageし、元dev binary復元。private仮想入力の日本語fixture元音量／PC−20dB／virtual dual＋PC−20dBの3runsでASR nonempty／保存・DB一致／cleanup成功。実mic・speaker・normal profile・visible旧画面は触れず、旧画面は未反映。sample peakはtrue-peak保証ではなく、意味精度未測定・既知同期復帰bug未修正。releaseなし。詳細：[PC-AUDIO-NORMALIZATION.md](PC-AUDIO-NORMALIZATION.md)。

## 最新：実micを使わず、以前の公開日本語fixture＋仮想音声で自動確認

ユーザーが自分のmic録音の再利用をやめ、前のテスト音声＋virtual speakerを希望。公開FLEURS日本語と、完全隔離仮想入力に限るagent自動Start/再生/Stop/auditを明示承認。新scriptのprivate Pulse/null sink＋fresh XDG/Xvfbで11.1秒fixtureを確認。PC-only2segments／仮想dual（micは無音）3segments、保存・raw/event/SQLite一致・cleanup成功、stall0/late0/errors0。実mic／実speaker／host出力設定／visible profileに触れない。既知の復帰bugは未修正で今回のscheduleでは発生せず、意味精度/CER未測定。今後はこの公開fixture＋隔離仮想経路を使用。詳細：[VIRTUAL-PUBLIC-CHECK.md](VIRTUAL-PUBLIC-CHECK.md)。

## 最新：修正版実テストは文字起こし0、同期timeout後の復帰バグを人工再現

ユーザーは保存完了後も空と報告。最新14.358604秒の保存はnonzero／decode warning0／completion・DB・cleanup正常だがASR queued0。input stall1、遅着606208samples破棄（source identity不明）。production mixing.rsの人工到着順回帰で、degraded modeが次の区間も待たず即無音補完し、戻った入力を破棄し続け得るバグを再現。音量だけを原因とせず、修正・人工回帰を優先。新capture/playback／設定変更なし、品質不合格。詳細：[DUAL-INPUT-CHECK.md](DUAL-INPUT-CHECK.md)。

## 最新：修正版local debug候補を反映、同じprivate dual画面でREADY

ユーザー「修正を反映して」を受けembedded debug/no-bundle build＋Nix stage成功。新root `../runtime/nixos-dual-sync-candidate`、store `v907pl178by5zqbkl357fhjrhngr61fx-meetily-local-candidate`。旧候補保持・dev binary hash一致で復元。ユーザー保存完了／旧画面終了を確認後、残存owned preview helperだけcleanupして同じprofile :0/4453へresume。embedded origin／is_recording=false／新native実在を確認、自動Start/Stop／再生なし。修正後の実録音・意味精度・GUI保存新回帰は未実施。次は固定音量で同じ承認済み短PC音声のユーザー操作再テスト、報告＋non-content audit。releaseなし。詳細：[DUAL-INPUT-CHECK.md](DUAL-INPUT-CHECK.md)。

## 最新：同期＋Stop末尾を修正、native人工テスト28件成功

ユーザーが同期＋Stop-tail修正と、1.2秒有限待機→警告＋無音補完／遅着sample破棄を明示選択。新audio/mixing.rsのsample位置ring、selected-input情報伝達、pipeline共通保存/VAD経路、sample timestamp、queued audio完了後exact tail→VAD flush、callbackなしtimeout、late/overflow warningを実装。native ring11＋pipeline10（実VAD・音を出さないeSpeak合成fixture、ASRなし）＋既存VAD7＝28PASS。source regression15PASS、未変更の飽和／alias診断2FAILは保持。初回Tokio runtime不足・positiveにならないtone fixtureの失敗もログ保持。candidate更新／実録音再生／Qwen認識／GUI新保存回帰／releaseは未実施。現在のvisible windowは旧候補のまま、精度改善を主張しない。詳細：[DUAL-INPUT-CHECK.md](DUAL-INPUT-CHECK.md)。

## 最新：人工データでRust処理を直接検証、4点の懸念を再現

ユーザー「検証して」。新script `scripts/verify_dual_audio_code.py`でproduction ring/mixer/resample/Stop-flushを変更なし抽出しRust test実行（Stop外部依存はmock、full app統合ではない）。挙動確認8PASS／診断目標4FAIL：遅延source600ms→出力1200ms、Stopに500ms ring残留、加算hard saturation、12k→4k alias−13.979dBを再現。初回alias期待値の符号ミスはテストだけ修正、失敗log保持。script exit0は失敗4点の再現成功であり製品合格ではない。原因の条件付き挙動を確認したがユーザー誤認識との因果は未確認。本体修正／録音再生／ASR／candidate更新／releaseなし。詳細：[DUAL-INPUT-CHECK.md](DUAL-INPUT-CHECK.md)。

## 最新：追加録音なしのコード調査でmix／resample／Stop-tailの懸念を特定

ユーザーは「追加録音せず、コード経路を調べる」を選択。Linux Pulseはmono48k floatを本体へ送り、VADの5点移動平均で16k化後Qwen IEEE float入力へ渡す。実600msのOR-triggered ring mix（時刻整列なし）、Stopで未mix ring tailをflushしない、100% system加算とhard saturation（comment不一致）、48k→16kの高域減衰／alias残留をコードで確認。標準ライブラリによる人工到着順モデル／解析はproduction Rust試験ではなく今回の原因確定でもない。新規録音再生・入力切り替え・本体修正なし。次候補は人工データでproduction回帰検証。詳細：[DUAL-INPUT-CHECK.md](DUAL-INPUT-CHECK.md)。

## 最新：固定音量でも精度はほぼ不変、再テスト保存監査は正常

ユーザー回答「ほぼ変わらない」。private dual profile最新保存は24.6秒mono48k／636379bytes、decode warning0、raw1segment非空、両device／completion／SQLite本文exact＋時刻1ns／cleanup正常、read-only non-content audit passed=true。保存異常はこの監査で検出されなかったが、独立source／ASR入力歪み／意味品質の証明ではない。録音再生の自動開始・本文export・model/gain/VAD/default変更・releaseなし。次候補のPC-only切り分けはユーザー選択待ち。詳細：[DUAL-INPUT-CHECK.md](DUAL-INPUT-CHECK.md)。

## 最新：dual入力は動作報告、単語／文章の誤認識があり精度は不合格

ユーザー「精度は悪いができた」、症状選択「単語や文章を別の内容に誤認識する」。local non-content auditは15.6秒mono48k warning0、両device保存／completion commit、raw1segment本文とSQLite一致・cleanup正常。times14.350000000000001 vs14.35のbinary64差のみを1ns toleranceとしてauditor修正しpassed=true、ASR補正ではない。ja設定enumとadapter Japanese mapping／float32 mono16k契約を確認。AAC abs>1は2/748800samplesで元ASR clipの証明ではない。HPF＋normalization／mono mix一Qwen、RNNoise=false。意味品質の合格にせず、次はどちらのsourceで語誤りが出るか既知文で切り分け。release／model/gain/VAD/architecture変更なし。詳細：[DUAL-INPUT-CHECK.md](DUAL-INPUT-CHECK.md)。

## 実mic一台＋PC再生音の同時入力を明示選択、確認画面READY

ユーザーは会議の自分／相手双方を認識したいと依頼、方式選択で「マイク一台＋PC再生音」を明示選択。EchoCancel_Mic＋EchoCancel_Speaker.monitor、local Qwen／ja、手動Start/Stop、releaseなしを維持。新private dual-preview-spocggm1、window「会議入力確認 / マイク＋PC音声」、port4453。既存debug候補の両入力preferred deviceを設定、選択monitor列挙・READY/is_recording=falseを確認。前の承認済み7.2秒micテストのUSER-INITIATED PC再生commandを準備しただけで、再生／録音を自動開始していない。現在はユーザーによるPC発話→別mic発話の確認待ちで、dual認識成功はまだ未確認。existing UI meterはfake sine dataなので証拠に使わない。詳細：[DUAL-INPUT-CHECK.md](DUAL-INPUT-CHECK.md)。通常profile／gain/VAD/model defaultsは変更しない。

## 実micの短時間capture／認識／保存を確認、releaseは作成しない

ユーザーがEchoCancel_Mic／日本語の手動テストに「できた」と報告。承認済みprivate profileだけをローカルread-only auditし、7.2秒mono48k音声のfinite/nonzero・decode warning0、completion commit／system_audio=null、非空raw1segmentとSQLite text/audio時刻一致、ASR input削除／checkpoint cleanupを確認。音声や発話text／native logsをtool/cloud contextへ出さず、non-content metricsのみを取得。selected実mic→local Qwen→Stop→保存の短時間確認であり、精度・全mic／長時間・native restart-reopenの受入ではない。release/build/stageなし。詳細：[MIC-CHECK.md](MIC-CHECK.md)、log `../logs/meetily-mic-save-audit.log`。

## 最新指示：release作成を停止、実mic確認へ（初期準備時点）

ユーザー「まだリリースは作らないで。micで使えるかどうかを確かめて」。入力はEchoCancel_Mic、言語は日本語を明示選択。実会議／desktop／cloud/authへの許可ではなく、限定テスト発話のマイク確認。既存WAV修正済debug候補を使い新build/stageはしない。

新private profileとprivate ALSA Pulse bindingでEchoCancel_Micだけへ接続、本体system monitor側は存在しないsocketへ分離。device enumerationはdefault/meetily_micのInputのみ、Outputなし。visible title「Meetily — マイク確認 / EchoCancel_Mic / 日本語」、READY/is_recording=falseと当該nativeのhost input stream0を確認。自動Start／合成再生／実音声保存／cloud送信は行わず、ユーザーのStart→日本語発話→Stopを待つ。usable samples/認識/保存の成功はまだ未確認。実音声／発話text/nativeログをそのままtool出力・cloud contextへ送らず、ローカルmetricsとユーザー報告で確認する。[MIC-CHECK.md](MIC-CHECK.md)。

## checkpoint継ぎ目を修正：float32 WAV→最終AAC一回

ユーザーが時刻警告の切り分け・修正／回帰を承認し、保存方式としてfloat32 WAVを明示選択。旧AAC checkpointのcopy-concatは約60秒合成試験で個別decode合計より4368samples長く、先頭priming1024samples追加。MP4 movie edit-listの1000Hz丸めによる端数tail17samplesも確認。警告だけの問題ではなかった。

新checkpointは元float全bitをWAVへ保持、停止時に連続sample PTSでAACを一度encode、movie timescale=sample_rate。raw/ASR/model/default/VAD/correctionは変更しない。過去のMP4 recordingは書き換えない。legacy-only明示復旧はstream-copy互換をpartial／timing注意として維持、mixed/duplicate/gapはaudio mergeで拒否・原files保持。WAV/recovery read-only detectionも接続。

実nativeの3checkpointと全sourceを一度encodeした参照のdecoded bytesがFFmpeg7/9×48k/16kで全一致・警告0。FFmpeg9は元2892017samplesと一致、7は末尾paddingのdecoder差が残る。native25／monitor4tests、fixture通常・worker crash・save failure、実Qwen602.585秒/40bursts/47raw segmentsのStop保存／restart回帰成功。新long PCM warning-level auditも0。Stop待ち9.923秒（final encodeの費用含む）。NixOS embedded-debug候補を同じ修正へ更新し71秒複数checkpoint／strict decode／restart確認。user app/dev serverを止めずdev binary復元。releaseや多時間品質の完成ではない。[CHECKPOINT-TIMING.md](CHECKPOINT-TIMING.md)。

## 10分合成録音／NixOS standalone候補（部分検証）

ユーザーはvisible previewに「問題なさそう」と報告し続行を承認。配布対象はまず手元のNixOSを明示選択。実Qwenで602.498秒／40合成bursts／57raw segments、Stop tail・全event/disk/SQLite一致・native再起動とhash不変まで確認。worker PID1つ、temporary WAV最大1（queue長そのものではない）、public録音／ASR失敗eventsなし。worker plateau RSSほぼ一定、native medianは約25MiB増加し、leakなしとは断定しない。

decode raw出力でDTS警告14件を保持。元MP4のffprobeではPTS/DTS非増加・packet重なり0だがsample境界／警告原因は未解決。一般の多時間音声受入ではない。

custom-protocol embedded UIのNixOSローカル候補をNix storeへstage、LP64 BLAS/ORTをclosureに明示保持。開発library環境なし／tauri://localhost origin必須で、人工minutes編集・restart・実exportsと実Qwen合成録音・restartの両回帰成功。nix-shellなしのpackaged synthetic launcher READYも確認。debug build／external model／同host限定で、release-size／clean host／一般Linuxは未完。dev server/.nextとvisible user previewは維持、defaults/locks/models/VAD/correction/cloud/auth/実mic-desktop不変。詳細・起動：[LINUX-STABILITY-PACKAGING.md](LINUX-STABILITY-PACKAGING.md)。

## ユーザー操作用プレビューをvisible native起動（合成のみ）

ユーザー「いったん完成形をこちらで確かめたい」に対し、合成音声モードを明示選択。`scripts/preview_synthetic.py` を追加し、user display :0／新規HOME-XDG／private D-Bus-Pulse／無効ALSAで実nativeを開いた。人工minutesを表示、実Qwenとenを隔離profileだけに設定。Startはユーザー操作のみ、その後helperが固定eSpeak英文を専用sinkへ一度再生する。通常設定・実mic/desktop／cloud/LLM/authを使わない。READYと表示確認済み、ユーザー操作／録音成功はまだ未報告。実用版全体の完成・品質受入ではない。操作／再開：[USER-PREVIEW.md](USER-PREVIEW.md)、log `../logs/meetily-user-preview.log`。アプリを開いただけで録音していない。

## 実Qwen：合成音声→native録音／停止／保存／再起動（接続確認）

ユーザーの続行承認で、fixed fixtureを実1.7B decoder-INT8へ置換するモードをGUI testに追加。実workerはproxyなしで直接起動、private Pulse/eSpeak英文の全再生→推論pending中Stop→manager除去後tail→raw event/disk/SQLite全segment一致→native再起動で同meeting再閲覧を新規隔離profile3回で確認。音声decode／長さ、worker終了、一時WAV削除、録音files/checkpoints hash・SQLite ID/text/times不変を確認。model/default/VAD/correctionや依存lockは変更しない。

原稿8.656秒、全再生版Stop待ち6.157／6.230／4.538秒（load/recognition/drain/save含む、性能保証ではない）。raw冒頭やsegmentationは変動し、誤認識を補正せず保持。精度追試・品質合格ではない。fixture故障3回帰も成功。実mic/desktop／会話／cloud/LLM/authなし。詳細：[REAL-QWEN-GUI.md](REAL-QWEN-GUI.md)、log `../logs/meetily-gui-real-qwen-verified.log`。次は長時間運転／queue・memory／配布package。全体完成はまだ未達。

## native録音GUI：開始→Stop tail→保存→再起動／障害（合成確認）

専用private Pulse server（null sink／Unix protocolのみ、hardwareなし）へeSpeak人工英文を再生。隔離HOME/XDG・無効ALSAで、Home開始→本体VAD→固定応答Qwen-protocol fixture-worker→Stopでglobal manager除去後のtail→audio/raw transcript／SQLite保存→native再起動→同meeting再閲覧が成功。ASR／LLMモデル・cloud/auth・実mic/desktopは使わない。通常／worker exit7→後続burstでfresh PID／completion metadata保存失敗の3シナリオが成功。音声decode・temporary cleanup・worker終了・再起動でfile/checkpoint hash不変を確認。

GUIが見つけたQwen key lookup拒否、ALSA phantom default mic、ASR errorでnative捕捉中にUIだけpost-stopへ進む問題を修正。active errorは継続警告としStopを維持。Stopの認識不完了とfile-save成功は別扱い。保存失敗ではcheckpoint保持、故障注入先は**試験側で手動修復**後の再閲覧であり自動復旧ではない。native追加1／既存43passed＋real Ollama1ignore、frontend24＋独立hook2、TypeScript／production build成功。詳細：[RECORDING-GUI.md](RECORDING-GUI.md)、log `../logs/meetily-gui-recording-tail-final.log`。実ASR modelを通す録音全体／長時間／hard crash・power loss／配布版は未検証。完成条件はまだチェックしない。

## native GUI：再登録→編集→再起動→JSON／Markdown保存（合成確認）

Linux/Xvfb・新規HOME/XDG・専用D-Bus・無効Pulse/null ALSAで、人工1segmentと議事録を未登録folderに用意。Homeから明示再登録→Meeting Details→本文編集→revision2保存→native app再起動→同meeting再閲覧→隔離DownloadsへのJSON／Markdown実保存を確認。元item/evidence IDとbefore/after履歴を保持、raw metadata/transcriptsはbyte不変。ASR/LLM/auth/実収録は実行していない。

`frontend/tests/gui/minutes_native_smoke.py`、logs `../logs/meetily-gui-minutes-final.log`（再実行成功）、`meetily-gui-minutes-resume.log`。WebKitWebDriverのsplit-pane座標クリックは不安定だったため、ボタンは実DOM `.click()`、本文はWebDriver入力、IPC/SQLite/files/downloadは実native。物理ポインター操作の保証ではない。初回の無反応は既存dev serverのCSS/JS 404で、サーバー再起動後解消。再起動waitも旧本文ではなく固定evidenceを待つよう修正。native downloadの未検証をこの限定フローだけ更新し、実録音Stop全体・競合UI・hard crash/power loss・長時間・clean-host配布は未検証のまま。

## ディスク会議の検出・明示再登録（接続／合成回帰確認）

default recording rootの未登録foldersをread-only／bounded scanし、利用者が選んだmetadata/transcriptsだけSQLiteへtransactionで再登録するnative commandsとHome panelを追加。元files／canonical minutes／user edits／archivesは変更しない。fingerprint変化は拒否、同native requestの繰返しはidempotent、segment SQL IDsをsession namespaceで分離。保存済meeting IDからnotesを再閲覧できる。

SQLite close/reopenで元speech／revision／user editを確認し、元全file byte一致。DB fault trigger→rollback→retry、corrupt/stale拒否、archives非昇格、pagination／size bound／symlinkも確認。旧IndexedDB recoveryの現在folder推測・binding消去・checkpoint cleanup、startup age-based purgeを停止。旧audio mergeは既存file非置換／temporary no-clobber／timeoutに変更。DB open failure時のWAL/SHM削除retryも撤去して元errorを保持する。

native recovery7＋incremental5／sink6／minutes17／transcription8／DB policy1＝44passed、既存real Ollama1ignore。frontend新4＋旧hook保全2＋既存18、cargo check／TS／production build成功。人工文／tmpdirs／SQLite／mock/SSRのみ、実収録・推論・cloud・資格情報アクセスなし。詳細：[RECORDING-RECOVERY.md](RECORDING-RECOVERY.md)、logs ../logs/meetily-recovery-final.log／meetily-recovery-frontend-preservation.log／meetily-recovery-preservation-final.log。全GUI／hard crash・power loss／corrupt DB repair／audio-only orphan／diskとIndexedDBの差分統合・duplicate-aware旧save／長時間minutes・packagingは未完。完成条件の復旧全体はまだチェックしない。

## 録音保存失敗の通知・checkpoint保全（部分完了）

開始時に保存先が作れない場合はcapture前に拒否し、同名・同時刻folderの再利用をUUIDで防止。停止時はaccumulator queueを完全にawaitしてからfinalizeし、保存失敗を握りつぶさずrecording-stoppedのsave_succeeded / save_errorとpersistent UI warningで示す。checkpoint／結合audio／metadataのtemporary保存を保護し、transcriptsとcompletion metadata保存後だけ正常終了cleanup。audio saving無効でも停止時transcripts／completion metadataを再保存する。

人工PCM zerosとtmpdirsでqueue1000件、checkpoint失敗→警告／buffer保持→修復、同名folder非上書き、結合失敗時の元音声／checkpoints保全、apostrophe pathを確認。初回reserved empty tempfileが既存encoderのoverwrite拒否でBroken pipeになり、private tempdir新規outputへ修正。Rust sink6／incremental3／minutes17／transcription8（real Ollama1ignore）、frontend18、cargo check／TS／production build／diff check成功。実収録／推論／cloudなし。詳細：[RECORDING-SAVING.md](RECORDING-SAVING.md)、log ../logs/meetily-save-drain-final.log。再起動時のorphan一覧・DB再登録／crash復旧・長時間disk failure／完全hung checkpoint encoder・native GUIは未完／未検証。

## ユーザー編集の保護・出力・再閲覧（ローカル接続／合成回帰確認）

`live_minutes/document.rs` にsession/保存済meeting ID経由のread/edit/export commandsを追加。本文・状態・担当・期限を編集し、ID／分類／元evidenceは維持。expected revision conflictは拒否、入力中draftはAI props更新や再読込で保持。ユーザー編集は項目全体を保護し、before/after履歴をcanonical optional `user_edits` に保存。AI応答はvalidation/repair前とcommit時の両方で保護を復元し、推論中に保存された編集も失わない。新発言による保護項目の状態更新も自動では行わず、UIでmanual reviewを説明する。

short writer lockを推論と分離し、immutable/no-clobber archives、canonical→Markdown projection failureの明示、再export修復を実装。既存schema1は互換、破損／未知schemaは消去せず拒否。Meeting DetailsからSQLite folder_pathで再閲覧でき、session registryなしで読む。JSON/Markdownは同一canonical revisionからローカル出力（未保存draftは含まない）。Blob downloadは既存app方式で、native保存先のGUI実動作は未検証。

minutes17（実Ollama1ignore）＋既存transcription8／sink3、frontend document3＋既存control/routing13、cargo check・TypeScript・production build成功。人工文／tmpdirs／in-memory SQLite／SSR／mock DOMのみ、録音・推論・cloud／資格情報アクセスなし。log: `../logs/meetily-minutes-edit-final.log`、詳細：[MINUTES-EDITING.md](MINUTES-EDITING.md)。次は保存失敗・再起動復旧／orphan recoveryと全体GUI操作。長時間incremental・native download・未保存SPA/native-close guard・clean-host packagingは未検証／未完。

## Stop tailを含む最終議事録接続（synthetic-only確認）

録音startupでsession UUID＋TranscriptSinkを固定し、manager除去後も末尾evidence／folderを同じsessionから読む。LiveMinutesPanelのStop遷移での無条件disableをやめ、明示有効化したsessionだけdrained event後に最終passをqueue。継続モデル要求が実行中なら待ち、provider/model/accountを固定する。Stop本体はLLMを呼ばず、モデル失敗／usage limitは議事録だけpause。手動の最終草案再試行を追加。新session ID不一致はrequest前／保存前に拒否。

Stop timeoutでJoinHandleがdetachされる問題も修正。owned handleをabort＋await、dispatcher内workerはJoinSet所有。認識／保存失敗をcompleted countとは別のboolで報告し、未完tailはautomatic final禁止＋warning。start/Stopのengine lifecycleを直列化してtask登録前のStop raceを防止。未知confidenceを旧85% fallbackから-1 sentinelへ修正し、UIに捏造割合を表示しない。

Rust minutes10＋transcription8成功（既存実Ollama1はignore）、frontend runner7＋confidence1＋provider routing5、cargo check／TypeScript／production build成功。synthetic evidenceでlive→Stop tail撤回→revision2/JSON/Markdown/archive保存と、同一evidenceでモデルcallなしのprojection復旧／stale result非保存／incomplete final要求前拒否を確認。mock controlで最終queue・取消・usage pause・bounded retriesを確認。録音／ASR／Ollama/cloud推論・資格情報アクセスなし。log: `../logs/meetily-final-minutes-final.log`。詳細：[FINAL-MINUTES.md](FINAL-MINUTES.md)。実GUI全体／長時間incremental（48k bytes上限）／意味品質／編集export・再閲覧／再起動復旧は未完。次はユーザー編集の保護・保存／出力／再閲覧。

## 本体Qwen adapter接続（Linux opt-in、全体完成は未達）

`qwen_provider.rs` をengine選択／録音開始前のmodel・明示language検証／Stop時shutdown／Tauri commandsに接続。`QwenModelSettings.tsx` に絶対local paths、JA/EN/TH選択、保存・handshake後のprovider選択と失敗表示を追加。default/Whisper/Parakeetは維持。mono16k float32一時WAV、bounded JSONL、request ID/confirmed:false検証、timeout/crash/cancellationでkill＋削除、次requestのfresh起動（失敗audio再送なし）。30秒超のburstは全sampleを連続requestに分割して改行連結。raw誤認識は削除／補正せず、文字起こし画面にAI草案の注意を表示。

Rust adapter5 + 既存worker2 + sink3 + minutes6 tests成功（実Ollama1は既存ignore）。frontend provider routing5 tests、native cargo check、TypeScript、pnpm10 production build成功。実workerをapp adapterから生成5秒無音JA/EN/THで処理しempty outputとprivate temporary file cleanupを確認。最初は古いworkerのfloat32 ready欠如を正しく拒否、frozen/offline release再build後成功。これはmodel lazy-load前のhandshake/無音testであり、実speech／録音／全GUI操作／品質受入の検証ではない。依存lock変更・capture/playback/cloud／資格情報アクセスなし。logs: `../logs/meetily-qwen-integration-final.log`, `meetily-qwen-adapter-smoke.log`, `meetily-qwen-worker-build.log`。導入と未完範囲：[QWEN3-ASR-SETUP.md](QWEN3-ASR-SETUP.md)。次はLinux monitor captureとsynthetic sink試験。stop timeout lifecycle、最終議事録、編集export/reopen、復旧は未完。batch Qwenはまだ未対応。

## Linux native monitor接続（synthetic capture成功）

公開libpulse C APIでnative PulseAudio／PipeWire monitorをdiscoveryとAudioStreamへ接続。monitor metadata確認＋`pulse-monitor:` identity、default sink monitor解決、mono48k float32→既存AudioCapture/channel、mic／Windows CPAL維持。source移動禁止、bounded startup／nonblocking polling＋thread Stop、server/source失敗はrecording-errorへ。LinuxでALSA playbackをsystem capture候補に混ぜない。Cargo/pnpm lockは変更なし。Nix runtime libraryとDebian `libpulse0`依存を追加。

専用null sinkへ生成440Hzのみ再生するexampleで、native discovery・app channel・start/stop2回・pause/resume・monitor削除エラー・missing source拒否・同名専用sinkの明示再作成/reconnect・cleanup成功。実mic／desktopは開かず、ASR/cloudなし。初回はPipeWireでsource削除後streamがREADY/suspendedのまま残り、error検出が失敗。source存在を1秒ごとに確認する修正後に成功。pulse2＋既存transcription7＋sink3tests、cargo check成功。録音全体／WAV保存・ASR・最終議事録のend-to-endや長時間・server restart・配布packageは未検証。導入：[LINUX-SYSTEM-AUDIO.md](LINUX-SYSTEM-AUDIO.md)。log: `../logs/meetily-pulse-final.log`。次は最終議事録と編集・出力・復旧。

## 前工程：同じ1.7B INT8のASR入力診断、float32で静音改善・短語失敗は残る

ユーザー「ok」で別exampleによるPCM16追加量子化／float32保持のASR診断を承認。model/worker/default/appは据え置き、ASR gainなし。固定VAD .7/peak100のmaskとknown-public-speechの診断ASRを分離し、gated scoresはprojectionとして明示。182窓×2fresh processes（各117実ASRcalls）、negative65はASR0。旧車／本noiseの同一PCM16 signalも4窓×2でrecheck。text/tokens/gate/review/calls全再現、baseline FLEURS30 textは全一致、全unconfirmed。

追加−40dB英語raw WERはPCM16 **41.14%→float32 10.86%**（gated projectionではPCM16 49.71%）。日本語CER3.02→2.64%、タイ語14.97→8.48%。VAD33/33 vs29/33。ただし33pairsは改善14／同点16／悪化3。`いいえ` padding→`家。`、quiet/noise→`いい。`、旧`本`noise→`ほう。`は両形式で残る。新seedの成功で旧失敗を置換しない。真のsource holdout・meeting品質受入れではない。非量子化model比較／補正実装／cloud／統合はしない。詳細：[QWEN3-ASR-ASR-INPUT.md](QWEN3-ASR-ASR-INPUT.md)、`evidence/qwen3-asr/worker/asr-input/`。

## 前工程：承認された1・2完了、公式wrapper一致／音量とPCM16量子化を分離

ユーザーは「1,2だけ」と明示承認し、認識後補正も考慮するよう追加依頼。公式pinned Python/Torch wrapperを変更せず実行し、343windows×3gains=1023sequences/296,667framesで同じORT wheelのRustとbit一致。過去Nix ORTとの差は最大2.5034e-6、事前tolerance1e-5内、.5/.7の全gate intervals一致。2pass再現。初回Nix Python依存setupはsource build600s timeout、独立CPU wheel venvへ切替しruntime差も分離して照合した。

VAD gain5profilesを事前固定、threshold .7/>90msを維持して575windowsで比較。既存343でmiss6→gain100で4、gain300で3、slow RMSでは7。全profile未解決。33sourcesの追加-40dBではfloat32保持+gain100で33/33検出、再PCM16量子化では29/33。-60dBでは7sourceがPCM16で完全zero、増幅で復元できない。新100変形条件も含め、negative65窓は全profile FP0だがreal noise/会議保証ではない。2passの全gain arraysはbyte同一。

詳細：[QWEN3-ASR-VAD-INPUT.md](QWEN3-ASR-VAD-INPUT.md)、`evidence/qwen3-asr/worker/vad-input/`。ASR推論0、提案3の非量子化ASR比較・補正実装・cloud送信・worker/model/default/app採用は行わない。補正はrawと候補を分離し、否定/数値/決定の意味変更を自動確定しない方針を考慮する。過去ASR短語失敗を撤回しない。Rust17/Python18/Node14+306endpoint reference、fmt/compile成功。

## 前工程：固定VAD候補は追加254窓で不合格、quiet speech/短語誤りが残る

ユーザーのcontinueでpeak20/.7/>90msを推論前に固定し、前回noise変形未使用のFLEURS26clips×8=208speech、新2speakerの車/本×6=12speech、新synthetic negative34窓を検証。26clean clipsは元gridでも見ているため、完全source holdoutではない。固定候補はmiss6/FP0、現行ONNX/.5はmiss5/FP0、既存v4/.5はmiss1/FP3。候補のquiet EN5/TH1（全-40dB）はv4なら検出。再調整/失敗除外/本体採用はしない。

新短語はgate12/12、既存v4/full-input ASR controlは一致11/12（本+white SNR0→ほう。）、全speech review_required/confirmed=false。ASRは新版VADを統合した測定ではない。両modelの全VAD probabilitiesは2pass byte同一、短語ASRもfresh2workerでtext/status/gate/reviewを再現。初回CC0 license guardのprepare拒否は推論前、同sourceを保持して正しいlicense対応へ修正した。相槌はい/うんとofficial Python wrapper runtime parityは未確認。

詳細：[QWEN3-ASR-VAD-VALIDATION.md](QWEN3-ASR-VAD-VALIDATION.md)、`evidence/qwen3-asr/worker/current-vad-validation/`。Rust17/Python12/Node14、306endpoint reference、fmt/compile成功。worker/model/default/app不変。次にquiet input scale/gateを比較する場合も別候補として先に固定し、失敗を保存する。本体統合/live受入は禁止のまま。

## 前工程：ユーザー承認で公式現行VADを比較、候補設定のみ・採用保留

ask_userでユーザーが「公式現行VADと比較」を明示選択。pinned公式ONNX（repo `1e261b0…` / package6.2.3）を別artifactで取得・Git blob照合。512new+64context/state[2,1,128]/scalar srの契約でRust ORT診断を実装。新wrapper helper2 tests成功、同89windowsを2pass比較しprobabilities byte同一。

same peak20/.5/>90msではv4のmiss2/FP2が、公式現行のmiss0/FP1へ改善。旧burst false-positive/quiet EN missesは解消するが、低振幅harmonic toneで新FP。前と同じ24設定gridではpeak20/.7/>90msなど7設定がin-sampleゼロwindow-error。しかし同fixtureで選んだ探索結果であり、held-out/自然会話/公式Python wrapper実行parityは未確認。short-word ASRは別問題として残り、worker/app defaultは一切変更していない。

詳細：[QWEN3-ASR-CURRENT-VAD.md](QWEN3-ASR-CURRENT-VAD.md)、`evidence/qwen3-asr/worker/current-vad/`。次は候補を固定して未使用条件と複数speaker短語・独立wrapper parityを検証。比較承認を本体/model採用承認と解釈しない。

## 前工程：同一v4の設定grid・ASR/context診断完了、対策採用は保留

ユーザーの「続けて」でsame v4/512を3gain×4threshold×2min-duration=24設定、89 request windowsで比較。誤検出/未検出とも0の設定はなし。gain100はquiet ENを救うがburst false-positiveが残り、threshold/minを上げてnoiseを止めると本当のspeechも消す。known EN speechを診断でVAD bypassすると両passで読める（you are→you're、raw WER13.33%）。既定workerにbypass/fallbackは追加していない。

短語`いいえ`の14入力×full/crop=28requestsは両modeとも検出14/14、一致7/14。cropは救う例と壊す例があり、一律切り出しへ戻さない。診断PCM100倍も否定語を救えず、review理由なしの誤りも確認。worker/model/default/VAD/ASR/appは不変。詳細：[QWEN3-ASR-DIAGNOSTICS.md](QWEN3-ASR-DIAGNOSTICS.md)、`evidence/qwen3-asr/worker/diagnostics/`。

次は新しい公式VADとの比較を別途承認してから行う候補。VAD更新だけでASR短語の不安定さが治るとは考えない。本体統合・live受入は引き続き禁止。

## 前工程：追加robustness評価は不合格、本体統合を保留

ユーザーの「ok」で公開音声+合成noiseの追加評価を実施。same binary/model/VAD/contextを維持し、capture/playbackなし。JA2/EN1/TH1の8条件=32speech、18negativeのうちburst noise2条件が`はい。`を生成（ASR1、review理由なし、confirmed=false）。EN1clipは-40dB/white SNR0で未検出。licensed Lingua Libreの人間の短語`いいえ`は検出6/6だが転記一致3/6、前後1秒zero padで`家。`になる。false positives/EN misses/短語clean,padded,-40dBをfresh-worker追加2passで再現した。

無音抑制・既存30clip回帰の成功だけでは採用できない。短語fixtureは1speakerの単語発音であり自然会話の相槌品質ではない。`はい`/`うん`、実keyboard/music/環境音は未評価。次はgate false-positive/false-negativeと短語のASR context不安定さを切り分け、同じ失敗fixtureを残して対策を比較する。閾値/モデル/本体のpolicyを今回は変更していない。

詳細：[QWEN3-ASR-ROBUSTNESS.md](QWEN3-ASR-ROBUSTNESS.md)。raw evidence：`evidence/qwen3-asr/worker/robustness/`。noise generator2/report3 + 既存supervisor4/metrics5成功。

## 前工程：独立worker修正完了（限定fixture回帰合格、会議受入は未達）

ユーザーの「修正して」で修正。same pinned Silero v4を**内部forward 512samples**で呼び、sample-based endpointingを実装。defaultは発話presence gate後に**30秒上限内のoriginal PCM全体**を一回認識し、contextを削らない。`--crop-context`は同一worker診断のみ。本体ASR/VAD/録音/既存locks・モデル選択は変更なし。

全30speechでcorrected six-thread 1.7B decoder-INT8基準のraw textと文字列一致：**JA CER3.05% / EN WER5.14% / TH CER6.49%**。無音は空/ASR0。corrected512でもcropすると**2.48% / 5.71% / 10.18%**で、geometryだけではcontext/数詞表記問題を解決しない。raw採点・review-required textは保持。旧失敗reportも残す。

Rust15/Python4/metrics5と306endpoint reference cases成功。8negative空/ASR0、original/-20/-40dB/30秒long silent tailの日本語1clipはreference sentence保持。旧未検出500ms cropを検出して要確認のraw textとして残すが、真の相槌品質は未証明。sample128001のEOF保持。実noise/会話/Windows/live/二時間運転は未検証、**本体統合はまだしない**。

詳細：[QWEN3-ASR-WORKER-FIX.md](QWEN3-ASR-WORKER-FIX.md)、`evidence/qwen3-asr/worker/fixed/`。下の480/crop試作と解決策調査は履歴であり、現行実装の説明ではない。

## ユーザーの最新決定

**Meetilyを土台に切り替えることを明示承認。** branch `feat/live-minutes`、upstreamは `a2cb62e827da7ef59f65064c97233efb2313878e`。Linux評価のcheckpointは `13f432e`。旧Murmurは `../murmur` の `feat/live-minutes`（`93b8397`, `50f8b6d`）に保持し、必要なMIT実装だけ参照・移植する。

LLMはユーザー指定の **gpt-6-luna / 公式Sign in with ChatGPT OSSプラン利用**。アプリ専用の新規ログインで、既存Codex/piの資格情報はコピー・利用しない。音声はローカルのまま。クラウドへ送るのは明示選択時の文字起こし・議事録、または明示実行した人工文fixtureのみ。

## 今回追加したもの

- `live_minutes.rs`, `LiveMinutesPanel.tsx`: 根拠segment ID、安定item ID、撤回/置換履歴、構造検証、原子的JSON/Markdown保存とrevision archiveを移植。
- full reconciliationは48,000 UTF-8 bytesで上限。超過時はモデルを呼ばず、録音は続ける。無効なJSONへのrepairは最大1回。モデル/権限の失敗では録音に触れず議事録更新だけ停止する。
- MeetilyのTranscriptSegmentにはspeaker識別がないため、evidenceのspeakerは明示的にnull。mic/systemやログイン名を人名と捏造しない。話者分離の移植は次工程。
- `siwc.rs`: 公式discovery、dynamic-agent初回登録、stable opaque host ID、state/nonce/PKCE S256、issued client ID、RSA署名/JWKS/issuer/audience/exp/iat/nonce検証を実装。
- 複数account/workspaceのclient IDと検証済みsubjectを別々に保持。新ログインは検証後にactive化。発行済みだが未完了の登録も残し、code expiry後にdynamic登録を繰り返さない。
- app専用protected credential store: Unix directory 0700 / file 0600、atomic replacement。Windowsはuser-scope DPAPI ciphertext、失敗時にplaintextへfallbackしない。
- refreshのthread/process直列化（tokio Mutex + OS file lock）、token/expiry/scopesのまとめた置換、terminal refreshエラー時だけtokensを停止。sign-outではrevocationを最大2回試し、未確認ならローカル使用停止と設定画面でのdisconnect案内。
- model一覧は選択accountのBearerで**公開 `/v1/models`**。Codex cacheを利用しない。Lunaが一覧にない場合に他モデルへ自動変更しない。
- 推論は**公開 `/v1/responses`のみ**、`store=false`, `stream=true`。preview非対応parametersは送らない。SSEの日本語byte分割・改行形式・completed/failed/incomplete/refusalを処理。partial draftは保存しない。
- usage limit・policy・permission・invalid requestでは自動更新をpause。temporary errorにはbounded backoff（3回まで）。別課金/モデル/providerへfallbackしない。
- `ChatGptPlanControls.tsx`: active account、Continue with ChatGPT、再同意、model selector、Manage usage、録音なしの日本語人工文テスト。onboardingの最初の画面からも開けるので、ASR/builtin summaryのdownload前に認証・人工文テストが可能。
- Tauri wrapperのclean-checkout helper準備とNode mockテスト3件を移植。Linux deviceの実名にdisplay suffixを混ぜるバグも移植修正。ただしmonitor取得自体は未解決。

## 検証（完了と未検証を分離）

- 公式auth初回版: Rust check成功、SIWCテスト12件、議事録構造/保存5件成功。Node sidecar mock3件、TypeScript、frontend production build、native build成功。
- その後の自己reviewで、SSEエラーの**実HTTP status/request ID保持**、model再取得の再試行操作、モデルの無断切り替え防止、host ID紛失時の無断再生成拒否を追加。最終checkpointでSIWC 14件、議事録6件が成功（実Ollama1件ignore）。Node3件、TypeScript、production build、native buildも成功。ログ: `../logs/meetily-checkpoint-build.log`, `meetily-siwc-final-frontend.log`。
- テストは人工RSA token、temporary directory、localhost mockのみ。**その後、ユーザー自身が公式browser loginとLuna人工文テストを実行し、「テスト成功」と報告した。** これは実アカウントでの初回推論成功のユーザー確認であり、こちらがtoken内容を調べた確認ではない。latency数値はまだ取得していない。
- Windows DPAPIのAPI署名は、現行ソースのprotect関数をLinux metadata-only probeでcompileし確認済み（4.75s）。Windows実機・暗号化/復号の実行検証の代わりにはならない。`../logs/dpapi-api-check.log`。
- 元Murmurのqwen3:1.7b人工文テストは意味誤りで失敗（約65秒、repairを含む）。この結果を弱めない。Meetily移植後の実Ollama再測定はまだしていない。
- 初回移植testでsha2がbuild-dependenciesにしかない点を見落としcompileが失敗。runtime dependencyへ追加して修正。upstream欠陥とは扱わない。

## 日本語ASR / Linux内部音声の予備評価（2026-10-12）

- 実会議・マイク・desktop音声は収録せず、公開FLEURS日本語validation先頭3件（41.94秒）+人工5秒無音を、Meetily本体のWhisperProvider/Engineからローカル処理。CLI example、fixture取得/hash検証、CER/RTF計算とテストを追加した。
- CPU-only debugの予備値: baseのspeech CER 15.48%、RTF 0.848。smallは5.81%、RTF 3.231。3件の読み上げのみなのでモデル採用・会議品質・release速度の保証ではない。base初回はbuildと重なったため探索記録を分離し、build終了後の再実行を主値とした。
- 無音をASRへ直接入力するとbaseは`。`、smallは`お疲れ様でした。`を生成。VADを通していない試験だが、既知の無音誤生成として保持。正規化で句読点が消えてもraw非空を計測する。
- Node tests **8件成功**（metrics 5 + sidecar 3）、native example build成功。再取得後のPCM/正解文/model hashes一致を確認。モデル約636MBと音声はrepo外、公開文と測定値/出典/hashのみ `evidence/asr/` に保存。
- CPALのAlsa host入力にはmonitor名がなく、Pulse側にはmonitorが存在。既存bundle FFmpegにはPulse入力なし。名前の修正だけではこの環境のsystem audioを取得できないことを再確認した。
- **ユーザーはLinux専用native PulseAudio backendの追加を明示承認。** PipeWireのPulse互換serverからmonitorを選択し、既存pipelineへ接続する方針。mic/Windows CPAL経路は維持。backendの導入・人工null sink capture・stop/reconnect検証は次工程で、実desktop/mic収録の許可とは解釈しない。
- 詳細と再現: [ASR-LINUX-EVALUATION.md](ASR-LINUX-EVALUATION.md)。このcheckpointの変更は評価tools/docsのみで、appのcapture/ASR設定は変更していない。

## Qwen3-ASR独立CPU比較（進行中・2026-10-12）

- ユーザーは5650GE CPUで0.6B/1.7Bを実測し、日本語・英語・タイ語の精度/RTF/RSS/低遅延を比較してから実装するよう依頼。公式BF16非量子化を先に比較し、速度不足時にINT8/INT4も調べる。コードは `benchmarks/qwen3-asr/`、重み/公開音声/engine/venv/build/logはrepo外。既存ASR・録音・app lockfileは変更していない。
- FLEURS validation各言語先頭10件、speech計319.16秒、2/4/8秒prefix、人工無音を準備・hash検証。Thai APIのscan上限を回避するため同じvalidation Parquetを直接取得して先頭10件を抽出。C、Candle、Rust decoder-INT8、比較用C++ mixed-Q4を実行。読み上げ少数標本であり会議/長時間/Windowsの保証ではない。
- 独立metricsテスト5件成功。znver3/LP64 releaseのupstream libテスト32件成功（INT8 matvec/argmaxのscalar-reference含む）。実際の公開fixture warm-upをGDBで止め、AVX2 integer-dot `vpmaddwd`のhit/1命令stepを確認。OpenBLAS runtime core Zen・LP64・実6 threads、release薄LTO/CGU=1を確認。1/3/4/5/6/8/12、交互5/6、affinity、OMP wait挙動も実測。thread数で認識結果も変わるため、速さだけで品質同一としない。
- 確認中に重大なABI不一致を発見: NixデフォルトOpenBLASはILP64だが、Rust bindingのCBLAS引数はi32。初期Rust測定を `../runtime/qwen3-asr/results/exploratory/invalid-blas-abi/` へ退避し、推奨根拠から除外した。`blas64=false` でLP64へ揃え、ABIガードとscalar SIMDテスト、INT8/BF16の再ビルド・全再測定が完了。旧ILP64 libraryのloadを明示的にモデルload前に拒否する負テストも成功。観測上動いたことをABI安全性と取り違えない。
- streamingでは32-token budgetによるThai末尾欠落を発見、final budgetを512に増やして再試験。早い仮説をprefix固定すると `海外` が `海岸` のまま残る例もあるため、終了時に同じ全発話をoffline再認識する比較も追加。途中草案は根拠確定扱いにしない。全構成に無音幻覚があり、小Q4は繰り返しが顕著。失敗記録は保持。
- **ユーザーはINT8を選択し、このCPUで本当に最適化されているかの確認と、16bit比較も追加依頼。** その後 **1.7B decoder-INT8** を明示選択し、幻覚の対処方法を質問。本体への接続はまだ行っていない。現在のINT8はdecoderのみ、encoder/prefillはF32。16bitはまず公式BF16保存・CPU F32演算で確認し、5650GEで全層ネイティブBF16演算するという意味にはしない。
- 修正済6-threadのweighted RTF: 0.6B BF16/C 0.278 vs decoder-INT8/Rust 0.186（JA CER 8.00% vs9.14%）。1.7Bは0.610 vs0.419（JAは両方3.05%）。RSSはINT8の方が多い。1.7Bのdraft+全発話final再認識は入力終了後6.03–10.29秒、0.6Bは1.85–3.09秒（各言語1clipのみ）。会議やWindowsの成功ではない。
- 結果、CPU確認、public-only evidence: [QWEN3-ASR-CPU.md](QWEN3-ASR-CPU.md)。pinned engine revisions、失敗、再現は [../../benchmarks/qwen3-asr/PLAN.md](../../benchmarks/qwen3-asr/PLAN.md)。次は選択済み1.7B INT8の独立worker原型。無音幻覚は既存Silero VADの再利用、speechなし時の推論/flush禁止、短い相槌・語尾を残す境界処理、異常反復検知を候補として公開音声/人工無音/noiseで検証する（まだ未実装・未検証、RMSだけで小声を消さない）。PulseAudio、録音、クラウド資格情報には触れていない。

## 明示的な制約

実験版。構造検証は意味の正しさを保証しない。最終stop直後のtail reconciliation、長時間の増分window、manual edit保護、native ASR/diarization品質、録音の障害復旧、notes destinationsは未完了。AI草案をそのまま確定会議録にしない。

refresh成功後の新ID tokenを検証するJWKS取得が失敗した場合、credentialsを置換せず拒否する。安全側だが、serverがrefresh tokenを回転済みなら再ログインが必要になる可能性がある。credential回復の耐障害テストは次工程。

開発UIのGoogle font取得はnetwork retryあり。package済みfontのself-hostと完全offline buildを区別し、完全offline buildは未保証。

## 再現・作業環境

```bash
cd /home/user/Code/meeting-assistant-research/meetily
nix-shell shell.nix
# pnpm 10.34.5 (12とは混在させない)
cd frontend
pnpm install --frozen-lockfile
pnpm exec tsc --noEmit
node --test tests/lib/tauri-sidecar.test.mjs
NEXT_TELEMETRY_DISABLED=1 pnpm build
cd ..
cargo test --locked -p meetily --lib siwc::tests
cargo test --locked -p meetily --lib live_minutes::tests
cargo build --locked -p meetily
```

raw cargoでは先にllama-helperをbuildしてTauri binariesへstageする。検証中は親の `murmur/target` を共有したため、`CARGO_TARGET_DIR=/home/user/Code/meeting-assistant-research/murmur/target`。GUI試験は `MEETILY_DISABLE_ANALYTICS=1`、isolated XDG data/config/cache、Xvfbを使用。これらのprivate runtime filesはGitに追加しない。

## 出典・保存

移植元は自分のMurmur実験branch。元MurmurのMIT noticeは `licenses/Murmur-MIT.txt` に保持。FFmpeg/modelの再配布条件は別問題。private credential directoryはGit ignoreにも追加。

## 次の境界

1. 新GUIを確認し、scoped commit。Wayland/niriの実desktopで隔離data/configを使うnative GUIとlocalhost dev serverを起動した（録音は開始しない）。
2. **公式browser loginとLuna人工文テストはユーザー報告で成功。** token内容/authorization URLをログやツール出力へ出さない。refresh・再ログイン・remote revocationの実運用検証は別工程。
3. 未提供/未対応なら公開APIのstatus/code/request IDで診断。旧Codex/private APIへ戻さない。
4. 公開日本語音声fixtureでWhisper等のCER/latency/RTFを測定し、Linux system audio、speaker identification、incremental/final reconciliation、recovery、note出力へ進む。実際の通話収録は別途許可を得る。

## 独立1.7B INT8 worker / 発話ゲート試作

ユーザーの「じゃあそれで」で幻覚対策試作を開始。`benchmarks/qwen3-asr/worker/` は既存Meetily ASRと別のLinux-only Rust JSONL worker。モデル常駐・LP64検査・30秒PCM16入力上限・request ID・単一実行・supervisor timeout/kill/restart・新規VAD/認識状態・原PCM時刻・未確定出力/要確認flagを実装。本体capture/provider/schema/既存lockは変更していない。

公開音声/人工音だけで検証。Rust 11 + Python supervisor 4 tests成功。無音（発話前/後）、白色noise4水準、50/440Hz計8条件は空文字/ASR呼出0。VAD-only上限20倍gainで、-40dB化した日本語1clipの先頭取りこぼしは改善（ASRのPCMは原音のまま）。128001sampleの末尾を保持。ただし500ms未整列cropは検出されず、実際の短い相槌の保護は未検証。

**品質受入未達、本体統合禁止。** VAD各区間を別認識した試作はJA CER 3.62% / EN WER 5.71% / TH CER 14.27%に悪化。30秒上限内で内部pauseを残す一つの認識windowに変更して全30clip再測定：**2.86% / 6.29% / 10.48%**。選択モデルのungated基準3.05% / 5.14% / 6.49%より英語/タイ語が悪い。無音対策と実音声精度は別であり、境界切出しの対策が次の課題。自然音でのVAD誤検出/追加「はい」は初期試作で観測しraw保持。300ms無音prime案は白色noiseを誤検出し撤回。失敗も保存した。

手順/制約：`benchmarks/qwen3-asr/worker/README.md`。公開evidence：`docs/meeting-assistant/evidence/qwen3-asr/worker/`。主log：親`logs/qwen-worker-window-acceptance.log`。Windows/live draft/実環境noise/二時間運転/本体統合は未実施。次は発話有無gateと認識context境界を分けた比較を行い、短い真の発話・quiet speechの保護を同時に評価する。

### 解決策調査（2026-10-12、未採用）

`QWEN3-ASR-HALLUCINATION-RESEARCH.md` に一次情報と新規診断を保存。embedded Silero ONNXはblob hashでofficial v4.0と一致。しかしRust libraryは480 samples/30msを直接modelへ送り、v4 official wrapperの最低512/trained frame契約と合わない。worker外のforward-only診断でsame weights/PCMの480→512を比較：500ms cropの最大pは0.168→0.574、quiet原音のpositive framesも増加。noise4種は両方zero positive。ただし元日本語冒頭音にも512のほうが強く反応し、false-positive解決を意味しない。真の相槌保護は未受入。現行worker/app policyは変更しない。

タイ語raw CER 65→105 errorsのうち数字3clipが5→58（+53）、その他7clipは60→47（-13）。数字のタイ語綴り化等が大きいため、「文脈切断で発話欠落」という前の説明は不十分。raw scoreを変えず意味/表記/挿入/脱落を別分類する。reference外の冒頭「はい」も実際に話されたか未確認で、純無音の幻覚と混同しない。推奨次比較はofficial互換VAD geometry → 同一workerのpresence-only full PCM vs crop → official現行VAD/auto language比較。モデル交換/語の削除/LLM補正を先に行わない。
