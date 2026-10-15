# 二入力遅延：native Pulse入力と収録時刻配置の隔離検証

**後続の最新状態**：ユーザーが未完3（CPAL同期）・5（window短縮）を依頼し、120ms／有効時刻不連続の保持・復帰を選択。120msはnative60秒＋GUI PC-only/native-dual/旧CPAL-dual成功。CPAL時刻は部分修正後も共有信号位置差300→30ms／invalid3／gap24908でFAIL、本体timed有効化は保留。以前のunknown625/653/617はcallbackが時刻を捨てていた経路であり、backend時刻非対応の証明ではない。新候補・失敗・検証詳細は[CPAL-WINDOW-120MS.md](CPAL-WINDOW-120MS.md)。以下は600ms時点の記録。

## 最新：起動差を時刻で保持、native二入力の60秒／GUI回帰成功

継続の修正依頼に対し、Linuxに**明示選択式** `pulse-input:<source>` を追加。既存CPALマイク／既定機器は残し、保存済選択を自動変更しない。native micとPC monitorが選ばれた場合（PC-onlyも含む）だけcapture-time mixerを有効にする。表示中previewは変更せず、実mic／物理再生／cloud／releaseなし。

### 時刻契約・処理

- `AudioChunk.capture_timestamp: Option<f64>` はセッション開始から、pause時間を除いた**先頭入力frameの推定収録時刻**。既存 `timestamp` はhandoff／旧経路用として分離。
- Pulseはdrop前のread-head latencyをfresh timing updateで取得し、signed latencyを共通 `Instant` へ写す。取得失敗・100ms超のstale値はunknown、arrival／sample-zeroで埋めない。hardware timestamp／完全な時計同期の保証ではない。
- `CaptureClock` は最初の有効backend時刻をanchorとし、そのrunのsample数で連続時刻を進める。次のsnapshotとの差が100msを超えればunknown。pause/resume generationで再anchor。固定source offset補正なし、長時間drift補正は未実装。
- `TimedMixer` は48k sample位置へ配置。起動までの未収録範囲はsilenceとして保持し、最初の入力を双方0へ移動しない。隣接推定の5ms以内jitterだけsnap。真のgap／overlap／invalidをwarning・errorへ、既出区間のlateはdiscard。600ms窓／不足区間ごとの1.2秒wait／WARN＋silence／exact Stop tailは維持。
- native micはnon-monitor sourceをexact IDで列挙・検証、DONT_MOVE＋ADJUST_LATENCY／mono48k float32／100ms fragment。共有sourceへのlatency要求の影響を否定しない。micのHPF・normalizer、PC norm、VAD／ASR／model／gain設定は変更なし。
- UIはnative入力を「Pulse・時刻同期対応」と表示するが、valueはexact ID。CPALは従来経路のまま。

### 失敗を保持

CPAL/ALSA timestampを流用したtimed probeはFAIL：初回unknown625、gap2081／overlap2031。anchor版もunknown653、別診断617でFAIL。**CPALの時刻同期が直ったとは主張しない**。そのためlive CPALをtimed mixerへ自動移行せず、native Pulse入力を別の選択肢とした。ログ `meetily-timed-sync-probe-{first,clock-anchor,clock-diagnostics}.log`。

native lifecycle診断も初回FAILを保持：Pulse17のJSON module一覧はindexを省略するため、numeric/stringの両扱いでも解決しなかった。`pactl list short modules` のexact name＋argumentから一意indexを確認する試験側修正。その後idle suspended sourceのcallback待機がFAIL。owned private null sinkへの人工再生をlifecycle試験中だけ追加し、remove→error→missing拒否→明示recreate→timed callback復帰→bounded Stopを確認した。productのwait／同期合格条件を緩めてはいない。`native-shared-lifecycle[-fixed,-final,-index-fixed]` FAIL、`native-shared-lifecycle-active` PASSを全保存。

### 最終検証

同じprivate null-sink timelineをnative mic remapとPC monitorで取得し、**一つの人工再生**で比較（別々のplayback起動差を同期と誤認しない）：

