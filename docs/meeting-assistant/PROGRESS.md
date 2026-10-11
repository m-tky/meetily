# 開発状況（2026-10-11）

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
