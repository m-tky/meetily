# 会議入力確認：実マイク一台＋PC再生音

## その後：PC正規化を承認・実装

ユーザーがPCにも上限付き自動正規化を明示選択。新candidateの元音量／−20dB公開fixture＋private仮想入力検証まで成功。mic／VAD／model／resamplerは維持、旧visible画面は未反映、同期復帰bugは別件として未修正。最新状態は [PC-AUDIO-NORMALIZATION.md](PC-AUDIO-NORMALIZATION.md)。以下のsystem raw記述は正規化導入前の記録。

## 現在のテスト方式：公開日本語fixture＋隔離仮想音声

ユーザーは自分のmic録音の再利用をやめ、使用中の実micを避けるvirtual speakerテストを指定。公開FLEURS日本語と、**private仮想環境だけでagent自動Start／再生／Stop／audit**を明示承認した。PC-only／仮想dual（無音mic）でnonempty ASRと保存・DB一致を確認、実mic／speaker／host output設定／visible appには触れない。既知復帰bugは未修正、意味品質の受入ではない。最新結果・制約・再現は [VIRTUAL-PUBLIC-CHECK.md](VIRTUAL-PUBLIC-CHECK.md)。以下のuser7.2秒mic replay／手動onlyは歴史的記録で、今後のこの診断sourceとして使わない。

## 修正版の手動再テストで文字起こし0、timeout後の復帰不具合を再現

ユーザー「音が小さかったせいか、なにも文字起こしされなかった」、追加確認で**Stop／保存完了後も空**と回答。音量原因と決めつけず承認済み最新private recordingをnon-content audit。

- folder count3、14.358604秒／decoded14.358625秒、mono48k／363393bytes、finite/nonzero、decode warning0、completion commit、選択両device、SQLiteとの空segment一致、ASR input／checkpoint cleanup正常。
- raw segments0。auditor passed=falseはnonempty transcript条件の失敗で、保存媒体破損を示すものではない。AAC decode16k RMS0.075689／peak1.040282は混合保存音声の値で、PC単独のgain／元ASR clipの証明ではない。
- native logをローカルで最新Start boundary以降だけ解析（行や本文をtool exportせず）。ASR queued chunks0、VAD segment0、transcription completionあり、VAD／transcription／recording channel failure messages0。**ASRへ渡すspeech区間自体がなかった**。
- input stall warning1、late discard messages37、合計606208samples（48k換算12.629333秒相当）の遅着破棄。これは破棄sample合計でwall-clock欠落時間／PC単独の値ではない。現logにsource identityがないためmicかsystemかは未特定。

追加録音なしでproduction `mixing.rs`を直接compileした人工回帰がFAIL：初回timeout後に古い区間が届いて破棄され、次の区間のmicがsystemより20ms先に到着すると、`degraded=true`が新区間の待機までスキップして即無音補完する。後のsystem callbackも遅着となる。**一度timeoutすると、復帰した入力を待たず破棄し続け得る実装不具合**を再現。以前の回復testは両入力を先にbufferしており、この到着順をカバーしていなかった。

logs：`meetily-dual-sync-empty-save-audit.log`、`meetily-dual-sync-empty-diagnostics.json`、`meetily-dual-sync-recovery-regression.log`。private `empty-transcription-diagnostics.json`にもaggregate保存。実音声／本文／native log linesのexport、追加capture/playback、設定変更なし。

音量不足／PC入力の欠落との因果をまだ単独確定しないが、まずこの同期復帰不具合を修正・人工回帰してからcandidate再反映するのが妥当。**音量を上げるだけの再テストを先に要求しない**。1.2秒有限待機＋警告＋既発行区間の遅着破棄という合意方針は維持し、次の区間まで無期限にdegraded即出力を持ち越さない設計が必要。現在開いたcandidateはこの未修正バグを含む。認識品質合格／releaseなし。

## 同期＋Stop末尾を修正、人工データ回帰を確認（下記復帰不具合を後で発見）

ユーザー「続けて」の後、decision gateで **同期＋Stop末尾を修正** を選択。入力停滞時の方針も明示選択：**最大1.2秒待機後、警告付きで欠けた区間を無音補完して継続、その区間の遅着サンプルは破棄**。共通デバイス時計の導入、resampler／mixer gain・飽和／VAD／model変更は範囲外。

### 実装