| run | 両入力の保持bursts | late／gap／overlap／invalid／overflow／stall | PC−mic位置差 |
|---|---:|---:|---:|
| native shared・20秒 | 10／10 | 全0 | 約−10ms |
| native shared・60秒 | 30／30 | 全0 | 約−10ms、変化なし（10ms解析解像度） |
| native shared・20秒＋remove/reconnect | 10／10 | 全0、lifecycle PASS | 合格範囲内 |

60秒の最大handoff gapはmic113.543ms／PC104.078ms。source read-head推定とsample配置の部分検証であり、実機latency／multi-hour drift／精度の合格ではない。

本体embedded GUI＋実local-Qwen、公開FLEURS11.1秒−20dB＋private zero mic、UI選択保存→browser reload→Start/Stop→native reopen：

| run | ASR segments | 保存秒 | stall／late／error events | 全体 |
|---|---:|---:|---:|---|
| native dual初回 | 1 | 12.8393125 | 全0 | PASS |
| fresh native dual repeat | 1 | 13.894625 | 全0 | PASS |
| fresh native dual repeat2 | 1 | 14.0499375 | 全0 | PASS |
| PC-only回帰 | 2 | 12.311875 | 全0 | PASS |

全4runsでcapture-time有効、100ms fragment、PC latency診断、ASR非空、event/raw/SQLite text一致、completion、decode warning0、checkpoint／temporary入力cleanup、UI再起動・保存files不変、owned helpers終了を確認。保存秒の起動silenceも隠さない。zero mic試験であり、両者に実発話がある場合のASR精度／AEC／重複音の品質検証ではない。

native unit **66PASS**（clock5＋timed8＋Pulse5＋pipeline15＋旧mix14＋norm10＋VAD7＋state2）、Python metrics4PASS、frontend picker6PASS、TypeScript成功。production source harness18PASS＋既知DSP2expectedFAIL（saturation／aliasは未修正）。最終ログ `meetily-timed-sync-final-{regressions,frontend-tests}.log`、`native-shared-60s`、`public-single-final`、`public-native-dual[-repeat,-repeat2]`。

候補 `../runtime/nixos-timed-sync-candidate` → `/nix/store/gal613ay5fjacyfg0y0fw4bxqg793ic1-meetily-local-candidate`、inputs `../runtime/timed-sync-candidate-oSHEYB/inputs`。built/staged binary SHA256一致、dev binary復元SHA256一致を再確認。隔離frontend exportのみ使用、通常dev server／表示中5wh候補／既存録音は不変。候補はdebug・no-bundleでreleaseではない。

人工transportの再現（repoで実行、host音源不使用）：

```sh
nix-shell shell.nix --run 'CARGO_TARGET_DIR=/home/user/Code/meeting-assistant-research/murmur/target cargo build --locked -p meetily --example dual_latency_probe'
nix-shell shell.nix --run 'python3 scripts/probe_dual_latency.py --probe ../murmur/target/debug/examples/dual_latency_probe --alsa-plugin-dir /nix/store/dgipq4hrj8a8cp55c7szkb7ghr9ajpd2-alsa-plugins-1.2.12/lib/alsa-lib --mode native --shared-source-clock --seconds 60'
# lifecycleには --seconds 20 --native-input-lifecycle を使用。
# GUI回帰は scripts/test_public_virtual_audio.py の明示候補／model／ORT等に
# --dual --native-mic --pc-db -20 --select-via-ui --expect-normalization
# --expect-fragment-100ms --transport-diagnostics を指定（fresh private Xvfb）。
```

次に表示へ反映するにはユーザーの保存完了・閉鎖確認が必要。その後native Pulseマイクをユーザーが明示選択する必要があり、既存CPAL aliasを自動置換しない。実二入力／pause中buffer境界／server再起動／multi-hour・drift／処理窓短縮は未検証または未実施。

---

## 前工程：PC低latency修正後もCPAL起動差が残っていた記録

## 承認・範囲

ユーザーは入力UIについて「ok, できた」と報告。その後 `/plan` で二入力遅延の修正案を依頼し、「計測→PC低遅延比較→必要なら時刻同期→処理単位短縮」の計画に「じゃあ、進めて」と指示。今回の作業は公開／人工音声＋完全隔離仮想環境のみ。実mic／物理speaker／表示中preview／OS音量／cloud／releaseに触れない。UI成功報告を実二入力の同期・精度合格と読み替えない。

