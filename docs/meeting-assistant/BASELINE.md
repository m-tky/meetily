# Meetily Linux baseline / 土台の再比較

対象: upstream `a2cb62e827da7ef59f65064c97233efb2313878e`、app v0.4.1。
ブランチ: `research/linux-baseline`。日時: 2026-10-11。

## URL・ソース

ユーザーが提示した https://github.com/Zackriya-Solutions/meetily と、以前cloneした meeting-minutes は同じrepo。GitHub APIのfull_name / redirect / 現行HEADで一致を確認。Rust/Tauri 2 + Next.js 14で、旧Python/FastAPI backendはarchiveで非サポート。現行Rust coreを使う。

## 完了した検証

- 同一NixOS環境（Ryzen 5 PRO 5650GE、RAM約29GiB、PipeWire、Wayland/niri）で、upstream Cargo.lockを変更せずCPU build。
- `cargo build --locked -p llama-helper`: 成功（3m16s）。
- `cargo check --locked -p meetily`: 成功（3m12s）。
- `cargo build --locked -p meetily`: 成功（4m39s）。共有targetのdebug実行ファイルは約577MB（releaseサイズではない）。
- FFmpeg 7.0.2-staticとhelperをTauriのbinariesにstage。FFmpegのGPLv3+条件は再配布前に別途確認する。

## フロントエンド検証上の自分の操作ミス

最初のshellはpnpm 12.3.4だった。pnpm 12はpackage.jsonのpnpm.overridesを読まなくなっており、upstream frozen lockとの設定不一致でinstallが拒否された。その後のexec auto-installとpnpm 10 installを混在させ、依存のpeer suffixとroot symlinkが不一致になってTSがmodule-not-foundとなった。**このTSエラーはupstreamのコード欠陥と扱わない**。

検証用shellを `pkgs.pnpm_10`（10.34.5）へ固定。混在したgenerated node_modulesをruntimeへ退避し、こちらのauto-installが変えたpnpm-lock.yamlをupstreamへ戻した。frozen+offlineでclean install、type check、production buildを再実行し、すべて成功。Cargo/pnpm lockはupstreamのまま。最初の混在installは比較材料から除外する。

## フロントエンド・GUIの最終結果

- pnpm 10.34.5のclean install、`tsc --noEmit`、Next.js production build: 成功。
- localhost dev server: HTTP 200、初回compile約11.4s。Google font取得のretryあり（完全offline buildは未保証）。
- 隔離XDG data/config/cacheとXvfb :98でnative実行ファイルを起動。**Welcome to Meetilyが実表示され、SQLiteのfresh DB初期化まで成功**。証拠: `evidence/linux-onboarding.png`。
- model downloadボタン・録音ボタンは押していない。モデル自動scanは4件、推奨summary modelはqwen3.5:4b、ready=false。マイク/通話の収録、ASR品質、復旧、長時間安定性はこの起動試験に含めない。
- native buildとGUI表示の両方をMurmurと同じLinux環境で確認できた。ただし実Wayland compositor上の表示やWindowsは未確認。

## アプリ側の変更範囲

- Nix開発shellのみ追加。
- telemetryの検証用environment opt-outを1箇所追加: `MEETILY_DISABLE_ANALYTICS=1`。PostHog clientはenabled=falseとなる。baseline試験でusage metadataを送らないためであり、完成アプリのopt-in UXはまだ作っていない。
- 録音、ASR、DB schema、LLMは変更していない。

## 比較（実装の所在と検証を分ける）

| 項目 | Meetily Community | Murmur / 今回のbranch |
|---|---|---|
| 土台 | 原upstream、native Rust core | Meetilyから派生、native Rust core |
| frontend | Next.js/React、BlockNote | 同系統に機能拡張 |
| ASR | Whisper.cpp / Parakeet | Whisper.cpp / Parakeet |
| 日本語/Fun-ASR | 音声品質・速度未検証 | 音声品質・速度未検証 |
| 話者分離 | Community側にsherpa実装を見つけていない | sherpa-onnx + 話者編集実装あり、実品質未検証 |
| 継続議事録 | 未実装 | 今回の実験版のみ。小型LLMの意味fixtureは失敗 |
| 保存 | SQLite、checkpoint録音の実装あり、障害試験未実施 | SQLite、checkpoint/復旧強化の実装あり、障害試験未実施 |
| ChatGPT plan公式利用 | 未実装 | 未実装、旧非公式経路は無効化 |
| Linux native build | 今回成功 | 今回成功（sherpa API追随修正が必要だった） |
| Linux内部音声 | ALSA/CPAL基盤。現ホストのPulse monitor取得は未解決 | 同様。名前の不一致修正だけでは解決しない |
| dependency lock | upstreamにCargo/pnpm lockを同梱 | 元はignore、今回生成しcommit |
| Windows実機 | 未検証 | 未検証 |

Meetilyを土台に変えるだけで日本語ASR、Linux monitor、リアルタイム議事録が完成するわけではない。Murmurを丸ごと捨てず、必要なMIT実装（話者分離、復旧、小さな議事録prototypeとテスト）は選択的に再利用できる。

## 再現

```bash
cd meetily
nix-shell shell.nix
pnpm --version # 検証時10.34.5
cd frontend
NEXT_TELEMETRY_DISABLED=1 pnpm install --frozen-lockfile
pnpm exec tsc --noEmit
NEXT_TELEMETRY_DISABLED=1 pnpm build
cd ..
cargo build --locked -p llama-helper
mkdir -p frontend/src-tauri/binaries
cp target/debug/llama-helper frontend/src-tauri/binaries/llama-helper-x86_64-unknown-linux-gnu
cargo check --locked -p meetily
cargo build --locked -p meetily
# GUIは録音を開始せず、専用XDG_DATA_HOME/XDG_CONFIG_HOMEで起動。
# MEETILY_DISABLE_ANALYTICS=1 を必ず付ける。
```

実行ログ: 親ディレクトリ `../logs/meetily-baseline-build.log`, `meetily-clean-frontend-check.log`。
Murmurの保存checkpoint: branch `feat/live-minutes`, commit `93b8397`。正式認証の仕様は同branchの `docs/meeting-assistant/ADR-002-OFFICIAL-SIWC.md`。

## 推奨（最終承認前）

原upstreamのMeetilyを土台にして、Murmur由来の必要なMIT機能を選択的に取り込む。理由は同じ録音/ASRの基盤が実起動でき、upstream lockで再現でき、今回の目的に不要な派生機能まで抱えずに進められるため。Murmurの話者分離・復旧は有用な参照実装として保持する。

これは品質・速度の優越を実測した結論ではない。日本語ASR・長時間の実証が必要な点は両候補共通。

**その後ユーザーがMeetilyへの切り替えを明示承認。** feat/live-minutesへ継続議事録と公式SIWCを実装中。現状はPROGRESS.mdを参照する。