- 新 `frontend/src-tauri/src/audio/mixing.rs`：選択された入力ごとに累積サンプル位置を保持し、共通output cursorの600ms区間で混合する。両入力が選択されている場合は両方の区間が揃うまで待機。mic-only／system-onlyは無効側を待たない。
- 1.2秒timeout後はdegraded modeで残る入力を継続。outage開始時にnative warning＋既存`AudioError::StreamFailed`通知。受信が戻っても既に無音で出した区間のサンプルは破棄し、二重挿入・時間伸長を防ぐ。両入力が追いつくと通常待機へ戻る。overflowは既存4.8秒queue上限を維持し、捨てた位置をsilence gapにして時刻をずらさず警告する。
- `pipeline.rs`：受信timeout中も待機期限を検査。通常／Stopの混合音声を同じ保存・VAD経路へ通し、保存timestampはcallback時刻ではなくoutput sample位置から計算。Stopではchannel closeまでqueued source chunksを読み、exactな短いring末尾を処理してからVADをflush。複数Stop markerで先にpartial ringをflushしない。録音receiverが閉じた場合は従来のsilent ignoreではなくerrorを返す。
- `recording_manager.rs`からOption deviceの有無を明示渡し、`audio/mod.rs`へmixing登録。新ringは旧static mut counterを持たない。

**サンプル順の同期であり、device capture timestamp／開始offset／長時間clock driftの精密補償ではない。** timeoutやoverflowで欠けた音声は復元できず、無音化／遅着破棄を警告する。capture resampler内部の未flush残りの検証は今回のring-tail修正とは別。

### 検証

Nix shell／shared targetで `cargo test --locked --offline -p meetily --lib` のfilterを使用（test buildのみ、app binary／候補packageのbuild・stageなし）：

- `audio::mixing::tests::` **11PASS**：到着順・single/dual・disabled・exact500ms tail・不均等末尾・1.2秒有限待機・outage recovery・late interval破棄・overflow位置・irregular chunk exactサンプル一致。
- `audio::pipeline::tests::` **10PASS**（既存policy1＋新9）：実Tokio run／channel close／queued Stop marker／新callbackなしtimeout・late discard／保存receiver failure／timestamp・長さ／exact tail重複なし。実Silero VADを使用。local eSpeak `--stdout`→FFmpeg pipeのみでoffline合成音声を生成し、保存PCM exact一致とASR宛16k segmentのサンプル・開始時刻が直接VAD参照と一致することを確認。**音声再生・capture・Qwen ASR推論はしていない**。このspeech testはshell.nixのespeak-ng／ffmpegとlocal ORTを必要とする。
- 既存 `audio::vad::tests::` **7PASS**。native合計**28PASS**（実機録音／認識品質／保存MP4・SQLite／GUIの新統合確認ではない）。
- 更新 `scripts/verify_dual_audio_code.py`：source-level regression **15PASS**、変更しないDSP診断 **2FAIL**（hard saturation／alias）。旧8pass＋4failのbaselineログは保持。script exit0は期待件数の再現成功であり残るDSPの品質合格ではない。

失敗も保持：初回pipeline fixtureはTokio runtime外でbatch processorを作り5tests panic、test runtimeだけ修正。追加positive-speech試験の数学toneは48k経路でVAD segmentが空となりfixture guardが失敗（未prime／prime双方）。このtoneでpositive deliveryを検証したとは言わず、guardを弱めずoffline eSpeak fixtureへ置き換えて正のsegmentを確認。model／gain／VADをgreenのために変えていない。

logs：`../logs/meetily-dual-sync-native-tests.log`、`meetily-dual-sync-pipeline-tests.log`、`meetily-dual-sync-regressions.log`、`meetily-dual-postfix-source-tests.json`／`.log`。初回失敗は`meetily-dual-sync-pipeline-tests-initial.log`、tone failuresは`meetily-dual-sync-pipeline-tests-synthetic-fixture-failure.log`／`-primed-tone-failure.log`。

再現environment：`nix-shell shell.nix`、`CARGO_TARGET_DIR=../murmur/target`（親research directoryからの絶対pathを使用）、`ORT_DYLIB_PATH=/nix/store/x46rnziynpalr4phm6yh0x46s42xq5kw-onnxruntime-1.27.1/lib/libonnxruntime.so`（このhostのlocal path）、`MEETILY_DISABLE_ANALYTICS=1`、`--test-threads=1`。

