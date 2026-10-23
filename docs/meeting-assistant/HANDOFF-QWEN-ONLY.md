# 次のエージェントへの引き継ぎ：Qwen-only ASR

## このcheckpointの目的

ユーザー依頼「あるていどのところまで進んだら、別のエージェントに渡すようのドキュメントを作って、pushしておいて」に対する引き継ぎ。**実装＋隔離fixture GUI回帰が完了した区切り**で停止する。製品完成・実0.6B認識・releaseの合格ではない。

- 作業repo: `/home/user/Code/meetily`
- branch: `feat/live-minutes`、push先: `origin` (`m-tky/meetily`)
- この変更の直前commit: `252cf6f`（Linux ASR-only onboarding）。この文書とQwen-only実装を同じcheckpointでcommitする。実際のcommit IDは `git log -1` で確認する。
- tag作成・release公開・workflow手動dispatchは行わない。

## 最初に読むもの（優先順）

1. repo直下の `AGENTS.md` / `CLAUDE.md`：先頭の最新overrideを優先。
2. [QWEN-ONLY-ASR.md](QWEN-ONLY-ASR.md)：今回の仕様、全検証、失敗、再現コマンド。
3. [PROGRESS.md](PROGRESS.md)：先頭のQwen-only節が現在地。下部は履歴で、旧engine維持／batch未対応という記述は失効。
4. [BASELINE.md](BASELINE.md) / [ADR-002-OFFICIAL-SIWC.md](ADR-002-OFFICIAL-SIWC.md)。要約に触れる場合は [LUNA-SUMMARY.md](LUNA-SUMMARY.md) も読む。

## 確定したユーザー判断

- ASRは **Qwen3-ASR 1.7B / 0.6Bのみ**。小は0.6B、既定は1.7B。
- 旧Whisper／Parakeetは選択肢を隠すだけでなく、実装・IPC・UI・不要依存まで削除することを明示選択済み。fallbackとして復活させない。
- live録音・音声import・保存音声retranscriptionを共通Qwen adapterへ接続。
- 入力別文字起こしはMic／PCという取得元だけ。人物推定・diarization・ForcedAligner採用はしない。
- cloudは明示操作が必要。既存公式SIWC／Luna設定・認証・要約経路は今回変更しない。Codex/pi資格情報を利用しない。

## 実装の入口

| 領域 | 主なファイル |
|---|---|
| native model catalog / 選択 | `frontend/src-tauri/src/audio/transcription/{qwen_model,engine}.rs` |
| live・batch共通adapter / 個別設定 | 同directory `qwen_provider.rs` |
| batch所有worker / 排他lease | 同directory `batch.rs`、`audio/{import,retranscription}.rs` |
| 同意付きdownload / cache初期化 | 同directory `qwen_setup.rs`、`nix/qwen-model*.json` |
| 旧設定のatomic条件付き移行 | `frontend/src-tauri/src/database/repositories/setting.rs` と `setting/tests.rs` |
| worker identity / tensor shape | `benchmarks/qwen3-asr/worker/src/{main,lib}.rs` |
| UI catalog / model selection | `frontend/src/lib/qwen-models.ts`、`components/{TranscriptSettings,QwenModelSettings,QwenAutoSetup}.tsx` |
| native GUI回帰 | `frontend/tests/gui/{qwen_batch_native_smoke,recording_native_smoke}.py` |
| runtime startup probe | `scripts/check_qwen_worker_catalog.py` |

DB schemaと旧key／model filesは保持。旧providerだけconditional SQLで移行し、同時の明示Small選択を上書きしない。Unknown Qwen IDは拒否。Batchはmodel/language固定・取消・temp cleanup・録音/setup/model変更との排他をnativeでも実施。失敗・空認識・取消で既存本文を消さない。

既存1.7B workerの2引数CLI互換は維持。新workerは両サイズを識別する。`--model`付きweightsなし起動は `model_loaded:false` のruntime probeであり、推論ではない。

## 通過した検証と証拠

前sessionの最終logをこの引き継ぎ時に再確認した。native/build/GUIをこの引き継ぎsessionで再実行したわけではない。

