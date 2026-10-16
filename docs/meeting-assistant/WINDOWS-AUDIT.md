# Windows対応調査・段階計画

## 後続承認：Qwenセットアップ対応 RC4 の公開準備

ユーザーがpush後の公開も明示承認。既存RC3のtag/assetsは変更せず、`windows-v0.4.1-rc.4` を個人forkへ新規公開する。release notesを新しい同意付きWelcome／Settings導線に合わせ、Windows CIのnative setup／onboardingとNode setupテストを必須化した。Windows Job Objectの実子process終了テスト（NoProfileの固定Start-Sleepのみ、録音／model／cloudなし）を追加。事前TypeScript＋Node25件成功。CIのbuild/install/uninstall/publish成功と公開assets再照合を確認するまでは公開完了と扱わない。新導線の実モデルHTTPS取得／GUI統合受入は別途未検証として維持する。

## 最新追記：Qwen同意付き初回セットアップ導線（ソース実装、Windows受入は未実施）

ユーザーがWelcome＋Settings両方、Qwen-only初回設定完了を明示選択。固定モデルのHTTPS download／SHA256・容量・排他・no-clobber／隔離cache初期化／キャンセル・子回収／成功後の選択を追加した。既存Parakeet・要約のdownload済み偽装や自動追加downloadは行わない。READMEにもRC3と新ソースの差を記載。Linux合成native・frontend・production export・Nix評価とWindows Job Object API型検査は成功。**新しいWindows build／GUI統合／実HTTPS downloadは未検証で、公開RC3は不変。** 詳細・失敗・残課題：[QWEN-WINDOWS-ONBOARDING.md](QWEN-WINDOWS-ONBOARDING.md)。実音声・ログイン・cloud推論・新releaseは行っていない。後続のユーザー指示で、このソース変更のcommitと個人fork mainへのpushが承認された。

## 初回調査の範囲（後続の実施結果は末尾）

ユーザー決定：まず調査と対応計画のみ。対象は現在の `meetily/`。検証候補としてvirt-manager上にWindows 11がある（CPUアーキテクチャ・割当資源・デバイス構成は未確認）。

ソースと既存記録を静的に調査した。Windowsビルド・テスト・VM起動・設定変更・実音声収録・ログイン・クラウド送信・リリースは実施していない。本書以外の変更は行わず、既存のNix関連作業を維持した。

## 結論

Windowsの土台は既存実装を再利用できる。ただし、現在使用中のQwen3-ASR 1.7B decoder-INT8はLinux用試作に依存しており、同等機能のWindows対応で最大の作業になる。Linux-onlyフラグを解除するだけでは完成しない。

Windowsで起動できること、Qwenが推論できること、マイク＋PC音声から保存・再閲覧まで動くことは別々に検証する。話者分離の追加やASRモデルの置換は今回の対応計画に含めない。

## 確認した実装と障害（初回静的調査時点）

### 1. Qwen：明確な移植障害（最優先）

- `frontend/src-tauri/src/audio/transcription/qwen_provider.rs`：`QwenSettings::validate` と `qwen_is_supported` がLinux限定。プロトコルの案内や `frontend/src/components/QwenModelSettings.tsx` もLinux専用。
- `benchmarks/qwen3-asr/worker/src/main.rs`：非Linuxターゲットを `compile_error!` で拒否。OpenBLASのLP64検査・6スレッド固定・ORT動的ロードを要求する。
- `benchmarks/qwen3-asr/worker/Cargo.toml`：Qwenエンジンはリポジトリ外の `../../../../runtime/qwen3-asr/rust-native/crates/qwen-asr` にpath依存。通常の単独cloneではworkerを再現できない。
- 当該エンジンの `src/safetensors.rs`：Unix `RawFd`、`mmap`、`pread`、`madvise`、`sysconf`、`munmap` 等を使用。`src/kernels/pool.rs` に `libc::sched_yield` もある。Windows向けのOS抽象化が必要。
- 当該エンジンの `build.rs`：BLASリンク設定はmacOS/Linux向けのみ。workerのOpenBLAS外部関数も含め、Windows用LP64ライブラリ・リンク設定・実行時DLL探索を整備する必要がある。
- アプリの `tauri.conf.json` のexternalBinはllama-helper/FFmpegのみで、Qwen worker・モデル・Qwen用ORT/OpenBLASの導入は含まれない。

必要な作業：エンジンの取得元・revision・ローカル差分を固定し、Windowsファイル読み込みとスレッド処理を移植。MSVC向けworkerと依存DLLを構築し、モデル準備・INT8 sidecarの互換性を確認する。既存Linux版の認識条件・float32入力・30秒上限・失敗時fallback禁止を維持する。移植方式や依存の同梱方針は実装前に合意する。

### 2. 音声：WASAPI基盤あり、選択保持と回帰検証が必要

- `frontend/src-tauri/src/audio/devices/platform/windows.rs` にWASAPI列挙・入力/出力設定がある。
- 主録音経路は `recording_manager.rs` → `AudioStreamManager` → `stream.rs` のCPAL入力ストリーム。出力デバイスを使うWindows loopback経路の実行確認が必要。
- 別の `capture/system.rs` に非macOS未実装の公開ヘルパーがあるが、これだけを根拠に主録音経路がWindows非対応とは判定しない。
- Windowsデバイス解決には部分一致と、指定デバイス未検出時の既定デバイスfallbackがある。意図しない別入力の収録を避けるため、選択デバイス消失時の停止/エラー/明示再接続方針を整える必要がある。
- Mic/PCのsource保持は共通pipeline・保存層にある。Windowsで入力の取り違え・同時発話・末尾drain・切断を検証する。

### 3. 保存・復旧：共通実装あり、Windows固有条件が未検証

`recording_saver.rs`、`incremental_saver.rs`、`recording_recovery.rs`、`live_minutes.rs` は既存の一時ファイル・atomic persist・WAV checkpoint・FFmpeg最終AAC化を利用できる。

Windowsでは日本語/空白/apostropheを含むパス、FFmpeg concat内のWindowsパス、使用中ファイルの置換/削除、読み取り専用先、強制終了後の保全・再登録を検証する。Unix権限やLinux O_NOFOLLOWと同等の保護をWindowsで確認する必要があり、既存のUnix-only symlinkテストだけでは代替できない。

Qwenの一時WAVを別プロセスから開く動作、timeout/取消/worker crash後の終了・削除、Windowsユーザー領域のアクセス制御も検証対象。

### 4. ChatGPT連携：DPAPI実装あり、Windows実行は未検証

`frontend/src-tauri/src/siwc.rs` はWindowsユーザースコープDPAPIを実装済み。平文へのfallbackは禁止されている。既存記録のAPI署名compile確認はWindows上の暗号化/復号・再起動・更新の実証ではない。

人工データで保護保存・再読込・排他・破損・置換失敗を検証し、その後、別途許可のある段階でVM内ブラウザのloopback callbackとアプリ専用ログインを確認する。ホスト側の資格情報をVMへコピーしない。実会議の送信は不要。