このcode/test checkpoint時点ではvisible candidateは旧版だった。通常profile・実保存音声・本文・native recording logs・user Start/Stop／playbackには触れず、release作成なし。今回の単語置換を改善したというclaimはしない。以下のユーザー指示でcandidateを更新した。

### 修正版candidateを反映、同じprivate画面でREADY

ユーザー「修正を反映して。次はどうする？」でlocal debug候補の更新を承認。既存embedded frontendを使い、Tauri custom-protocol `--debug --no-bundle --ci`、beforeBuildCommand無効／updater artifacts無効／endpoints=[]の一回限りoverlay、Cargo locked/offlineでbuild成功。新inputs `../runtime/dual-sync-build-*/inputs`からNix stage成功。release／frontend rebuild／通常profile変更なし。元dev binaryはtrapでatomicに復元しSHA256一致、stage binaryとbuild input一致、旧candidateとは異なることを確認。

新GC-root `../runtime/nixos-dual-sync-candidate` → `/nix/store/v907pl178by5zqbkl357fhjrhngr61fx-meetily-local-candidate`。旧 `nixos-local-candidate` とinputsは保持。model／worker／VAD／gain／resamplerは同じ。build log `meetily-dual-sync-candidate-build.log`、stage log `meetily-dual-sync-candidate-stage.log`、build path pointer `../runtime/dual-sync-build-path`、non-content verificationはbuild directory内 `candidate-verification.json`。

切替前にユーザーが「保存完了を確認して閉じた」と明示回答。旧helperが残っていたため最初のresumeは「This preview is still running」で失敗（`meetily-dual-sync-preview.log`）。`/proc`で当該script／private profileを確認し、そのowned helperだけSIGTERMでcleanup。Start／Stopコマンドは呼ばずnormal appは対象にしていない。その後同じ `dual-preview-spocggm1`、DISPLAY:0、port4453で新candidateをresume成功（`meetily-dual-sync-preview-resume.log`）。

`preview_meeting_inputs.py`にembedded origin assertionとstate内binary pathを追加。READY前の`is_recording=false`、`tauri://localhost`、device選択を確認。新storeのnative process実在、同じprivate profile、automatic_recording=falseを `sync-preview-verification.json`へ保存。**修正版画面は開いたが、修正後の実録音／認識品質／GUI保存MP4・SQLiteの新回帰はまだ未実施**。音声／本文／native recording logのtool exportなし。

次の最小確認：両入力選択は維持し、音量を開始前に固定。ユーザーStart→既存 `play-pc-test.sh`で同じ承認済み短音声をPC再生→追加mic発話なし→ユーザーStop／保存完了。PC側の意味認識が改善したか、末尾欠落・重複がないかをユーザー報告＋non-content保存監査で比較。新たな録音・再生をagentが自動開始しない。改善判定後にmic発話を含むdual確認へ進む。

## 最新の合意（初期方式選択）

ユーザーは「マイク一つで、スピーカー／マイクの両方を聞き、会議音声をちゃんと認識したい」と依頼。方式を確認し、**マイク一台＋PC再生音の同時取り込み（推奨）**を明示選択した。物理マイクでスピーカー音を拾い直すだけの方式ではない。マイクは自分の声、PC output monitorは相手の声を取り込む。日本語／local Qwenは維持し、releaseは作らない。

入力：EchoCancel_Mic＋EchoCancel_Speaker.monitor。選択出力へ流れる他アプリ音／通知も録音されることを選択時に説明済み。これはStartの自動操作、実会議・他者の会話の自動録音、cloud/authの許可ではない。まずユーザー操作の限定テストで両方の認識・欠落・重複を確認する。

## 最新：両方は動作したとの報告、認識精度は不合格

ユーザー「精度は悪いができた」。症状を明示選択：**単語や文章を別の内容に誤認識する**。入力経路／保存動作と意味認識のacceptanceを分離し、meeting品質を合格扱いしない。

local read-only `audit_meeting_inputs_save.py`：15.6秒mono48k、383239bytes、finite/nonzero、decode warning0、completion commit、mic/system両device保存、raw1segment非空、SQLite本文一致、temporary ASR input／checkpoint cleanup正常。event/drainedや個別source energyの新実測ではない。

初回strict timestamp比較は14.350000000000001 vs14.35というJS/Rust/SQLite binary64 roundtrip差でfalse。本文はexact一致。auditorを**本文exact＋audio times絶対1ns tolerance**へ変更しpassed=true。この修正は計測比較だけで、transcript補正／音声処理変更ではない。logs `meetily-dual-save-audit.log`／`meetily-dual-save-audit-final.log`。

