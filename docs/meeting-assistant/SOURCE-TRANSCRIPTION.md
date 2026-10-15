# 入力別文字起こしのみ：Murmur方針のMeetily実装

## 最新のユーザー判断

「文字起こしだけ」「話者分離はなし」「murmurの方針で実装して」を受け、**Meetilyのlive経路を入力別VAD／ASRへ変更**した。人物の声による識別・クラスタリング・ForcedAlignerは製品経路へ入れない。過去の混合1系統diar／alignment調査は履歴として保存するが、採用計画ではない。

```text
マイク ─┐                      ┌ マイク専用VAD ─┐
        ├ 共通sample時計の窓 ───┤                ├ 共有ASRを直列呼出し → 本文＋取得元
PC音声 ─┘                      └ PC専用VAD ─────┘
        └ 既存mixer → 従来の混合録音／WAV checkpoint／最終AAC保存
```

これはMurmurの **取得元を保った別々の文字起こし** を採用する変更。Murmurの保存形式・DSP・手動diarizationまで移植したものではない。

## 実装

- `audio/pipeline.rs`：選択された入力ごとに独立した `ContinuousVadProcessor`。両入力が有効なら2状態、片方だけなら1状態。VAD設定・capture gain／normalizer・ASR設定／model／languageは変更しない。
- 既存の120ms共通windowから、mix前のmic／system PCMを各VADへ渡す。無音／欠落paddingも各選択入力へ同じ長さで供給し、録音相対sample時計を維持。到着時刻をASR時刻と推定し直さない。
- 発話は元のDeviceTypeのままASR queueへ送る。mixをASRへ渡したり、PC発話をmicへ付け替えたりしない。人物の話者境界による切出し／文字時刻推定なし。
- ASRは既存worker1つで直列処理し、modelを共有。2つのASR modelを常駐させない。各providerの既存処理・Qwenの30秒bounded request／unconfirmed policyはそのまま。
- Stopは共有ringの短い末尾を正確にdrainしてから両VADをflush。ASR receiver失敗は報告し、録音末尾の保存は続ける。未知入力・人物を推測して補完しない。
- 保存音声のmixer／formatは従来のまま。入力別WAVの永続保存／stereo録音への変更は行わない。

## 取得元metadata・表示・保存

`mic`／`system` は **マイク／PCという入力元**。1入力に複数人がいても分離しない。「自分」「相手1」などの人間の同定には使わない。

- live `TranscriptUpdate` に `speaker` を追加し、既存の `source` にも同じ取得元を入れる。
- canonical `transcripts.json`／履歴snapshot／SQLite／再読込／pagination／IndexedDB復旧／録音orphan復旧へ任意 `speaker` を引き継ぐ。
- SQLは既存migration `20251110000001_add_speaker_field.sql` のnullable columnを再利用。新schema migrationや依存変更なし。
- live／保存後の表示とcopyは **Mic／PC** のbadge／prefixのみ。未知文字列・legacy mixed・imported mono audioはbadgeなし。旧JSONはspeaker省略をNoneとして読み、過去の本文や取得元を勝手に書き換えない。
- 人物識別をしなくても、2入力が同時に同じ文を発話することはある。frontendのduplicate判定はsequence ID、旧データfallbackも取得元を区別し、同じ本文・wall-clockだけで片方を消さない。sequence0も有効。
- 独立VADでは長い発話が後から完了する。native sink／frontendは音声start＋sequenceで順序を整え、本文／根拠IDを変更しない。SQLite paginationは同時刻にrowid tie-breakを付ける。
- 既存議事録へ渡すspeaker metadataも取得元のみ。既存promptはmic/systemがverified human identityではないと明示済み。議事録機能を有効化したり、cloudを呼んだりはしない。

## 検証済み範囲

**Rust55 PASS／frontend8 PASS、実Ollama test1は既存ignoreのまま。**

| filter | PASS |
|---|---:|
| `audio::pipeline`（新入力別3を含む） | 20 |
| `audio::transcription::worker::source_metadata_tests` | 1 |
| `audio::recording_saver::transcript_sink_tests` | 7 |
| `audio::recording_recovery` | 8 |
| `database::repositories::transcript::source_tests` | 2 |
| `live_minutes` | 17（別に1ignore） |
| frontend source／順序／duplicate／badge SSR | 8 |