### 5. ビルド・配布：既存Windows CIあり、独自版用の整備が必要

- `.github/workflows/build-windows.yml` はWindows x64/MSVC、pnpm 9.15.9、llama-helper、FFmpeg、ORT、Tauri installerの準備がある。ただし現状はVulkanビルドが中心で、Qwen workerを構築しない。pnpmも現在のローカル開発規約である10系と不一致のため、lockとの整合を確認して揃える必要がある。
- `frontend/src-tauri/build/onnxruntime.rs` は本体用Windows x64 ORT 1.22.0を取得する。Qwen workerの明示ORT指定と同一視せず、各依存の要求API/ロード先/ライセンスを確認する。
- `tauri.conf.json` は上流のアプリ識別子・更新URL・公開鍵を保持。独自版を上流版と混同しない識別・更新方針が必要。まず非公開の検証ビルドを使い、上流更新を適用しない構成にすることを推奨する。
- 署名scriptはDigiCert設定がない場合skipできる。初期検証は署名購入や公開releaseを前提にしない。公開配布時の署名・SmartScreen・FFmpeg/モデル/依存DLLの再配布条件は別途確認する。

## 推奨する段階計画と合格条件

1. **Win11 VMの前提確認**：承認後にguestのx64/ARM、CPU命令公開、vCPU/RAM/空き容量、音声デバイス、WebView2、開発環境を確認。Qwenは6スレッド固定のためVM割当との整合を確認。最初はGPU不要のCPUビルドで進める。
2. **本体のWindows基準線**：pnpm 10 frozen lock/MSVC C++・Windows SDK・CMake/LLVM等を揃え、llama-helper/FFmpeg/ORTを含むbuild/check/testと画面起動を確認。隔離データ領域・analytics無効・更新停止の検証構成を使う。
3. **Qwen移植**：固定ソースとWindows依存を用意。ready handshake→無音→既知の公開日本語音声→float32/30秒境界→crash/timeout/取消の順で検証。既存Linux fixture回帰も実行し、速度と認識差を記録する。
4. **音声から永続化まで**：VM内の合成PC再生音、次に独立した合成mic入力、最後に両入力。Mic/PCタグ、Stop末尾、WAV/AAC、JSON/SQLite一致、再起動・再閲覧・議事録の人工文編集/出力、保存失敗と復旧を確認する。実マイクや会話は許可なしに使わない。
5. **認証と配布確認**：人工DPAPI試験、許可後の専用ログイン/人工文推論。非公開installerを作り、開発用PATHなしで起動・依存DLL・モデル導入・再起動・設定保持を確認する。公開releaseは別途承認を得る。

## VMで分かること／分からないこと

Win11 VMはWindowsビルド・UI・DPAPI・保存・CPU推論・ゲスト内PC音声loopbackの検証に適する。ただしゲストが録るPC音声は基本的に**ゲスト内の再生音**で、ホストの会議音声が自動的に対象になるわけではない。mic入力の経路と独立性も別途設定確認が必要。

CPU命令公開・資源割当・仮想音声デバイス・GPU公開の有無は未確認。VM上の速度を実機性能と同一視せず、実機のUSB/Bluetooth、スリープ復帰、物理GPU、長時間会議品質は別の受入試験とする。

## 続行：VM起動と開発準備の承認・現在地

ユーザーが「VM起動と開発環境の準備まで許可」を選択。不足するRust/MSVC・Node/pnpm等の導入と本体CPU版ビルドまで承認済み。その後「VMは好きにしてよいので、開発がしにくかったら好きに変えて」と追加承認され、開発に必要なVM資源・接続設定の調整も可能になった。録音・ChatGPTログイン・公開配布はこのVM運用の承認と混同しない。

`virsh -c qemu:///system` で `win11` の構成を読み取り、起動成功を確認した。