non-content診断：native logからenumのみ抽出した最後の言語設定はja（本文はtoolへ出さない）。codeのja→Japanese変換・worker.force_language契約・mono16k IEEEfloat32送信を確認。RNNoise flag=false。本体micは80Hz HPF＋EBU R128 normalization、systemはraw、両者をmono mixして一つのQwen認識へ送る。Mixerの「soft scaling」というcommentに反しsum/abs(sum)は各sampleのhard saturationと等価だが、今回それが誤認識の原因とはまだ断定できない。

AAC decoded 48k748800samples：abs>1は2samples（2.67094e-6）、abs>=.99は4samples（5.34188e-6）。AAC codec overshootでも生じ、original ASR inputのclip／歪みを証明しない。保存AACは元float PCM／worker inputそのものではない。無根拠なgain/VAD/model修正はまだ実施しない。log `meetily-dual-recognition-diagnostics.log`／profile `recognition-diagnostics.json`。raw audio／発話textは未export。

ユーザーが語誤りのsourceを**PCから再生した声**と明示選択。さらに「音量が大きく、PC音声の途中で半分くらいにしたせいかもしれない」と報告。重要な交絡要因だが、physical speaker／downstream sinkの音量だけを変えた場合はmonitor入力が同じこともあり、音量変化を誤認識原因と断定しない。metadata確認時EchoCancel_Speakerはfloat32le stereo48k、mute=false、左右100%。変更した箇所と録音中のmonitor gainの時系列は不明。

大きなFFT比較やmodel変更の前に、聞きやすい音量を録音**前**に決め、録音中固定の短いPC再生テストを再実施する。既存dual windowでStart→prepared play-pc-test.shで同じ7.2秒の承認済みテスト録音を再生→今回はmicへ追加発話せず静かに待つ→Stop／保存待ち。音量はuser自身が操作し、agentは増幅／VAD／normalizer／モデルを変更しない。ユーザーによるPC側認識の再評価とlocal non-content保存auditで判定する。品質成功はまだ未確認。音が大きければ無理に続けずStopを優先する。model/capture architecture変更は別の明示decisionが必要、releaseなし。

## 固定音量の再テスト結果

ユーザーが再テスト完了後、精度は前回と「ほぼ変わらない」と明示回答。音量固定による改善は確認できず、途中の音量変更だけを原因と断定しない。

承認済みprivate profileの最新保存をread-only non-content audit：folder count2、24.6秒mono48k、636379bytes、finite/nonzero、decode warning0、raw1segment非空、選択mic/system device保存、completion commit、SQLite本文exact／時刻1ns以内一致、ASR temporary input／checkpoint cleanup正常、passed=true。log `../logs/meetily-dual-fixed-volume-save-audit.log`。これは保存健全性の確認であり、各sourceの独立capture／ASR入力無歪み／認識精度の証明ではない。AAC decoded16k peak1.004は元入力clipの証拠としない。

初回コマンドはhost PATHにpython3がなく失敗。`nix-shell -p python3 ffmpeg`で監査を実行して成功。録音・再生の自動開始、本文／音声／native logのexport、通常profile／gain／VAD／model変更、release作成なし。次の候補は同じ承認済み短音声のPC-only入力テストでmic混合／AEC経路を切り分けることだが、入力方式を変更する前にユーザー選択を得る。

## 追加録音なしのコード経路調査

ユーザーは次工程として「追加録音せず、コード経路を調べる」を選択。PC-only切り替え／新規録音・再生は行わない。本体コードの変更なし。

現行Linux経路：`capture/pulse.rs:242-319`で選択monitorを明示指定、Pulse側にfloat32LE／mono48kを要求（stereo→monoはPulseが処理）。`stream.rs:92-105`→`AudioCapture`へmono48kとして渡すため、PC側ではcaptureの追加resampler／mic HPF・normalizerを通らない。`pipeline.rs:842-902`でmicとmixし、同じmixed48kを保存・VADへ送る。VADの48k→16k後、segmentを`worker.rs:436-450,554-559`→Qwenへ渡す。この経路ではworkerの追加resampleを通らない。`qwen_provider.rs:124-163,247-350`はja→Japanese、IEEE float32 mono16k、無gain・無PCM16化。30秒超は連続windowに分割（今回24.6秒全録音ではこの上限を超えない）。