- eSpeak stdout→FFmpegの **非再生synthetic音声** 2入力を同時配置。実Silero VADを2状態で通し、各ASR-bound PCM／開始時刻が独立referenceと全一致。少なくとも各入力1発話を要求して空成功を防ぐ。
- 同fixtureの混合録音が従来mixer referenceとsample全一致、短いStop末尾保持・2回flushの非重複を確認。
- disabled入力にVADを生成しない。ASR channel故障でも録音末尾を保持／errorを報告。
- same text／same audio timestampのMic・PC行が共存し、real migrationを適用した隔離SQLiteで保存→close→reopen→paginationして両タグを保持。legacyはNoneのまま。
- source JSON roundtrip／遅延結果の時間順／orphan復旧→SQLite reopenを確認。
- React badge SSRはMic／PCのみで人物名なし、未知／旧データは空。TS typecheck、locked/offline native example check、`git diff --check` 成功。
- 初回compileは異なる2種類のDeviceType enumを取り違えて失敗。recording-state enumへ修正して成功。初回logも保持。

logs：`../logs/meetily-source-transcription-native-r1.log`、`native-r2.log`、`native-final.log`、`frontend-r1.log`、`frontend-final.log`（後4つも同じ `meetily-source-transcription-` prefix）。

```sh
nix-shell shell.nix
export CARGO_TARGET_DIR=/home/user/Code/meeting-assistant-research/murmur/target
# このhostのlocal ORT：既存のnative VAD testと同条件
export ORT_DYLIB_PATH=/nix/store/x46rnziynpalr4phm6yh0x46s42xq5kw-onnxruntime-1.27.1/lib/libonnxruntime.so
export MEETILY_DISABLE_ANALYTICS=1
for filter in audio::pipeline audio::transcription::worker::source_metadata_tests \
  audio::recording_saver::transcript_sink_tests audio::recording_recovery \
  database::repositories::transcript::source_tests live_minutes; do
  cargo test --locked --offline -p meetily --lib "$filter" -- --test-threads=1
done
cargo check --locked --offline -p meetily --examples
cd frontend
node --test tests/lib/transcript-source.test.mjs
node_modules/.bin/tsc --noEmit
```

## ユーザー操作の試用候補を開いた（次の承認）

ユーザー「試してみたい」→ `マイク＋PC音声` を明示選択。日本語／local Qwen／以前のEchoCancel入力を、**新規隔離profile**で試すdebug候補をbuild・stageした。

- frontendは必要なsource／public configのみ `../runtime/source-transcription-build-UV4UAY/frontend` へcopyしてproduction export。`.env`／元`.next`／元`frontend/out`を使わず、元のbuild artifactsを変更しない。新Mic／PC UIをembeddedしたTauri custom-protocol **debug**／no-bundle／locked offline build。updater endpoints=[]、署名・release・portable installerなし。
- GC-root `../runtime/nixos-source-transcription-candidate` → `/nix/store/wnimgmc798andrlssvrymyp80n981fr0-meetily-local-candidate`。native SHA256 `7a88dbe2375ec3efbe80bd6bf5fb3f61bee16ceda705800eaf509eab4120508d` はinputとstageで一致。workerは旧候補とbyte一致、ASR model／ORT／helper／templatesは既存を使用。
- 共有Cargo targetの元dev binaryは退避し、build後atomic復元、SHA256一致。旧visible候補／旧profile／通常profile／他のprocessは止めない。
- 新private profile `../runtime/dual-preview-4p61rc4k`、DISPLAY:0、WebDriver4454。タイトル **Meetily — 入力別文字起こし / Mic・PC / 日本語**。旧profile／4453の旧画面とは別。
- READY前に `is_recording=false`、embedded origin `tauri://localhost`、両選択入力の列挙をassert。新store native processも実在を確認。READY snapshotは空の録音画面／正しいMic・PC選択を表示、`automatic_recording=false`。Mic／PC本文badgeはユーザーが録音した後の確認事項であり、実音声の認識品質・保存成功は未承認。
- helperに任意 `--window-title` を追加、既存defaultは維持しpy_compile成功。最初の起動はWebDriver pathのtypoでprofile作成前に失敗し、訂正した起動でREADY。失敗logも保持。
- build script：`../runtime/build-source-transcription-candidate.sh`、build pointer：`../runtime/source-transcription-build-path`。logs：`../logs/meetily-source-transcription-candidate-build.log`、`meetily-source-transcription-preview-initial.log`／`meetily-source-transcription-preview.log`。
- Start／Stop／物理再生はtoolで呼んでいない。試用中のreal本文／録音／native recording logは読まない。実操作はユーザーが行い、短いmic→PC各別発話で取得元・停止後保存を確認する。

