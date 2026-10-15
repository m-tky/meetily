# 録音画面の入力機器選択

## ユーザー決定・範囲

ユーザーはUIからマイクとスピーカーを選ぶ機能を依頼し、ask_userで「録音画面に常時表示」を選択。スピーカー欄は**PC再生音を取り込む対象**の選択であり、OSの再生先や音量の変更ではない。実マイクは使用中なので、自動検証は既に承認された公開FLEURS＋private仮想音声だけ。

## 実装

- `RecordingInputPanel.tsx`をホーム画面に常設。「マイク」「PC音声（取り込むスピーカー）」の日本語ラベル、一覧更新、読み込み／保存／失敗表示。
- `DeviceSelection.tsx`は実際のnative機器一覧と同一device IDを使うnative select。旧fakeメーター／audio-level listenerを撤去。選択のための録音・level-monitor開始は行わない。compactモードにはbackend変更欄を出さず、既存設定ダイアログには残す。
- ConfigContextの新しい保存関数はget→merge→setを直列化。auto_save／save_folderなど他の録音設定を保持し、保存成功後にStart用のselectedDevicesを更新。失敗時は前の選択を維持。設定ダイアログも同じ保存関数を使用。
- preferences読み込み中・保存中・録音・開始・停止・処理中は選択不可。Startはprefsと一覧の準備完了、保存成功、現在の選択に対応するvalidation結果を必要とする。古いvalidationが新しい選択を一瞬有効化しないよう、機器IDをsnapshot比較。
- 一覧から消えた保存済み選択を自動で既定に戻さない。見つからない項目を表示し、選び直すまでホームのStartを無効化。これはUIの事前検証であり、一覧取得後の抜き差し／native側の既存mic fallbackの変更・保証ではない。
- LinuxのALSA input一覧取得失敗でも、利用可能なnative Pulse monitor一覧を返す。MicのないPC-only環境で有効なスピーカー欄／Startを隠さない。captureルーティング／マイク処理／ASR／VADは変更しない。

## 検証

- TypeScript、隔離コピー内のNext static export、Python構文、git diff --check成功。通常の`.next`／`out`とユーザーpreviewを変更しない。
- preference writer4＋picker5＝9PASS。保存設定保持、直列化、失敗後の再試行、choice snapshot、録音中ロック、missing device非自動変更、raw ID対応、PC-only、一覧エラー／更新回復を検証。
- native mixing11＋normalizer10＋pipeline12＋VAD7＝40PASS。これは既存回帰の成功であり、known recovery orderingを検証するテストは未追加・未修正。
- `test_public_virtual_audio.py --select-via-ui --pc-db -20 --expect-normalization`で実DOMから選択、native保存、browser reload、Start中ロック、Stop保存、native app再起動後の選択復元／recording=false／録音ファイルbyte不変を監査。録音ログは再起動時の上書き前に`native-recording.log`へ保持。
- 最終PC-only runは全体PASS。実mic／物理speaker／cloud使用0。保存・event/raw/SQLite等価性・cleanup成功。
- 最終virtual dual（Micはzero信号）は**UI検証PASSだが全体FAIL**。保存・認識非空・event/raw/SQLite・再起動・cleanup成功、一方recording-error1／stall1／late discard420018samples。初回dualもstall1／418095samplesでFAIL、結果を保持。既知のdegraded recovery問題と整合するが、破棄sourceはこのログから特定できない。精度、完全なfixture保持、実同時二入力の受入は主張しない。

ログ：`../logs/meetily-input-ui-{frontend-build,candidate-build,candidate-stage,native-tests,preferences-tests,picker-tests,public-single,public-dual,public-dual-initial-stall}.log`。

## 候補と表示中アプリ

- embedded **debug／no-bundle**候補：`../runtime/nixos-input-ui-candidate` → `/nix/store/z24swrrspim7w4i1l5bm8d0db8bk1ama-meetily-local-candidate`。
- isolated frontend build：`../runtime/input-ui-build-9kXlUb/frontend/out`。candidate inputとのbinary equalityとdev binary復元を確認。
- 新候補には承認済みPC normalizationも含むが、known degraded recovery bugは未修正。
- ユーザーが表示中の`dual-preview-spocggm1`は古いsync候補のまま。閉じたり、Start／Stop／再生したりしていない。切替え前に保存完了／window閉鎖確認が必要。
- ユーザーはknown recoveryを先に修正と明示選択。その修正／43native tests／新candidate／残存PC buffer latency診断は[INPUT-RECOVERY-FIX.md](INPUT-RECOVERY-FIX.md)を参照。さらに承認済み100ms PC fragment実装／46native tests／UI＋norm＋recovery＋fragment候補／private dual3runs警告0と残存latency診断FAILは[PC-CAPTURE-FRAGMENT.md](PC-CAPTURE-FRAGMENT.md)を参照。このファイルの候補と失敗結果は修正前checkpoint。その後ユーザーが保存完了・閉鎖済みを確認し、新fragment候補を同じpreviewへ反映した。OS上で選択済みEchoCancel機器が現在不在のため、選択を保持しmissing表示／Start disabledでREADY（録音false／capture0）。詳細はPC-CAPTURE-FRAGMENT.md。新しい処理方式／既存gain・model・VAD・resampler変更／releaseは別承認。