コード上の具体的な懸念（今回の誤認識原因と断定しない）：

1. **48k→16kは5点移動平均＋間引き**（`vad.rs:119-164`）。Sinc resamplerではなく、48kではfilter_size=2／内部5点、3倍整数比のため補間fractionは0。内部定常周波数応答の解析で4kHz −2.54dB、6kHz −6.324dB、12kHz −13.979dB。12kHzの残留成分は16k出力の4kHzへaliasする。高域の減衰とstopband不足はコードから分かるが、実テストの音声成分／認識への影響は未測定。
2. **二入力mixが同じ時刻を保証しない**（`pipeline.rs:40-49,100-152`）。実windowはcommentの50msではなく600ms、max bufferは4800ms。片方だけwindow分あれば即mixし、他方をzero-padする。timestampをmix整列に使わない。人工の到着順モデル「mic600ms→system600ms」で各600ms入力から1200ms出力になる条件を確認。実callback scheduleを測ったものではなく、常に二倍になる主張ではない。
3. **Stopでring residualをflushしない**（`pipeline.rs:927-966`）。VADはflushするが、未mixの両source各600ms未満のring残りは処理しない。末尾欠落のコード上の懸念（今回欠落した量は不明）。
4. **mixのcommentと実装が不一致**（`pipeline.rs:167-198`）。system70%ではなく100%、mic80%計算値は未使用。「soft scaling」はsum/abs(sum)で±1へのhard saturationと同じ。synthetic mic0.6＋system0.6→1.0。元ASR入力のclip有無はAACから確定できない。

`../logs/meetily-dual-code-path-analysis.json`は標準ライブラリによる周波数応答解析／ring到着順モデルのnon-content数値。**production Rustを実行した回帰テスト／実capture測定ではない**。保存監査正常でも、保存とASRで共通するmixの異常やASR側resampleを排除できない。次に進めるならproduction処理を人工データで直接回帰検証するのが妥当。resampler／mix／tailの修正、gain/VAD/model/architecture変更は別途ユーザーdecisionを得る。releaseなし。

## 人工データでproduction Rust snippetsを直接検証

ユーザー「検証して」で人工データ検証を承認。新しい `scripts/verify_dual_audio_code.py` は現行ファイルからring＋mixer、`resample_to_16k`、`flush_remaining_audio`を**変更せず抽出**し、`rustc --test`でコンパイルする。外部log macrosはstub、Stop-flushのVAD／sender依存はmock（senderはstd mpsc）、同じring／resample／flush本体を直接実行。本体app／GUI／実callback／Silero推論の統合試験ではない。テストはsingle-threadでring既存static mut counterの並列実行を避ける。抽出snippet SHA256をlogへ保存。

当時の実行（scriptは上記の修正後の検証へ更新済み、baseline結果はログ保持）：

```sh
nix-shell -p python3 rustc --run 'python3 scripts/verify_dual_audio_code.py'
```

当時の結果：**現行挙動確認8tests PASS、診断目標4tests FAIL**を再現。script exit0は「この8pass＋4failを確認できた」の意味で、DSP品質／製品合格ではない。exit codeだけでなくpass/fail件数も検査する。

- ring window28800／max230400samples（600／4800ms）。両方を先にbufferしたcontrolは600msで同時mixできるが、mic600msをmix後にsystem600msが届く人工scheduleでは1200ms出力になる。実runtimeでこのscheduleが起きたかは未測定。
- ringにsystem500ms＝24000samplesを残して本体Stop-flushを実行しても残りは24000のまま、VAD mock呼出1回・送信0。既存VAD segment800samplesがあるcontrolは正しく送信するがring tailは残る。StopのVAD送信動作自体の失敗と区別する。
- system0.7＋mic0→0.7、mic±0.6＋system±0.6→±1.0でunity gain／hard saturationを再現。
- 実resample関数へ600ms人工toneを通した結果：6kHz −6.324dB、12kHz→4kHz alias −13.979dB。DC0.25保持／28800→9600samplesのcontrolは成功。alias40dB以上抑制という診断目標は失敗（この40dBは今回の合意済みASR品質基準ではない）。

初回harnessのalias波形期待値に符号ミスがあり、挙動確認7pass1failになった。12k→4k折返しと5点filterの符号の積を修正した**テスト期待値だけの修正**で最終8pass0failへ。初回失敗logも保存し、本体DSPを変えてgreenにしていない。