- x86_64 / UEFI、CPU host-passthrough、8 vCPU（4 cores × 2 threads）。
- RAM 8GiB。資源設定は変更していない。Qwen推論時の余裕は未検証。
- virtio NIC、SPICE、virtio-vga、ich9 sound / SPICE audio。
- QEMU guest agent用チャネルは見当たらず、既存SSH/WinRMの標準ポートにも接続できなかった。
- WindowsはPIN入力を求めるロック画面まで起動。その後ユーザーが認証情報を提供し、エージェントによるログインを明示指示したため、コンソールからログイン成功。認証情報は本書・スクリプト・設定ファイルへ保存しない。
- 読み取り専用のguest preflight成功：Windows 11 Pro x64 build 26200、VS2022 BuildTools/Community 17.14、MSVC x64コンポーネント、Windows SDK 10.0.26100.0、WebView2あり。Rust/Node/pnpm/LLVMは未導入だった。
- Cドライブ空き約22GiB。Zドライブはhost `/home/virtual-machines/pool` のvirtiofs共有（guestはNTFSと報告）、空き約342GiB。専用 `Z:\MeetilyWindowsDev` を作成し、導入ログ/生成物を他のデータと分離する。既存フォルダ・Notepadの未応答ウィンドウは消去・強制終了しない。
- 開発ツール導入・version確認成功：Node.js v24.20.0、Rust/cargo 1.99.0、pnpm 10.34.5、LLVM 23.1.3（libclang.dllあり）。Node/LLVMのinstallerはwingetによる公式取得先・hash照合後、UACを確認して許可した。既存VSのインストール変更はしていない。
- npmの通常stderr noticeをPowerShell `ErrorActionPreference=Stop` が失敗扱いにした初回setup誤判定を保持。pnpm自体は導入済みで、別のversion確認で成功を確認した。後続build runnerはnative commandのstderrを成功判定に使わずexit codeで判定する。
- 拡張preflightの再実行が長時間化・メモリ増加したため、そのために開いた診断用PowerShellタブのみ終了した。初回preflightの結果は保持。広範囲の再帰診断は続行しない。
- sourceをguest専用 `%LOCALAPPDATA%\MeetilyWindowsDev\source` に展開し、targetは `Z:\MeetilyWindowsDev\target`、native build jobs=2/debug情報無効で準備。本体用の検証identifierとupdater endpoints空のTAURI_CONFIGをprocess限定に設定。アプリ起動・録音・モデル取得・クラウドログインは実施していない。
- frontend `pnpm install --frozen-lockfile`、TypeScript検査、Next.js production export はWindows上で成功。
- 初回sidecarテスト3件は期待値がPOSIXパス固定のためWindowsで失敗。`frontend/tests/lib/tauri-sidecar.test.mjs` のmockへhostに応じたpath.win32/posixと固定rootを注入し、完全一致assertionへ強化。Windows CPU dev/明示target＋空白入りcustom dirを追加し、5件がLinuxホストとWindows guestで成功。production wrapperは変更していない。原source zip後の変更はこのtestのみで、転送SHA256を固定してguestへ反映した。
- 初回helper buildはvirtiofs上で `install.vcxproj` を探すMSBuildが、生成済み `INSTALL.vcxproj` を見つけられず失敗。両ファイル名の差をhostで確認し、targetをguestの `%LOCALAPPDATA%\MeetilyWindowsDev\target`（C: NTFS）へ変更。C:の空き10GiB以上を起動条件にした。同一sourceのhelper CPU buildが4分11秒で成功したため、Windows native targetは共有ドライブへ置かない。
- helper buildにはLNK4098（CRT library競合）などの警告が残る。抑制していない。実行時の安全性・推論品質や配布受入の証拠ではない。Cargoのnon-root patch/profile無視警告も保持。
- 初回本体checkはWhisper C++ソースのUTF-8文字列をMSVCが別codepageで解釈しC3688。process限定 `CL=/utf-8` でC++ compilationを通した。Windows全体のlocale/UTF-8設定は変更しない。
- 続くcheckはWhisperのbindgen 0.69.5＋LLVM23で `whisper_full_params` が `_address` のみになりE0609。LLVM22以降の既知のforward-declaration互換問題と一致する（[上流の説明と修正参照](https://git.zx2c4.com/linux-rng/commit/?id=dc01dfb37b34beeefcfe1c3055364d41a4070c7e)、[bindgen修正](https://github.com/rust-lang/rust-bindgen/pull/3278)）。lock更新・bundled bindingsへの逃避は行わない。
- 公式LLVM18.1.8 win64 installerをhostで取得し、[winget manifest](https://github.com/microsoft/winget-pkgs/blob/master/manifests/l/LLVM/LLVM/18.1.8/LLVM.LLVM.installer.yaml) のSHA256 `94af030060d88cc17e9f00ef1663ebdc1126b35e16bebdfa1e807984b70abd8f` と照合。初回download timeout/partial hash失敗は保持し、resume後に完全一致を確認してから7zでDLL/clang/includeだけ展開した。installer実行や既存LLVM23のuninstallはしない。
- 展開先は `Z:\meetily-windows-transfer.HnU5H1\llvm18`、process限定のLIBCLANG_PATH/CLANG_PATH/resource-dirを指定。無効bindingsのログ/控えを保存し、`cargo clean -p whisper-rs-sys` だけで生成物を再作成。本体 `cargo check --locked -p meetily` がWindowsで成功（3分56秒）。
- 本体 `cargo build --locked -p meetily --features tauri/custom-protocol` がCPU/development profileで成功（5分16秒）。release最適化・NSIS/MSI installer・署名・配布用DLL配置を検証したものではない。
- `cargo test --locked -p meetily --lib siwc::tests` はWindows上で14 passed / 0 failed（359 filtered out）。人工データでDPAPI保護保存、署名・claim検証、loopback/callback、SSE等の対象テストが通過した。全Rustテスト・実ChatGPTログイン/クラウド疎通の合格を意味しない。
- 生成物確認：`%LOCALAPPDATA%\MeetilyWindowsDev\target\debug\meetily.exe`（74,258,432 bytes、SHA256 `6e82645ee9d3ffcf5d97ca6568d12c12b743c72c7e70c415d441ac19f7b31792`）。helper exeは6,034,944 bytes、ステージ済みsidecarとhash一致。debug直下にapp_lib/DirectML/onnxruntime/onnxruntime_providers_shared DLLあり。DLLロード・画面起動は未検証。
- guestのCargo.lock・pnpm-lock.yaml・修正テストのSHA256をhostと照合し、全て一致。依存lock・本番アプリコードは変更していない。完了時C:空きは約7.1GiB、RAM/vCPU変更は不要だった。
- 前記結果はローカル `../runtime/windows-audit/build-final-result.json`、`artifact-result.json` とshared専用logに保持。hostのsidecarテスト5件も再実行成功。アプリ未起動・録音なし・モデル未取得・クラウドログインなし。VMは起動状態で維持する。

構成抽出の初回コマンドではホストPATHにpython3がなく失敗したため、virsh XMLの対象項目を直接抽出して確認した。ローカルのコンソール確認画像はrepo外 `../runtime/windows-audit/` に置き、Gitには含めない。

guest操作にはlibvirtキー/ポインター入力を利用し、VM内からホストbridge専用HTTP（192.168.122.1:8874、当該guest IPのみ許可、限定されたスクリプト/ソース配布・JSON結果受付）で開発準備を実施した。結果の取得後、この一時HTTP serverは停止済み。SSH/WinRMの有効化・firewall緩和・VM資源変更は今回は行っていない。ホスト側ソースはgit管理対象のfrontend/llama-helper/ルートCargoファイルのみ495ファイルを作業treeからzip化し、SHA256を固定（repo外runtimeにmanifest）。既存node_modules/target、資格情報、実会議、runtimeモデルは含めない。

## 続行：Windows画面起動・人工議事録の保存/編集/再起動/書出し

ユーザーの「続けて」を受け、前段のCPU development binaryをそのまま検証した。本番ソース・依存・lock・既存Linux環境は変更していない。

### 隔離と起動

- 初回Welcome表示をWindows WebView2で確認。native IPCが利用可能、`is_recording=false`、`is_analytics_enabled=false`。ONNX Runtime 1.22.0のbundled DLL初期化成功を起動ログで確認した。
- 専用identifierの `%APPDATA%\local.meetily.windows-baseline` にsynthetic markerを付け、既存の無印profileなら起動を拒否。WebViewデータは `%LOCALAPPDATA%\MeetilyWindowsDev\smoke-01\webview` に分離した。通常profileやホストの認証情報は使わない。
- Chromium CDPはguestの `127.0.0.1:9339` のみ。PowerShellからloopback接続し、`http://tauri.localhost` の対象ページだけを操作した。外部向けport/firewall変更なし。一時HTTP serverも再開していない。
- guest作成のZ:フォルダはhostではroot所有で書込み不可だったため、初回script配置が失敗。ホスト所有の既存transferフォルダへ変更し、限定script/JSONを共有した。VM全体の権限変更・広範囲の再帰診断はしていない。
- `llama-helper.exe` はモデルを渡さず実際に起動し、ping→pong、shutdown→goodbye、exit 0を確認。FFmpegは `-version` がexit 0、8.0.1 essentials。本体隣のtemplates・helper・FFmpeg・ORT/DirectML DLL配置も確認した。推論は未検証。

### 人工データの検証結果

- 既存テストと同じ方式でonboarding状態だけ人工的に準備した。Parakeet/summaryはどちらも `not_downloaded` のまま。モデル導入/onboarding完走の受入証拠ではない。
- 録音せず、2件の人工transcriptを本物の `api_save_transcript` で専用SQLiteへ保存。Mic/PCタグがWindowsの保存済み会議画面にそれぞれ表示された。
- canonical fixtureの保存先はC: NTFSの `%LOCALAPPDATA%\MeetilyWindowsDev\smoke-01\recordings\日本語 O'Brien 空白`。既存のfixtureなら作成を拒否する。既定Music配下のrecovery scanは呼ばず、既存録音を探索しない。
- Saved minutesを開き、実DOMのEdit→textarea input/change→Saveを実行。revision 1→2、編集文、元item ID、2件のsource ID、before/after履歴、Edited/protected表示を確認した。React内部やnative保存処理のmockは使っていない（物理pointer操作の試験ではない）。
- revision 1を指定した古い編集要求が競合として拒否され、revision 2が保持された。JSON/Markdown backend exportの内容も確認した。
- 所有する検証アプリだけを終了し、同じprofileで新しいnative PIDとして再起動。SQLiteの会議・Mic/PCタグ・編集済みrevision 2・protected表示・履歴が復元された。録音/analyticsは引き続きfalse。
- 実DOMのExport JSON/Export Markdownを押し、実際のWindowsダウンロード2ファイルを確認。JSONはcanonical snapshotと一致し、Markdownは編集文を含む。`minutes-revision-2.json`（1,216 bytes、SHA256 `04419593765bb32a5fc4a5f2be44d98ab8a063bd35902035dea91445b4195fde`）、`.md`（688 bytes、SHA256 `a6d326f223d092f9b5c45e9e24ac0d628d4113b916ceb4c668a5ee64aa54ccd3`）。
- **テスト環境の注意**：CDPのdownloadPath指定はsuccess応答でもWebView2に反映されず、最初の検証は隔離Downloadsにファイルがないため失敗した。実際はVMユーザーの通常Downloadsへ出力されていた。自分のexport要求の時間内に作成された上記2つの正確な名前だけに限定して検証し、内容一致で成功を確認。任意のユーザーdownloadを走査/変更していない。この2つの人工exportファイルはVM内に保持した。
- raw `metadata.json` / `transcripts.json` の前後SHA256は一致。専用models配下のファイルは0。canonicalのrevision 2と編集履歴もrestart/export/競合要求後まで一致した。

### 証拠と終了状態

ローカル証拠はGit外 `../runtime/windows-audit/smoke-evidence/`：`verification-result.json`、`result-17.json`（native再起動後）、`private-webview.png`（人工データのWebViewだけ）、`app-stderr.log`、canonical snapshot等。初回のdownload先誤判定・初回runnerの余分な空object出力は別の初回記録に保持し、後者のPowerShell Task戻り値はrunner側だけで抑制した。

検証後、所有するアプリだけを終了。guest内で当該PIDとCDP listenerの終了を確認し、VM自体は起動状態で維持した。C:空き約7.1GiB。RAM/vCPU変更は引き続き不要だった。

**未実施**：実録音・再生、WindowsのASR/Qwen推論、ChatGPTログイン/クラウド送信、recovery scan、installer/release配布、録音開始から保存までの一連の受入。次の主要作業はQwen Windows移植であり、外部エンジンの固定方法・OS抽象化・BLAS/DLL配布方式は実装前に決定する。

次の判断材料：現在の外部エンジンは `runtime/qwen3-asr/rust-native` のclean revision `37c1e35be333945dc9b5771a6a33fe4b2064533f`。`qwen-asr` 0.11.0のcrateソースは約868KiB、MIT、workspaceからedition/licenseを継承し、Rust依存はlibc。推奨はこの必要crateを出典・license・revision付きの固定ソースとしてMeetilyに同梱し、そのコピーにWindows移植を加える方式（モデルは同梱しない）。外部runtimeの現物と既存Linux binaryを直接書き換えず、通常cloneでの再現性を得る代わりに、上流追従は明示的な更新になる。対案は外部作業コピーのままVM上だけで先行検証し、依存管理の正式化を後にする方式。その後ユーザーが「固定ソースを同梱して移植」を選択。以下の範囲を実装・検証した。

## 続行：固定エンジンとWindows workerの初期移植

### 変更範囲

- `vendor/qwen-asr/` に上記revisionの必要crateだけを同梱。MIT license、元ファイルSHA256付き `UPSTREAM.json`、ローカル差分説明 `MEETILY.md` を添付。モデルやビルド生成物は含めない。
- Windows専用のread-only RAII mapping（memmap2 **0.9.11**）、標準FileExtによる位置指定read/write（部分I/O・Interruptedを処理）、Rust allocatorと対応するVec、thread yieldを実装。Safetensorsとdecoder INT8 sidecarの両方を移植した。WindowsではUnix madviseを使わず、既存のskip bitmap付きページtouchを維持する。
- Linux側のmmap/pread/madvise・BLAS・allocation分岐は維持。外部runtime checkoutはGit cleanのまま、稼働中Linux workerやアプリの差替えもしていない。
- workerのpath依存を `../../../vendor/qwen-asr` へ変更。compile gateはLinux＋Windows x64/MSVCだけへ拡張。**TauriアプリのQwen Windows gateはまだ解除しない**。ready情報・float32/PCM16・30秒上限・認識context・言語・既定offのalignment等は変更していない。
- worker Cargo.lockの変更はmemmap2の追加だけ（同梱crateのstandaloneテストにも小さなlockを添付）。本体Cargo.lock/pnpm-lock.yamlは前段のSHA256のまま。
- `nix/worker.nix` はエンジン参照とpath置換のみ同梱sourceへ合わせ、既存のユーザー作業を維持した。他のNix関連ファイルの変更は今回の作業によるものではない。

### Windows実行結果

- guestに別領域 `%LOCALAPPDATA%\MeetilyWindowsDev\qwen-port-01` を新設。最初の転送は59ファイル、source ZIP SHA256 `8be8ee019fe30e88de1a796ba773f789b3002114a70c7827ee8f3bed77bc3d67`。後からGEMMテストだけをhash照合で更新した（`9e81acbce9d627722f936cb9bcfa6ebbbf1bc2c7414bd9c25a83bfd47f433919`）。host/guestでworkerとstandalone engineのlock一致も確認した。
- [公式OpenBLAS 0.3.30](https://github.com/OpenMathLib/OpenBLAS/releases/tag/v0.3.30) の `OpenBLAS-0.3.30-x64.zip`（LP64、**x64-64ではない**）を取得。公式release asset digestのSHA256 `8b04387766efc05c627e26d24797ec0d4ed4c105ec14fa7400aa84a02db22b66` と一致を確認してから展開し、licenseも保持。付属MSVC import libraryを使い、DLLは専用worker隣へ配置した。system PATH/他アプリは変更しない。
- Windows `cargo build --locked` が成功。workerのlib14件＋main3件＝**17件のテスト成功**。専用engineテストは直接manifestを指定し、scalar/OpenBLASの**両モードで各6件成功**。Unicode/apostropheパス、並行read、壊れたheader/offset、INT8 cacheのwrite/map/reopen/破損、2MiB超のallocation、非正方GEMMとtransposeを検証した。BLASモードのGEMMは実際にcblas_sgemmを呼ぶ。
- 最初のdependency package指定のengine testコマンドはexit 0でも実行件数がログに出ず、合格証拠には数えなかった。direct manifest＋実stdout件数＋exit codeを確認する別runnerで再実行した。初回ログも保持。
- 実workerでLP64/6 threads確認→ready→ping/pong→生成した1秒のmono16k float32無音→`no_speech`（asr_calls=0/model_loaded=false）→shutdown/bye→exit 0を確認。VADは既存と同じ埋込みSilero v4。Qwen/aligner重みは転送・ロードせず、ASR本推論はしていない。
- Windows debug workerは4,129,280 bytes、SHA256 `0dad0fd8b82a1ec4c49d4f4e13ca86e5fd8135e9e9c312e96df3c0cf5ec668d0`。Unixだけが使うsize/constantについて新規Windows分岐でdead-code warningが3件残る。抑制や機能変更で隠していない。

### Linux回帰と終了状態

- Linux engineテストはscalar/OpenBLASで各5件、上流kernelテスト9件、workerテスト17件、debug buildが成功。
- 同梱sourceを使うNix release workerもbuild/check成功（14 lib tests）。最終出力 `/nix/store/10kgbmr9iydq1d37dvw9blfv1idswdnq-qwen-speech-worker-0.1.0`。既存の配備先へは差し替えていない。
- Nix workerの無音protocol試験では、最初に手指定した旧ORT 1.22.2がexecutable-stack制約で失敗し、別stdenvの1.27.1を指定した試行もglibc不一致で失敗した。いずれも保持し、ライブラリやOS保護を改変して回避しない。**ビルドと同じpkgs scopeのonnxruntime**をNix評価から取得し、ready/ping/float32無音/shutdownが成功した。手近なNix store globからruntimeを選ばない。
- 証拠はGit外 `../runtime/windows-audit/qwen-evidence/`、`qwen-linux-*.log`、`qwen-nix-build.log`。fmt/diff検査も成功。所有するCLI workerは全て終了し、VMは起動維持。VMのRAM/vCPU/OS設定・他のアプリは変更していない。

**残件（初期移植時点。実モデル試験は次節）**：実モデルでのWindows認識、Linuxの同じ公開fixtureとの比較、warm/cold load・memory・速度、Windows workerの取消/timeout/crash後始末、DLL/モデル導入とアプリ側gateの接続、実音声収録・installer。今回の結果をQwen Windows対応完成や認識品質の合格とは扱わない。

## 続行：Windows実モデル＋公開日本語音声の認識

ユーザーの「じゃあ、テスト音声で試して」を受け、隔離した単体workerで実際のQwen推論を実施した。マイク/PC録音・音声再生・クラウド送信・認証操作は行わない。本体のWindows Qwen gateも変更していない。

### 入力とモデルの保全

- 既存の公開 [google/fleurs](https://huggingface.co/datasets/google/fleurs) validation日本語から3種類：`fleurs-ja_jp-validation-{2-1607,0-1519,3-1634}`。CC-BY-4.0、Conneau et al., FLEURS (2022)。既存manifestの各WAV SHA256と転送後のguest SHA256が一致。参考文とraw結果を別々に保持した。
- 実モデルは `nix/qwen-model.json` のQwen/Qwen3-ASR-1.7B、revision `7278e1e70fe206f11671096ffdd38061171dd6e5`。元の公式ファイルの全サイズ/SHA256と、派生tokenizerの既存SHA256を照合してから、専用share `asr-test-02/model-1.7B` にBtrfs CoWコピーを作成。元モデル・配備済みworkerは変更しない。モデル重みを新たに取得/公開したものではない。
- pool配下がNOCOW継承だったため最初のreflinkはEINVALで失敗。最初の空の試行領域は残し、**新しい専用の空モデルディレクトリだけ**NOCOWを解除して再試行した。pool全体/VM diskの属性やVM資源は変更していない。
- **INT8 cacheは既存の信頼したLinux生成物を再利用**。元cacheのidentityが検証済み元shardと一致することを確認した。Linuxのns単位mtimeはWindows FileTimeの100nsにそのまま移せないため、コピーしたshardだけmtimeをUnix epochへ統一し、コピーしたcacheのoffset16のidentity 8 bytesだけを再計算した。量子化payload/config/layoutはSHA256一致で不変、guestからepochも確認。これはfixture準備であり、製品のcache導入処理やWindows上の実モデル再量子化を検証したものではない。

### Windows結果

- 同じ移植sourceから `cargo build --locked --release` 成功。workerは2,968,576 bytes、SHA256 `dabf6276833880749628b071628995c82f9db9a76043a804bb1e9751597dbf23`。LP64 OpenBLAS 0.3.30、6 threads、本体用ORT 1.22.0をprocess限定指定。既存debug build/アプリ/OS設定は置換しない。
- readyではmodel_loaded=false。最初の発話で実モデルをロードし、**3種類＋最初の再実行の全4要求がrecognized / model_loaded=true / asr_calls=1**。full_input、window_final、confirmed=falseを維持。再実行のraw文字列も一致した。

|順番|音声長|モデルload|推論|要求全体（wall）|
|---|---:|---:|---:|---:|
|村の散策・初回|7.98s|61.63s|23.17s|86.86s|
|ギャップイヤー|11.10s|0s|11.36s|11.75s|
|日本の島々|12.12s|0s|7.54s|8.02s|
|村の散策・再実行|7.98s|0s|5.17s|5.33s|

- 認識例：`興味をそそられる村を30分ほど散策するのは価値があります。`
- OS記録のworker peak Working Set **5.67GiB**。500ms間隔のsampled Private Bytes peak **2.55GiB**。前者はfile-backed mapping等を含み、後者は瞬間最大やシステム全体のメモリではない。8GiB VMで実行できたが、アプリ併用時の余裕・長時間安定性は未確認。
- 初回は**process-cold / INT8 cacheあり**。モデルはZ: virtiofs共有上で、filesystemのcold cache・専用NTFS・実機性能を制御した正式ベンチマークではない。warm要求も1本はRTF>1であり、全条件リアルタイム対応とはしない。
- shutdown→bye→exit 0。cache SHA256は実行前後で不変。終了時C:空き約5.74GiB。所有workerは終了、VMは起動維持。

### Linuxとの比較・限界

Windows worker終了後、前段の未配備Nix release workerで**同じコピーのモデル/公開WAV/順序/6 threads**を逐次再推論した。同じpkgs scopeのORT 1.27.1を明示し、全4要求が実モデル認識、shutdown/exit 0。Windowsとraw文字列は3種類中2種類が完全一致し、島々の文だけLinuxの `本州列島で、世界で` に対してWindowsは `本州列島で世界で`（読点1つの差）。再実行も一致。違いを自動補正せずrawのまま保存した。ビルドのCPU target・BLAS/ORT版・OS/共有I/O条件が異なるため、差の原因や速度差をOSだけに帰属させない。

既存metrics（NFKC/lower-case、Unicode句読点/空白を除外、数詞読み替えなし）では、Windowsの3種類117文字に対してCER=0。これは小さい固定公開fixtureでの結果であり、raw完全一致・一般的な日本語会議品質・長時間/雑音/同時発話の受入ではない。

証拠はGit外 `../runtime/windows-audit/speech-evidence/`：Windows `speech-result.json` / `worker-stderr.log` / `release-build.log`、`fixture-manifest.json`、fresh Linux `speech-linux-result.json`、`comparison.json` と実行script。モデル/音声はGitへ追加しない。本段で本番コード・依存lock・既存Nix作業は変更していない。

**引き続き未実施**：Windowsでの実モデルcold量子化/cache生成、ローカルNTFSモデル導入、取消/timeout/crash後始末、アプリへのworker接続、Windows Mic/PC loopback〜Stop末尾保存の通し試験、長時間/実会議品質、実ChatGPT接続、installer/clean-host配布。単体の実音声認識は確認できたが、Windows版アプリ全体の完成ではない。

## Windowsアプリ接続・NSIS/Pre-release作成（継続中）

ユーザーが `integrate_qwen_then_package` と `publish_prerelease` を明示選択。Windows x64/MSVCの既存Qwen adapterを有効化し、子console非表示、bundled worker/ORTと別導入モデルの既定パスを追加した。Linux経路・ASR/VADポリシー・既存provider既定値は維持。公開配布/push/tag作成は今回行わない。

- Windows adapter単体3件＋子プロセスfixture1件がdebugで成功。float32 transport/分割・Unicodeパス・timeout・取消・crash後再起動・子終了・一時音声削除を検証。初回fixtureの250ms制限は大きいfloat transportに短過ぎたため、正常ケース10秒/専用timeoutケース250msへ分離した。最適化releaseでも**4件成功**（2.34秒）。
- 実公開日本語WAVを同じアプリRust adapter経由で2回認識、repeat一致・明示shutdown・一時音声削除に成功。初回約53.6秒。PCM16 fixtureを読めなかった初回smokeをPCM16/float32両対応へ修正。最初のPowerShell redirectの日本語mojibakeを認識品質の不合格/合格と混同しない。証拠 `installer-01/integration-result.json`, `adapter-real-speech.log`。
- Linux adapter regression **5件成功**、frontend TypeScript成功。Windows UIの明示選択後、handshake成功を確認して既存 `api_save_transcript_config` へ保存するよう補修（Settings画面からの選択を再起動後も保持）。未検証の実録音・cloud・自動provider切替は追加しない。
- `tauri.windows-release.conf.json` は `local.meetily.desktop` / Meetily Local、unsigned per-user NSIS、upstream updater無効。Qwen/initializer・LP64 OpenBLAS/ORT・helper/FFmpeg・VC143 CRT/OpenMP・templates・noticesを配置する。重いASRモデルは別導入。initializerは `model-preparation` feature必須にして既存Linux/Nixのdefault bin集合を変えない。
- build scriptはlocked Cargo/frozen pnpm、hash固定tool download、CPU/AVX2を使用。モデル準備は明示 `-Download` のみネット取得、全公式ファイルSHA検証・排他・既存不一致上書き拒否・local cache生成。既存onboardingは維持（Qwen準備だけでParakeet/summary導入済みを偽装しない）。
- 配布VMビルドの初回 `Invoke-WebRequest` がORTで0-byte stall。所有するpackage processのみ停止し、hashを維持してbounded HTTPS curlへ変更。VS CommunityにCMakeが無かったケースは別VSインストールから探索して解消。
- releaseビルドでC:不足（os error112、windows-sys/rustlsのrmeta/rlib書込み失敗）。独立debug/releaseグラフを作らないrelease testsへ変更。所有debug targetの再生成可能incrementalのみ削除し、残り**11,278 files / 6,495,768,827 bytes**を共有領域へコピー、件数/総bytesと主要exe/DLL SHA照合後にC:側cacheを削除した。元baseline exe SHA `6e82645ee9d3ffcf5d97ca6568d12c12b743c72c7e70c415d441ac19f7b31792` は退避先でも一致。C:空き7,568,674,816 bytesで再開。ソース・モデル・profile/ユーザーデータは削除せず、VM容量/メモリ/ネット設定も変更しない。
- `.github/workflows/windows-prerelease.yml` を定義。Windows用タグとbase versionを照合し、install/DLL-path/synthetic silence/helper/FFmpeg/uninstall成功後のみforkのPre-releaseへexe/SHA/metadata/smoke結果を添付。manualはActions artifactのみ。stable/draftの上書き・latest化・上流repoでの公開を禁止。action commit pinは公開tag refで照合、`actionlint`成功。実GitHub実行/push/tag/公開は未実施。

### 最初のNSIS candidateとWindows新規cache

- 最初のNSIS生成成功：49,213,197 bytes、SHA256 `eaa500c7330f376326304452fd054fc898645ccfd581c36c37dcfc781a8fab59`。このcandidateは最終配布物ではない。隔離per-user install後、開発用PATH無しでinstalled workerのready→float32無音→no_speech（model_loaded=false/asr_calls=0）→bye/exit0、OpenBLAS/ORTのinstalled DLL実ロードpath、helper ping/pong→shutdown/goodbye/exit0とapp-local VC CRT/OpenMP path、FFmpeg8.0.1/exit0を確認。所有uninstallerでアプリ/worker削除も成功。
- ASR公式9ファイルだけの独立CoWコピー `installer-01/model-cold-01` を作り、cacheが存在しないことを確認して**installed** `prepare-qwen-model.ps1` を `-Download` 無しで実行。全公式ファイルSHA照合後、Windows initializer自身で新規量子化/cache保存/再open検証に成功（exit0）。総437.24秒は共有Z:の全ファイルSHA検証を含み、純粋な量子化時間/NTFS性能ではない。initializer peak Working Set4.77GiB、1秒sample Private Bytes peak2.81GiB（瞬間最大やfull-app会議メモリではない）。
- 生成cache **1,724,760,064 bytes**、全体SHA `e210f429eca643153c005b11594adcd557305f0fc2604d696c8117179e7d5e4d`。先のLinux由来epoch-mtimeコピーcacheと**ファイル全体がbyte-identical**、offset24以降payload SHA `96e4ad3f590d30941305c4e7350450247ca9109226b7f2d87661cde73758a3dd`。元cacheをfresh dirへコピー/metadata書換えして生成を装ったものではない。公式9ファイルも生成後にhostで全SHA一致を再確認。元モデル/元cacheは変更しない。
- そのWindows新規cacheとinstalled worker/ORTをアプリRust adapterから実公開日本語WAVで2回使用し、raw `興味をそそられる村を30分ほど散策するのは価値があります。`、repeat一致・shutdown・Unicode一時音声削除に成功（初回wall70.24秒、一般性能保証なし）。UTF8 stdoutとして正しく保存できた。証拠 `model-init-result.json` / `installed-public-speech.log`、host `windows-cold-cache-comparison.json`。
- model検証の高遅延filesystemで4KiB readを繰り返さないよう、最終scriptは1MiB/SequentialScanでbufferした同じSHA256を計算する。cache生成アルゴリズムや公式hashは変えない。
- host Node distribution/sidecar contract **14件成功**、workflow内artifact/Pre-release guardの実Python **13ケース成功**（Windows tag/version・unsigned/identifier・uninstall結果・real-audio拒否・stable/draft拒否）。GitHub APIは呼ばない。Windows PowerShell5 parserでpublic3 scripts＋GUI/finalize2 scriptsのsyntax error0。最終source ZIPは77 files、SHA `b9ba9b11ae4a47225bdbc65402d5c3911e7d2b18bd3465aa4f5713e34ef7ff66`。

### 最終candidate・GUI・再install/uninstall完了

最終NSIS **49,205,405 bytes**、SHA256 **`4bd39d072e200b3a0a4d394f8cadcb36dc4f84d99a699a2da0059df3752033ad`**。Git外配布物：`../runtime/windows-audit/installer-evidence/artifacts/Meetily Local_0.4.1_x64-setup.exe`、同名 `.sha256`、`windows-build.json`、`installer-smoke.json`。installed main SHA `07f50026c758751899249f0c279ae9ca39bb89aea95bcb69b5743b022f079abb`、worker SHA `f598c862d8cc8decdb3ba365c8a233b87531e8dc5cd14d9f349c7471ed3ec7ed`。最終sourceでworker release17件、app release4件、Node14件が成功。clean checkoutでcustom-protocol fixtureがmissing frontendDistにならないよう、fixture前にNext exportする（Tauri本番buildの既存beforeBuildも維持）。

- 最終installerを再度隔離install。開発用PATH無しの**実installed release GUI**でnative IPC、Qwen supported=true、bundled worker/ORTと既定local modelパスを確認。Welcomeの実DOMリンク→Settings→Transcription→Qwenを選び、モデルパス/日本語をDOMで入力、**Validate and select Qwen**で実handshakeとSQLite `provider=qwen/model=Qwen3-ASR-1.7B-INT8` 保存を確認。単なるReact状態注入ではない。実録音は開始しない。
- 最初のGUI assertionは有効なWindows verbatim prefix `\\?\` と通常パスの文字列差を誤検出。test側の比較だけを正規化し、productionパスは変更しない。その後Radix tabは単なる `.click()` で切り替わらず、Summary側の候補を開いてtimeout。実際のmousedown/clickとactive-tab確認へtestを修正し成功。失敗DOM/JSONと所有synthetic profileを別名で保持した（本体不具合として隠さない）。
- GUI起動時PATHはSystem32/Windowsのみ。子Qwen PID1件を確認し、所有GUI終了後に子とloopback CDP9341 listenerが終了。録音/ChatGPT loginなし。GUIのlanguage/model/profileは人工状態として退避し、ユーザーの通常profileを置き換えない。
- **最終installed** `prepare-qwen-model.ps1` を既存Windows生成cacheにoffline実行：46.98秒/exit0。1MiB-buffer SHAによる全公式ファイル検証とfinal initializer/model load/cache validationを実行（cold量子化時間との直接比較ではない）。その後**最終worker**をapp Rust adapterから公開日本語WAVで2回認識し、同じraw散策文・repeat一致・明示shutdown・Unicode一時音声削除、exit0（初回wall48.61秒）。
- 所有GUI candidateをuninstallし、人工profileはmarkerを照合して専用Dev領域へ**退避**（削除しない）。同じ最終exeをさらに新規パスへinstallし、CI既定と同じworker/DLL/helper/FFmpegテスト＋uninstallを完遂。`installer-smoke.json` はstage=complete/keptInstalled=false。C:空き6,370,193,408 bytes。検証アプリ/workerは終了、VMは起動維持。
- 実最終exe/SHA/metadata/第三回install-uninstall結果に対して、workflowのpublish前Python verifierそのものもhostで成功。証拠は `../runtime/windows-audit/installer-evidence/` とその `SHA256.json`。GitHub API呼出し・commit/push/tag作成・Release公開は行っていない。実Actions runの成功とは区別する。

### PC音声についての追加質問と未検証範囲

ユーザーから「Windowsのマイクだけでなくスピーカー音も取れるか」の質問。実装上、現在の `RecordingManager → AudioStreamManager → AudioStream` はWindows出力デバイスを `get_windows_device` で取得し、`build_input_stream` に渡す。依存CPAL0.15.3 WASAPI backendはrender endpointに `AUDCLNT_STREAMFLAGS_LOOPBACK` を設定するため、PC再生音をマイクと別取得する経路はある。旧 `capture/system.rs` の非macOS stubだけを見て非対応と判断しない。

ただし今回、**実際に再生したPC音声のloopback取得もマイク実録音も行っていない**。「ちゃんと録れる」は未実証と回答。実再生/録音には別途、対象音源/endpoint/マイク無効化等を明確にした承認・試験が必要。Mic/PC〜Stop末尾保存/復旧の通し試験、NTFSモデル実導入、clean-host全依存closure、長時間/雑音/同時発話/実会議品質、実ChatGPT接続も未検証。installer生成・GUI選択・公開ファイル認識の成功を全Windows機能の受入としない。

### 合成PC音のみの取得試験：承認済み・VMロックで保留

ユーザーは `synthetic_pc_loopback_only` を選択。マイク・実会議・ASR/cloudを起動せず、VM内の合成音だけを対象とする。製品の `AudioStreamManager` へ `microphone=None` / output endpointのみを渡す明示実行用example `windows_pc_loopback_smoke.rs` を準備した。別processで全active render endpointの音声sessionを監視し、他processのactive音声があれば開始拒否・lease失効で停止/未保存にする試験runnerもGit外に準備した。**いずれも現時点で未ビルド・未実行の試験コード**であり、製品installerは変更しない。

VMへの起動操作後もrunnerのresultファイルがなく、画面を確認するとWindowsのロック画面/PINエラー表示だった。起動コマンドがロック画面入力になった可能性があるため、追加ログイン試行を停止。資格情報を推測・保存せず、ユーザーによるログイン待ちとする。合成音の再生・loopback取得はまだ行っておらず、実取得の成功とは記録しない。再開時はまず画面のログイン済み状態を確認してから起動操作する。

### 合成PC音のWASAPI loopback実取得：成功（VMの仮想出力限定）

ユーザーの `unlocked_resume` 回答後、デスクトップとRun dialogの表示を確認して再開。`windows_pc_loopback_smoke` をWindows releaseでbuildし、製品の `AudioStreamManager → AudioStream → AudioCapture` そのものをPC-onlyで動作させた。PCMをcapture channelへ注入するfixtureではなく、別のCPAL output streamで合成音を実際にrender endpointへ再生し、WASAPI loopbackから返ったchunkだけを取得した。

- Endpoint：**Speakers (PWEC Virtual Audio Device)**、render 48kHz/stereo/F32。取得は48kHz/mono/F32、**282,240 samples / 5.88秒 / 588 chunks**。peak0.12883155、backend errors0。マイク引数は常にNone、stream数1、microphone device未設定をassert。ASR/VAD・会議DB・GUI profile・cloudは起動しない。
- 523.25 / 739.99 / 997Hzの合成音を各0.8秒、間に0.4秒の無音で再生。保存WAVの独立したGoertzel検証で**周波数の優勢成分・順序・各0.8秒・無音間隔が一致**。検証器は正常波形と、無音/逆順/別周波数のnegative fixtureでも検証した。
- Stop後にcapture queueをdrainし、250ms後の追加chunk無し・state停止・stream数0をassertした後、**試験専用WAV writer**で保存。WAV SHA256 **`f2173433b20df6b0fffddb295470cd1dbd6c8f0b831617a842887f57fdf77435`**。これはGUIのRecordingManager/mixer/checkpoint/AAC保存・復旧の通し受入ではない。
- 別processの安全監視：開始前active render session0、実行中104回確認、試験自身のactive sessionを検出し、他processのactive音声検出なし。完了後の再確認でcapture process0 / active render session0 / 有効lease無し。所有のNoExit操作shellだけを識別して終了し、VMは起動維持。製品capture関連4ファイルのguest SHAがhost sourceと全一致。
- 試験runner自身の失敗も保持した。最初はIMMDeviceCollection GUID誤記でpreflight拒否。次はPowerShellのnull→空文字変換によるFile.Replace引数エラー、その次は置換時の一時的ENOENTでlease失効。いずれも停止し、失敗scope2個にWAV無しを確認。最終runnerは固定長recordを同じファイルへtruncate/renameせず更新する方式で成功。製品音声codeの変更ではない。
- ログイン直後のUI反映遅延で起動文字列の先頭`po`が別のメモ帳bufferへ混入した。対象末尾の2文字だけを除去して148文字・7行目先頭へ戻し、保存操作はしなかった。以降はmodifierを100ms保持し、Run dialog表示確認後に入力した。

証拠はGit外 `../runtime/windows-audit/pc-loopback-evidence/`（capture/guard/post/spectral JSON、合成WAV、試験source/runner、失敗JSON、SHA256 index）。この追加検証で製品installerを再生成・公開しておらず、最終NSISの既存SHAは不変。実機の物理スピーカー/ヘッドホン、Bluetooth切替、GUIから会議保存まで、実マイク/実会議/長時間音声は引き続き未検証。

### 初回GitHub Actionsによる公開試行：RC1のinstall gateで停止

ユーザーのpush承認（Nix含む全変更）により `bc2b831` を `m-tky/meetily:feat/live-minutes` へpush。その後のRelease作成依頼で同commitに `windows-v0.4.1-rc.1` を付け、[Actions run 38841402698](https://github.com/m-tky/meetily/actions/runs/38841402698) を初実行した。Windows2022で固定toolchainの導入と**NSIS生成・native testsを含むbuild stepは成功**（36分）。しかしinstall後のclean-PATH worker起動でstdout応答前に終了し、install gateは失敗、publishはskip、Releaseは未作成。VM成功とCI失敗を混同しない。

初版は失敗時にartifact uploadをskipしており、worker stderr入りJSONを回収できなかった。失敗ログはGit外 `../runtime/windows-audit/prerelease-rc1-failed.log` に保持。原因を推測でDLL/CPU問題に断定せず、次のRCでは(1)高コストな本体buildより前にstaged workerをclean-PATH起動しready/shutdownを検証、(2)processor count・stderr・終了code/hex・runtime inventoryと失敗時のDLL依存を記録、(3)失敗時のみUNVERIFIED diagnostics artifactを保存する。インストール失敗時にもworker stderr/終了codeをconsoleへ出す。publishのbuild成功必須条件・インストールgate・署名/モデルポリシーは緩めない。RC1タグは書き換えない。

診断追加の `8b3f7b8` / `windows-v0.4.1-rc.2` / [run 38845278807](https://github.com/m-tky/meetily/actions/runs/38845278807) は早期probeで停止し、UNVERIFIED artifact回収に成功。processorCount=4、exit=1、stderr=`BLAS thread count does not match worker threads`。DLL loader crashではなく、環境変数指定だけでは要求6とOpenBLAS起動時countが一致しないことが直接原因と判明した。次のRCではworkerの既存予算を `openblas_set_num_threads` で明示適用してから**引き続き厳密一致を要求**する。requested=6の既定値・protocol・ASR/VADは変更せず、1→6への明示適用と元の値への復元を行うnative回帰testを追加。既存RCタグは書き換えず、公開gateを通過するまでRelease未作成のままとする。修正後LinuxのNix worker buildでlibrary14件＋binary4件（明示BLAS予算回帰を含む）が成功し、別途4論理CPUだけにaffinity制限した実workerでもrequested6/ready→shutdown/exit0を確認した（モデル/音声無し）。Git外 `prerelease-rc3-linux-worker-tests.log` / `prerelease-rc3-four-core-startup.json` に保持。Windowsでの最終判定は次のActions結果で行う。

### RC3：Actions全gate成功・Pre-release公開完了

`d88e815` / `windows-v0.4.1-rc.3` / [Actions run 38846279279](https://github.com/m-tky/meetily/actions/runs/38846279279) は **success**。Windows workerはlibrary14＋binary4件（BLAS予算回帰含む）、app adapter4件、Node15件が成功。staged clean-PATH起動、NSIS生成、installed worker無音/実DLL path、helper ping/shutdown＋CRT、FFmpeg、uninstallの全gateが成功し、publish jobもsuccess。

公開先：[Meetily Local Windows windows-v0.4.1-rc.3](https://github.com/m-tky/meetily/releases/tag/windows-v0.4.1-rc.3)。draft=false / prerelease=true、stable/latestにはしない。annotated tagの実commitは `d88e8156156b21df87708836365531024d5497e2` とAPIで確認。公開4assetsはinstaller / `.exe.sha256` / `windows-build.json` / `installer-smoke.json`、モデル/音声/DBは含めない。

公開installer **`Meetily.Local_0.4.1_x64-setup.exe` / 49,212,462 bytes / SHA256 `cadd2e7fb084a0b16aa963e76a12d92e16ee2ef05b3e59beeb651630654e63e4`**。worker SHA `67c94417c6814d31ac7cb4ae4447734e8bd18005db89132b337cc56c947c86c0`。公開後に全assetsを再downloadし、binary hash・metadata・install/uninstall結果を確認した。

GitHubはasset名の空白を`.`へ正規化するため、初回upload後のchecksum内filenameとmetadata.nameだけが公開名と不一致だった。**installer bytesは変更せず**、Pre-releaseの2つのsidecarのfilename参照だけを公開名へ揃えて再uploadした。元sidecarもGit外に保持。将来のbuildもNSIS成果物を配布用directoryへcopyする段階で同じ正規化を行い、hash/metadataを作る。公開成功log・download物・検証材料はGit外 `prerelease-rc3-success.log` / `prerelease-rc3-release/` / `prerelease-rc3-original-sidecars/`。以前のVM installer SHA `4bd39d…` は過去candidateの証拠であり、このCI版のSHAとは異なる。実機/会議品質等の未検証範囲は前節のまま。
