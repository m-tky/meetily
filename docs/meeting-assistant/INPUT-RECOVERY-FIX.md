# 二入力degraded recovery修正とPC取得バッファの診断

## 決定・修正

入力UI追加後、ユーザーは「復帰不具合を先に修正」を明示選択。`AudioMixerRingBuffer::next_window`の`degraded`による将来区間の待機bypassを撤去。各missing区間は再び最大1.2秒待つ。degradedは警告episodeだけを表し、両入力が戻ると解除。既に出した区間の遅着破棄、Stopの非待機／exact tail、sample-order方針を保持。mic処理／gain／model／VAD／resamplerは変更しない。

追加production regressionsは、両source方向の「片方が先に届く→古いlate分→未来分→同じ順序を反復」、区間毎有限待ち＋episode毎warning、復帰待機中Stop exact tailの3件。既存timeout testは誤った即時2区間目出力の期待を有限待ちへ更新。初回の旧期待failureログを保持。

## 検証・限界

- Native mixing14＋normalizer10＋pipeline12＋VAD7＝43PASS。
- Std-only source test（DeviceType importだけmock）で旧bypass guardを復元すると追加3件が3FAIL、修正guardでは3PASS。これは旧binaryのnative testではなく、guardのcontrolled before/after。
- `verify_dual_audio_code.py`の自動included module test増加に合わせ15→18を更新。18PASS＋unchanged hard saturation／alias rejectionの2expected FAILを再現しexit0。品質合格を意味しない。
- UI付き公開FLEURS−20dB、private PC-only全体PASS。private dualは1run全体PASS、その独立repeatは全体FAIL（stall3／late16899samples）。永久破棄は修正したが、normal two-inputの安定受入とはしない。
- private stream診断runもstall3／late37459。実際にprivate mic1／PC1のuncorked capture streamがあることを検証。Micはzero fixture。
- paplay latency50msだけをprivate testで指定したcontrolも全体FAIL。production設定／OS default／native capture設定は変更していない。結果を保持し、fixture timing変更で合格を作らない。
- active-buffer診断runもstall3／late27877、保存・認識非空・event/raw/SQLite・native再起動・UI保存復元・cleanupは成功。再生2秒後、Mic buffer latency240µsに対し**PC buffer latency1,620,937µs**を計測。1.2秒waitを超える。Source latency値は双方0。
- PC `pulse.rs::connect`は`pa_stream_connect_record`へbuffer attributes=NULLを渡し、server既定のcapture fragmentに任せている。短い取得単位を明示する候補だが**未変更**。閾値やbuffer policy変更前にユーザー決定が必要。

## artifacts・表示

- debug／no-bundle candidate：`../runtime/nixos-input-recovery-candidate` → `/nix/store/kdwnlh2h3fgxjvmghgwkza7yxk8a3zhc-meetily-local-candidate`。UI＋承認済みPC norm＋recovery fixを含む。built input equality／dev binary復元確認。
- 旧input-UI候補と失敗結果を保持。新buildは同じisolated frontend exportを使い、通常`.next`／`out`を変更しない。
- ログprefix `../logs/meetily-input-recovery-`：`native-tests`、`native-legacy-expectation-failure`、`before-after`、`old_bypass_guard-snippet-tests`、`fixed-snippet-tests`、`source-regressions`、`candidate-build/stage`、`public-single/dual/dual-repeat`、`private-stream-diagnostics`、`private-latency-control`、`active-buffer-diagnostics`。
- 表示中の`dual-preview-spocggm1`は未変更。新候補への切替えはまだ行わない。実mic／physical playback／cloud／native log内容export／releaseなし。

この後ユーザーは100ms fragment指定を明示承認。実装とprivate公開fixtureのPC-only＋独立dual3runsではfragment一致／warning0だが、buffer latency表示は1.621秒のまま、新diagnostic全4FAILを保持。現在の候補／結果／制限は[PC-CAPTURE-FRAGMENT.md](PC-CAPTURE-FRAGMENT.md)。buffer latency値は単独瞬間値であり、全遅延・全失敗の原因証明／精度合格ではない。表示中previewは依然未変更。