logs：`../logs/meetily-dual-production-snippet-tests.json`（最終）、`../logs/meetily-dual-production-snippet-tests-initial.json`（初回）。人工データのみ／capture・playback・ASR・profile access・native log accessなし。**4点はproduction処理の条件付き挙動として再現したが、ユーザーの単語置換原因は未確定**。本体audio処理・input選択・normal profile・VAD/model/gain・debug candidate・releaseは変更しない。修正する場合は同期＋tail保存を優先候補とし、処理方式選択は別途decision gateを通す。

## 初期準備（確認前の記録）

`scripts/preview_meeting_inputs.py` を追加。既存WAV-fix済NixOS debug candidateを使用し、release／debug rebuild／package stageは追加していない。既存ASR defaults、model、VAD、gain、OS AEC設定は変更しない。通常のuser app profileにも書き込まない。

- fresh HOME/XDG/private SQLite／private D-Bus。既存mic-only profileとsynthetic previewは保持。
- CPAL→private ALSA Pulse alias `meetily_mic` をEchoCancel_Micへexplicit bind。
- 本体native Pulse monitor経路のserverをhost Unix socketへ接続し、preferred system deviceを **pulse-monitor:EchoCancel_Speaker.monitor (output)** に指定。
- local Qwen1.7B decoder INT8＋ja。人工transcript/minutes、自動Start/Stop、helperによる自動再生なし。
- 選択する二つのsourceが存在しmute=falseをmetadataだけで確認。選択output monitorが本体device enumerationにあることを確認、READY時is_recording=false。

window title：**Meetily — 会議入力確認 / マイク＋PC音声 / 日本語**、DISPLAY=:0、port4453。

profile：`../runtime/dual-preview-spocggm1`、helper log：`../logs/meetily-dual-preview.log`、routing/state JSONはprofile内。現在の本体既存mix/capture経路を設定した段階であり、全会議認識の品質合格・二重取り込み解消の証明・通常profileの恒久的default変更とはしない。

## 簡単な手動テスト

1. 上記windowでStart recording。不要な音を出すアプリを止める。
2. 次のlocal commandを実行し、先ほどユーザーが録音した**7.2秒の短いmicテスト**だけをPC側から再生する：
   `../runtime/dual-preview-spocggm1/play-pc-test.sh`
3. 再生が終わってから、マイクに別の短い日本語文（例「こちらはマイクからの発言です」）を話す。
4. Stopし保存完了を待つ。PCからの発話と現在のmic発話の両方が認識され、PC発話が二重に出ないかをユーザーが確認する。

`scripts/prepare_pc_test_playback.py` はこのcommandを**作成するだけ**で実行しない。sourceは承認済みmic profileの一つのaudio.mp4のみ、30秒以下をguard。native ffmpeg→float32 PCM pipe→paplay、destination EchoCancel_Speakerを明示。音声や発話textをtool出力・cloud contextへ送らない。読み上げ再生はユーザー自身のcommand実行時だけで、周囲へ音が聞こえる点に留意。第三者／実会議録音を自動再生しない。

今回のauditも実音声・発話text・native log全文をtoolで読むのでなく、local computed非内容metrics／一致boolとユーザー報告で行う。配置・device metadataだけでは両方の実認識を証明できないので、テスト前にsuccessを記録しない。

## 重要な既存制約

- 本体`start_audio_level_monitoring`は`audio::simple_level_monitor::start_monitoring`へ接続され、現状sin波の**fake level data**を生成する（lib.rs、simple_level_monitor.rs）。UI meterの動きを実入力／両方captureの証拠として使わない。この準備ではmonitoring commandを起動しない。今後実meterに変える場合はidle時のmicrophone capture範囲も考慮する。
- EchoCancel source/sinkがあることとAECが十分効いていることは同じではない。重複／実入力clipを確認せず除去済みとは言わない。
- mono mix→一つのQwen経路。話者識別／完全な同時発話分離／精度保証を追加したわけではない。
- 元のmic-only auditは7.2秒・raw1segmentのstorage確認済み（MIC-CHECK.md）。dual profileは別で、その成功を流用しない。

## 再起動

`preview_meeting_inputs.py` はmic previewと同じargumentsに `--system-monitor EchoCancel_Speaker.monitor` を追加し、port4453を使う。Stop/保存/close後は `--profile ../runtime/dual-preview-spocggm1` で再開できる。ユーザーがStartするまでcaptureは始めない。release作成は引き続き禁止。