## ユーザー報告：入力別ASRの試用成功、Lunaの接続準備へ

ユーザー「ok, できるようになった。画面は閉じた」と報告。これは **ユーザーによる試用成功報告** として記録し、toolで実音声・本文・保存内容や精度を検証したものとは区別する。

続けてLunaを使いたいとの依頼。方式確認で「Meetilyの議事録用にLunaを使う」を選択し、既存のアプリ専用公式ChatGPTログイン・まず人工文テスト、実録音／実本文はまだ送らない方針を確認。Codex CLI／app-server接続への変更ではない。閉じた画面のhelperが残っていたため、**保存済み・今回の試験用アプリ再起動可**の明示回答を得て、当該helperだけ終了→同じ `dual-preview-4p61rc4k`／4454で再開。

- タイトル **Meetily — Luna接続準備 / Mic・PC / 日本語**。同じcandidate、既存保存dataを保持し、通常profile／旧4453には触れない。
- helperはローカルWebDriver session IDをprivate stateへ保存して制御を再取得可能にし、`--no-ready-screenshot` を追加。実本文があり得るprofile再開で新しいscreenshotは作らない。IDや資格情報内容をログへ出さない。
- 準備toolは未録音／embedded originを確認し、議事録欄を開いてproviderを `chatgpt_plan` に選択、Continue with ChatGPTボタンの存在を確認。**ログイン／人工文テスト／継続更新／最終草案のボタンは押さない**。実本文・音声・native録音logを読まず、推論／録音／再生なし。
- 初回のUI準備はselector引用符のミスで失敗、次はDOM changeがReact stateへ反映されず失敗。ローカルReact onChangeでprovider選択するよう修正し成功。失敗log保持。controller準備だけで本体auth／providerコード変更やcandidate再buildはない。
- `../runtime/prepare-luna-login-ui.py`、`../logs/meetily-luna-login-preparation{,-initial,-r2}.log`、`../logs/meetily-source-transcription-luna-preview.log`、private `luna-login-preparation.json`。helper py_compile／diff check成功。
- **現在のprofileでのログインと人工文の実推論はユーザー操作待ち**。過去の別profileのLuna成功報告はあるが、今回の接続成功の代わりとはしない。モデル利用可否は当該accountの一覧と推論完了で確認する。公式方式の詳細は [ADR-002-OFFICIAL-SIWC.md](ADR-002-OFFICIAL-SIWC.md)。

## 次のユーザー報告・UI方針更新

Lunaの接続・人工文テストもユーザー「できた」と報告。その後、認証・モデル選択をSettingsへ移動、UIを英語化、補足説明の削除を依頼。実装・32 frontend test・新debug候補反映と既存ログイン／Luna／設定永続化のmetadata確認まで成功。現在の操作場所は **Settings → Summary → Meeting minutes**。以前の録音画面内ログイン／model selectorは削除した。詳細：[SETTINGS-UI.md](SETTINGS-UI.md)。

## 未検証・変更していないこと

- 実マイク／desktop capture／物理再生／実会議／cloud推論／認証操作／旧ユーザー録音の読込・改変なし。
- このcheckpointのsynthetic pipeline testは **実VAD＋ASR入力まで**。新経路での実ASR認識品質／GUI全体／long-duration／同時両入力の実時間追従を合格とはしない。
- 同時に両入力が話し続けるとASR呼出し数が増える。diar／alignmentを外したから全体が必ず速くなるとは限らない。既存unbounded ASR queue／長い発話のlive latencyの限界は残る。
- マイクがPC再生を拾うechoは、同じ内容が2入力に残り得る。人物判定／cross-input文字列dedupで黙って消さない。新AEC／Murmurのecho suppressor／追加gain変更なし。
- 初回実装checkpointではcandidate反映をしていなかった。その後、上記の明示承認で新frontendを含む別debug候補を開いた。旧画面／通常profileは保持。releaseなし、ユーザー操作の実認識・保存・再起動確認はこれから。
