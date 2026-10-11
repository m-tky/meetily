# ADR-002: 公式Sign in with ChatGPTによるLLM接続

状態: 要件・方式を確認済み。Meetilyに公式フロー/公開API providerの実装と人工fixtureテストを追加。**ユーザーが実ログインとLuna人工文テストの成功を報告**（PROGRESS.md）。実会議の品質・長時間・renewalはまだ未検証。

ユーザーは `gpt-6-luna` の利用とアプリ独立のログインを選択し、その後「非公式のCodex互換認証ではなく、最近公開された公式の方法」を必須とした。土台の再比較後、ユーザーがMeetilyへの切り替えを明示承認した。現在はMeetilyのfeat/live-minutesで実装している。

## 確認した公式仕様（2026-10-11）

- [OSS向け概要](https://developers.openai.com/siwc/token-sharing-open-source/)
- [登録・サインイン](https://developers.openai.com/siwc/token-sharing-open-source/sign-in)
- [アカウント・セッション](https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions)
- [モデル・推論](https://developers.openai.com/siwc/token-sharing-open-source/models-and-inference)
- [token reference](https://developers.openai.com/siwc/token-sharing-open-source/token-reference)
- [エラーと回復](https://developers.openai.com/siwc/token-sharing-open-source/errors-and-recovery)
- [preview制約](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations)
- [UI/UX](https://developers.openai.com/siwc/ui-ux-guidelines)
- [公式Codex app-server経由の構成](https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server)
- [ID token検証例](https://developers.openai.com/siwc/website)
- OpenID discovery: `https://auth.openai.com/.well-known/openid-configuration`

これはCodex CLI用のclient IDを流用して非公開APIを呼ぶ方式とは違う。

## 必須仕様

1. 初回に安定したhost ID（例: `urn:uuid:<UUIDv4>`）を生成・保存。再起動やサインアウトで変えない。
2. 初回登録は `client_id=dynamic_agent_client`, 実際のアプリ名の `agent_name_hint`。発行されたclient IDをユーザー/workspaceの登録に保存し、再ログインで再利用。`dynamic_agent_client`はtoken交換に使わない。
3. authorize: `https://auth.openai.com/api/accounts/authorize`。listenerを先にbind。redirectは**127.0.0.1**のHTTP loopback（localhostに置換しない）。同一試行のport/path/URIをcode交換でも維持。
4. 毎試行state・nonce・PKCE S256を新規生成。state、callbackエラー、発行client ID整合を検証してから交換。新ログイン中は既存アカウントを勝手に置換しない。
5. scopes: `openid profile email offline_access resource.invoke chatgpt.tokens.use.direct`; resource: `https://api.openai.com/v1`。
6. token: `https://auth.openai.com/api/accounts/oauth/token`、form encoded/public client、client secret不要。token responseの実際のgranted scopesでplan利用可否を判定。identityだけの同意を推論権限と解釈しない。
7. 維持されたJWT/OIDCライブラリでOpenAI JWKSによる署名・issuer・issued client IDへのaudience・exp/iat/sub・nonceを検証。JWTのpayloadをbase64 decodeしただけでは検証にならない。OpenID discoveryで確認した署名algorithmはRS256。
8. 登録と検証済みidentityを分離保持。emailだけで登録を結合しない。同じemailでもclient ID/workspaceは異なり得る。選択中アカウントをUIに表示。
9. 資格情報はアプリ専用の保護領域。原子的に保存、Unix 0600。WindowsではCredential Manager等の保護を検証。Git・ログ・ブラウザstorageへ保存しない。既存Codex/pi認証ファイルをコピー・更新しない。
10. refreshはその登録のissued client ID、refresh token、resourceを使い、同一セッションの更新を直列化。回転refresh token・access token・expiry・scopesをまとめて保存。temporary errorでは消さず、terminal refresh errorだけ利用停止。
11. サインアウトではdiscoveryのrevocation endpointへrenewable sessionの失効を試行。ネットワーク障害で遠隔失効を確認できなければ利用者へ通知し、ローカルtokenは使用停止。identity/client mappingとhost IDは保持。
12. モデル一覧は選択アカウントのaccess tokenで `GET https://api.openai.com/v1/models`。`models`配列の `visibility=list`, `slug`, `display_name`を使用。Codexキャッシュやfallback一覧を利用権限の証拠にしない。
13. 推論は**公開** `POST https://api.openai.com/v1/responses`。`store=false`, `stream=true`, instructionsまたはdeveloper message。本文のsystem messageやpreview非対応parameter（temperature/top_p/max_output_tokens/previous_response_id等）を送らない。
14. `response.completed`を確認するまで成功にしない。failed/incomplete、途切れたSSE、開始後の利用制限エラーを扱う。usage limitでは継続更新をpauseし、[ChatGPT Usage](https://chatgpt.com/settings/usage)へ誘導。別課金先/モデルへ黙ってfallbackしない。
15. 初回のplan利用を説明し、モデル選択の近くにUsing ChatGPT planとManage usageを表示。音声はローカルに留め、クラウドを選んだ場合に文字起こし・議事録を送ることを明示。

## 現時点の安全措置

Murmur由来の `backend-api/codex` 接続は新しい公式フローではない。公式実装を完了したと誤認させないため、Tauriの旧login/model-listとLLMの旧subscription経路をfail-closedで無効化した。既存資格情報には触れていない。新しいauth依存だけを追加して未使用のままにする変更も取り除いた。

Meetilyに新規のsiwc.rsを実装し、旧モジュールは移植していない。Unix owner-only atomic files、Windows user-scope DPAPIを採用。署名・nonce/state・scope・file-lock・SSE途切れ・usage limitの人工テストを実施した。refreshの耐障害性とWindows実機はまだ未検証。ユーザー自身のbrowser loginとLuna人工文推論は「テスト成功」の報告を得た。資格情報内容はツールで読んでいない。次は音声品質と運用耐障害性を確認する。