## 実装

- `capture/pulse.rs`の`MonitorLatencyMode`で`FragmentOnly`（従来0x0200 DONT_MOVE）と`AdjustSource`（0x2200 DONT_MOVE＋ADJUST_LATENCY）を比較可能にした。buffer attributesは引き続きfragment19200bytes／100ms、残りUINT32_MAX。現在のソースの通常`PulseCapture::start`はAdjustSourceを使用する。表示中候補は旧版なのでまだこの変更を含まない。
- このflagは取得元のlatencyも要求する。実serverが要求を尊重する保証や、共有sourceへの無影響保証はない。OS設定を恒久変更する操作はせず、比較は専用private serverだけ。
- `examples/dual_latency_probe.rs`：本体CPAL mic＋AudioCapture、native Pulse PC＋AudioCaptureから本体と同じsample-order ringへ渡すnative診断。VAD／ASR／encoderなし。共通セッション時刻でhandoff間隔／channel滞留／人工burstのsample位置／late／overflow／stallを測る。既存ringをpath includeして使い、別実装へ置き換えない。時刻は収録hardware timestampではない。
- `scripts/probe_dual_latency.py`：毎回fresh HOME/XDG／Pulse `-n`／Unix socket／null sink2個／exact-source ALSA aliasで起動。source/module/capture routeを検証し、同一の100ms・700Hz人工burstを2秒おきに両sinkへ送る。playback process開始差も数値記録。20／60秒に限定、native側90秒deadline。owned helpersを終了する。
- `pipeline.rs`：`MEETILY_AUDIO_TRANSPORT_DIAGNOSTICS=1`で、source enum／chunk番号／sample数／handoff時刻／pipeline受信時刻／mix出力位置・時刻をnumeric logへ出す。通常OFF、環境判定はOnceLockで一度だけ。late warningにはsource enumを追加し、未知sourceを推測しない。sample内容／transcriptは新診断へ出さない。
- `test_public_virtual_audio.py --transport-diagnostics`と`dual_latency_metrics.py`でprivate logをローカル集計。toolsへはnumeric/boolだけ。診断を要求したのにtelemetryがない場合は合格にしない。

## 同一probeのA/B（失敗は保持）

同じproduction capture／ringでflagだけを変える比較。処理窓600ms／不足時1.2秒wait／late discard／exact Stop／mic処理／PC norm／ASR設定は維持。

| mode/run | mic/PC input bursts | PC最大handoff間隔 | stall | late samples | ring出力のPC−mic位置差 |
|---|---:|---:|---:|---:|---:|
| fragment・20秒 | 10/9 | 1888.74ms | 6 | 55381 | ＋380ms（9対のみ） |
| adjust・20秒 | 10/10 | 102.39ms | 0 | 0 | −80ms |
| adjust・60秒1 | 30/30 | 102.14ms | 0 | 0 | −80ms |
| fragment・60秒 | 30/29 | 1879.00ms | 23 | 206367 | ＋540ms（29対のみ） |
| adjust・60秒2 | 30/30 | 102.55ms | 0 | 0 | −80ms |
| adjust・最終60秒＋lifecycle | 30/30 | 102.71ms | 0 | 0 | −80ms |

baselineのPC buffer latency表示は約1.62秒。adjustの60秒runsでは最大21.145／49.666／21.604ms。人工burstの位置差は60秒で10ms block解像度の増加なし。これはsample index差で、hardware clock同期／実機でのdriftなしの証明ではない。Mic HPF／normalizer／capture resamplerはそのままで、processing delayも含む。

最初の比較はpulse保全／sync指標を中心に診断。最終60秒は追加した固定probe target（handoff p95≤150ms／max≤300ms、burst位置差≤100ms、位置差の変化≤20ms）を検証しPASS。これらは人工transport probeの目安であり、製品全体／認識精度の受入基準ではない。以前のFAILを再採点・削除しない。

同じprivate serverで既存`pulse_monitor_smoke`も実行し、人工toneの繰返しstart/stop／pause/resume／source削除／no fallback／明示再接続／module cleanupが成功。

## 本体GUI＋実Qwen：PC-only PASS、dualはFAIL