- Native **59 PASS＋既存import fixture 1 ignore**。worker **19 PASS**、実runtime起動probe **6条件PASS**（weightsなし）。
- Frontend **Node46＋isolated Bun65 PASS**、TS／隔離Next export／locked offline embedded-debug build／Nix derivation評価／Python compile成功。
- 最終 `compat-native-gui.log`：両モデルのSettings選択→他サイズoverride import→選択サイズretranscription→JSON/SQLite一致→推論失敗／取消時の旧本文保持→native PID変更を伴うrestart PASS。held inference中のsetup／model-path変更／録音preflight拒否もPASS。
- 同じ最終log：各サイズ別fresh profile＋private Pulse/null sink＋eSpeak＋実capture/VADでStart→manager除去後Stop-tail→保存／event-raw-DB一致／workerとtemp cleanup→native restart/reopen PASS。
- **GUIのASRは全て意図的なfixture worker**。本物のmodel weights／0.6B speech推論・精度を確認したことにはならない。
- 引き継ぎ時に `node --test frontend/tests/lib/*.test.mjs` を再実行し **46/46 PASS**。`git diff --check` も成功。
- 引き継ぎ用の補助scanはbare shellに`python3`がなく初回失敗。Nodeで再実行し、新規filesがsource／manifest／文書のみであることと、既知credential形式の簡易diff scanを確認（網羅的secret監査ではない）。`git fetch origin feat/live-minutes`後、既存HEADはremoteより6 commits先・behind0。今回のcheckpointとともに既存の未push履歴も通常の非force pushで送る。

ローカルlog root（Git対象外）：`/home/user/logs/meetily-qwen-only/`

主要log：`native-tests-r4.log`、`worker-tests-r2.log`、`worker-probes-r1.log`、`node-complete.log`、`compat-native-gui.log`、`nix-eval-final.log`。引き継ぎ再実行は `handoff-node-tests.log`。初回cold-dependency／BLAS link／sidecar／TS／GUI-harness失敗も同directoryに残す。成功logだけを理由に過去失敗を消さない。

## 同じhostでのbuildと実行物

- 本体debug executable: `/home/user/Code/meetily/target/debug/meetily`
- embed済みfrontend export: `/tmp/meetily-qwen-only-frontend-iMMhiW/frontend/out`
- worker debug build: `/tmp/meetily-qwen-worker-target/debug`
- これらはローカル一時artifactで、別hostへpushされない。存在を確認してから使用し、Git checkoutだけで再現済みと誤認しない。
- flake-pinned pnpm **10** のshellを使用。pnpm12を同じnode_modulesへ混在させない。`--locked`を維持。
- Cargo.lockは旧ASRに不要となった9 packagesのみprune。残存package versions／pnpm lock／standalone worker lockは不変。ONNXはSilero VAD用に残る。
- 手順はQWEN-ONLY-ASR.mdと `frontend/tests/gui/README.md` を参照。GUIは必ずfresh HOME/XDG/private D-Bus/Xvfb、無効実機ALSA、private Pulse（必要な試験だけ）、`MEETILY_DISABLE_ANALYTICS=1`、screenshotなし。

## 次に進む順序と承認境界

1. **まずコードのみのpackaging/runtime監査**が安全。Windows/Linux CIに旧ASR由来のコメント／portability guard／build recipeが残る可能性がある。旧releaseが通ったことをQwen-only版の合格とみなさず、worker/initializer/ORT/LP64 BLAS・2 manifestsの同梱と起動契約を調べる。古い加速用ドキュメントも新仕様の根拠にしない。release build／公開は別承認。
2. 次の本物の推論回帰は **公開fixture＋隔離local worker** の0.6Bを候補とするが、今回のGUIはfakeであり未実施。既存weights/cacheの場所・provenance・利用許可を確認する。新規大型download／実会議／cloud送信を勝手に追加しない。
3. 完全な新規model HTTPS download→init→selection→native restartは両サイズの未検証範囲。既存modelからのseed／cache再生成とdownload完走を区別する。
4. 表示中アプリの反映を求められたら、**保存済み・windowを閉じたことの確認を先に取得**。通常Nix app／旧preview／profile／ログインは保持し、対象のowned helperだけ扱う。現debug buildを表示中appへ反映済みとは伝えない。

追加で未検証：Windows実機／clean host／長いimport・retranscription／multi-hour・hard crash／自然会議の品質。旧short-word/noise認識失敗は未解消。source-syncやDSPの過去限界はモデル二択化で解決しない。

## 操作してはいけないもの

- 実mic／host desktopの自動capture、physical speaker再生、実会議・ユーザー本文／音声／native録音logの読取、cloud inference。
- app資格情報の内容・token・auth URL・login後のbrowser command lineの表示、Codex/pi credential流用。
- 通常profile migration実行／user DB書換／既存model削除／Home Manager switch／表示中appの無断再起動。
- 追加ASR fallback／既定model・language・gain・VAD・audio processing変更／release・tag公開。

このcheckpointでpushするのはsource・tests・manifests・文書のみ。音声、モデル、アプリDB、資格情報、bulky build artifact、private raw logはGitへ入れない。
