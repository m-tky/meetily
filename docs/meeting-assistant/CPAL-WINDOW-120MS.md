# CPAL同期の追加診断／120ms処理窓

## 決定・状態

ユーザーは未完一覧の「3：従来CPALの同期」「5：処理単位短縮」の実行を依頼。decision gateで**120ms**、続く有効backend時刻の不連続について**新しい連続区間として復帰し、gap／重複を保持して警告**を選択した。

**5は実装・隔離回帰成功。3は部分修正のみで、同期受入FAIL。** CPALを本体の時刻同期mixerへ有効化しない。表示中の5wh候補／選択機器／既存録音は変更していない。実mic／物理speaker／cloud／releaseなし。

## 120ms実装

- live managerがsample-order／capture-time双方のmixerへ120msを明示指定。48kHzでは5760samples、従来600msは28800samples。比較用constructorの600msと既存回帰を残した。
- 1.2秒wait／episode warning／late discard／exact Stopは維持。buffer容量は**4.8秒のまま**で、window比に連動して960msへ縮めない（waitより小さくするとoverflowを作る）。
- VAD設定・発話終了500ms・ASR/model/provider・mic/PC gain設定・filter係数・resampler方式は変更なし。処理頻度は5倍になるが、CPU/RSSの長時間性能測定ではない。ASR結果が120msごとに出る保証でもない。
- synthetic speechのsaved PCM exact／Stop tail／real VADとの同じwindow単位での一致を600msと120ms双方で確認。VADの既存chunk-local resamplingにより、**600ms版と120ms版のASR入力・認識がbit-identicalという主張はしない**。既知saturation／alias failureは維持。
- private probeに`--window-ms 600|120`、GUI harnessに`--expect-window-ms 120`を追加。設定ログ120msに加え、numeric transportの通常mix blockが全5760samplesであることを要求する。

## CPAL：接続不足を修正したが、時刻の信頼性は未合格

前工程のlive CPAL callbackは`InputCallbackInfo`を捨て、`process_audio_data`へ渡していた。したがって以前のtimed probeのunknown625/653/617は、**CPAL backend自体が時刻を返せない証明ではない**。今回F32/I16/I32/I8のLinux callbackを時刻用入口へ接続して再診断した。

実CPAL0.15.3 ALSA実装はstatus timestamp−status delayをcapture時刻とし、status取得後にreadiしてcallbackへ渡す。private ALSA Pulse aliasの既定は44100Hz/stereoで、本体のpersistent resamplerを通る。接続後の試験でもbackend時刻とsample数の連続時計が起動中に約450msずれ、共有人工信号の位置一致は未合格。固定offsetやarrival補正は採用しない。

追加修正：

- `CaptureClock`はvalid snapshotが連続予測から100ms超離れた場合、valid時刻で新しいrunを開始。missing／NaN等はunknownのまま。gap／overlapはmixerで保持・警告する。これはnative Pulseにも共通の復帰契約。
- resamplerの古い／unknown prefixが消費された後、残りbufferの時刻を新しいvalid入力範囲から再計算。一度の不連続でunknownが永続伝播する問題を修正。境界を跨ぐoutput自体はunknownのままで、推測して配置しない。
- live CPALは従来sample-order経路を保持。時刻入口は`MEETILY_PRIVATE_CPAL_TIMING_PROBE=1`をprivate probeが指定したときだけ計測経路を使用する。通常OFF、保存prefs/defaultやnative選択への自動変更なし。

### 失敗結果を保存

| private shared CPAL+native PC／20秒 | invalid chunks | gap samples | stall | 全burst保持 | 判定 |
|---|---:|---:|---:|---|---|
| callback接続、600ms | 637 | 0 | 1 | 失敗 | FAIL |
| 不連続復帰、120ms（resampler復帰前） | 393 | 563494 | 1 | 失敗 | FAIL |
| PC-first startup試験 | 662 | 0 | 1 | 失敗 | FAIL |
| resampler復帰修正、通常startup／120ms | 3 | 24908 | 0 | 成功10/10 | **FAIL** |

最終試験のPC−mic位置差は最初約300ms、その後約30ms。shared-signal条件≤20ms／変化≤5msもFAIL。全burst保持だけで合格にしない。PC-firstは改善せず、product／最終probeの起動順を変更していない。前後のbackend読み出し時刻対応・ALSA/Pulse delayの妥当性・resampling境界が追加調査対象で、gapがすべて真の物理欠落だったとも断定しない。

ログprefix `../logs/meetily-cpal-120ms-`：`callback-first`、`discontinuity-first`、`start-order-first`、`resampler-recovery`および各build。unit-command180秒timeoutはunit自体48PASS後のexample build途中だった。失敗を消さず、standalone example buildを再実行した。

## 120msの最終検証

### native transport60秒

ONE private人工再生→同じnull sink monitorをnative mic remap＋PCで取得。両入力30/30bursts保持、late/gap/overlap/invalid/overflow/stall全0、位置差約−10msで観測増加なし（解析10ms解像度）。通常mixerは120ms。これはhardware同期／実機／multi-hourの保証ではない。

### 本体GUI＋実local-Qwen

公開FLEURS11.1秒−20dB＋private zero mic、DOM機器選択・prefs保存→browser reload→録音・Stop→native再起動。全runsで設定120ms／通常5760samplesを確認。

| route | ASR segments | 保存秒 | stall／late／error events | 全体 |
|---|---:|---:|---:|---|
| PC-only（capture-time） | 2 | 13.857875 | 全0 | PASS |
| native mic＋PC（capture-time） | 2 | 13.4471875 | 全0 | PASS |
| 従来CPAL mic＋PC（sample-order） | 2 | 11.7695625 | 全0 | PASS |

全3runsでASR非空、event/raw/SQLite text一致、completion、decode warning0、UI保存／reload／native reopen、保存files不変、temporary／checkpoint cleanup、owned helpers終了。最後のCPAL runは**同期時刻の合格ではなく、旧経路を壊していない確認**。zero micであり、両者に実発話がある品質・AEC・重複検証ではない。

unit72PASS：clock5／timed10／旧mix16／pipeline17／Pulse5／norm10／VAD7／state2。Python metrics4PASS、TS成功。source harness20PASS＋既知DSP2expectedFAIL。最初のharness判定は旧expected18のためexit1（実際20PASS）を保存し、追加2tests分だけexpectedを20へ更新、再実行成功。失敗DSP期待や合格条件は緩めない。

最終ログ `meetily-window-120ms-{native-shared-60s,public-single,public-native-dual,public-legacy-dual,final-regressions,final-source-ts}.log`。

## 候補・未完

新embedded debug/no-bundle候補 `../runtime/nixos-window-120ms-candidate` → `/nix/store/r993k53xyzmz47ffc577jhqri61xxxvi-meetily-local-candidate`。build input pathは`../runtime/window-120ms-build-path`へ記録。built/staged binary一致、dev binaryのatomic復元／保存コピーとの一致を再確認。既存隔離frontend exportを使用し、通常`.next`／`out`／dev server／表示中appは不変。build/stage log `meetily-window-120ms-candidate-build-stage.log`。

**3の残り**：CPAL/ALSAのbackend時刻対応、起動差、44100Hz resampler境界をさらに測り、共有信号一致・不連続警告／unknownのない通常runを検証する。本体timedへの接続は合格するまで保留。native入力を勝手に代替選択する解決策は使わない。

表示反映には保存完了・window閉鎖確認が必要。実二入力／数時間drift／pause-buffer境界／server再起動／CPU/RSS受入／精度／releaseは未検証または禁止のまま。