初期embedded low-latency候補で、公開FLEURS日本語11.1秒−20dB／private virtual／UI選択→保存→browser reload→Start/Stop→native reopenを確認。

| run | sampled PC buffer latency | ASR segments | stall | late | 全体 |
|---|---:|---:|---:|---:|---|
| PC-only | 583µs | 2 | 0 | 0 | PASS |
| dual | 625µs | 2 | 1 | 4978 | FAIL |
| 独立dual repeat | 708µs | 2 | 1 | 8879 | FAIL |
| source別診断dual | 541µs | 2 | 1 | 2749（全Mic） | FAIL |

全runsで100ms fragment一致／新latency<1.2秒診断は成功、UI選択保存・browser/native再起動・ASR非空・event/raw/SQLite・completion・decode warning0・cleanup成功。だがdualのrecording-errorを無視して合格にはしない。

### 起動時のsource差を特定

source別診断で以下を計測：

- 最初のhandoff：PC **0.662979秒**／Mic **1.916546秒**、差約1.254秒。
- 定常handoff最大gap：PC102.382ms／Mic59.082ms。
- channel滞留最大：PC0.630ms／Mic3.001ms。今回の計測でchannel backlogが秒単位遅延を作った形跡はない。
- 最初のmix出力 **2.465217秒**だけdegraded開始。late2749samplesはすべてMic、PC0。

**PCの秒単位配送遅延とは別に、起動時の入力供給差が残っている。** 本体ringは各入力の最初のsampleを同じ0位置へ置くsample-count方式。Mic最初のhandoffが遅れた理由（server/sourceの起動／backendのbuffer／収録時刻との関係）はまだ確定していない。handoff時刻を収録時刻と断定したり、固定1.254秒offsetで補正したりしない。

## 検証・候補

- native Pulse4＋mixing14＋pipeline12＋normalizer10＋VAD7＝47PASS。numeric計測追加後にもpipeline12PASS。
- Python metrics4PASS：source分離／gap・滞留／late集計、空logの非捏造、無関係content非export、旧unknown discardをsourceへ割り当てない。保存済private診断との抽出結果一致もPASS。
- source regression18PASS＋既定DSP2expectedFAIL再現。hard saturation／alias rejectionの既知failureは未修正。
- Python compile／rustfmt（Pulseと新example）／git diff --check成功。
- 初期候補 `../runtime/nixos-dual-latency-candidate` → `/nix/store/cxghkvrf9zpg7chgfphy2kjy19mch4d8-meetily-local-candidate`。
- numeric診断込み候補 `../runtime/nixos-dual-latency-diagnostics-candidate` → `/nix/store/38n0xk98phgi95n97dy2apbr05d5hckx-meetily-local-candidate`。inputsは`../runtime/dual-latency-diagnostics-gLReDd/inputs`。built/stage equality／dev binary復元確認。既存隔離frontend exportを使用し通常`.next`／`out`は変更なし。
- ログprefix：`../logs/meetily-dual-latency-`（`build-tests`、`native-regressions`、`instrumentation-tests`、`metrics-tests`、`source-regressions`、`fragment-first`／`adjust-first`、`*-60s-*`、`candidate-build/stage`、`diagnostics-build/stage`、`public-single/dual/dual-repeat/diagnostics`）。全失敗を保持。

## 未完と次の修正境界

今回でPC配送設定の効果は測れたが、二入力遅延全体は**未完了**。新候補は表示中previewへ反映しない。

次はCPALの収録timestamp／Pulse timingの信頼性・共通単調時計への対応を計測し、開始offset・欠落・pause/resumeを表現するsource時刻モデルを設計する。handoffを収録時刻へ流用しない。600msのwindow短縮／wait延長／startup前の音声切捨てで警告を隠さない。既存sample-orderテストを保持し、人工的な開始差・到着jitter・真の欠落・復帰・Stop境界でsource時刻同期の追加回帰を先に作る。AudioChunk時刻契約／startup録音範囲を変える前には具体的方式・欠落時の扱いをdecision gateで確認する。

マイクbackend変更、固定offset、処理窓短縮は今回未実施。実mic／desktop／cloud／実会話内容export／自動ASR補正／model・VAD・gain変更／releaseなし。表示中appと既存録音は保持。
