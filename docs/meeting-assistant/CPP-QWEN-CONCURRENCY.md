# parakeet.cpp Q8＋QwenのCPU同時実行試験

## 範囲・結論

ユーザーは組み込み候補をC++へ変更し、接続範囲の確認で「同時に実行できるかテストしてみて」と回答。**承認は隔離同時試験であり、本体へのリアルタイム接続・採用・表示中app更新ではない。** 保存後手動の既方針は変更しない。

Ryzen 5 PRO 5650GEで、最適化済みparakeet.cpp Q8（4threads）と現在のlocal Qwen1.7B decoder-INT8 worker（6threads）を別processで同時実行。**60秒の固定公開音声に対して2回ともthroughputは追従した。ただしC++側の一時遅延が増え、即時話者表示の受入ではない。**

| 条件 | cpp計算／RTF | Qwen全request時間／RTF | cpp最大push完了遅れ | Qwen最終Stop相当drain |
|---|---:|---:|---:|---:|
| cpp単独 | 26.002秒／0.433 | — | 0.574秒 | — |
| Qwen単独 | — | 25.345秒／0.422 | — | 3.922秒 |
| 同時・初回 | 38.444秒／0.641 | 33.224秒／0.554 | **2.150秒** | 4.691秒 |
| 同時・fresh再実行 | 40.371秒／0.673 | 34.857秒／0.581 | **2.243秒** | 4.633秒 |

- 同時実行ではcpp計算約48〜55%、Qwen request約31〜38%増加。それでも両RTF<1で、cpp最終push遅れ0.282／0.294ms、Qwenの最大request開始待ち0.081／0.103ms。固定10秒窓の待ち列は持続増加しなかった。
- cpp最大1回push処理1.172／1.140秒。暫定label age中央値1.175／1.221秒、最大**3.230／3.323秒**（単独最大1.619秒）。処理の追従とlabelの即時性を区別する。
- 500push×120ms、全960000samplesをcppに保持。Qwenは10秒×6request、全6件で実ASR1／非空／error0／confirmed=false。サンプルや失敗を捨てて速度を良く見せていない。
- 実際のcpp push計算とQwen request区間の重なり23.581／25.882秒を観測（共有monotonic epoch、cpp開始skew<20ms必須）。単に二processを起動しただけの並列判定ではない。Qwen requestのうちneural kernelだけのsampling profileではない。
- 同時両runのQwen raw6textsは単独と完全一致。cpp raw segmentsも単独と完全一致。**今回の再現性であり、一般的なbit-exact／品質保証ではない。**
- 同時の100ms sampled合算RSS peak7622.8／7624.1MiB（約7.4GiB）。Qwen約7420.6MiB、cpp約170MiB、Python supervisor約36MiB。共有pagesの二重計上がありPSSではない。高水位RSSとsampled RSSも同一指標ではない。leak-freeの証明ではない。

## 固定条件・限界

- cpp source/model/libraryは[CPU診断](PARAKEET-CPP-CPU-DIAGNOSIS.md)と同一。`build-znver3`、Q8_0、low9+4/cache264/FIFO264/update222、AVX2/FMA/F16C=1をmodel load前に検査。CPU-only。
- 入力は既存`../runtime/diarization-engines/repeat.wav`、public FLEURS日本語clip反復60秒／mono16k float32。SHA256 `7f8c95d73fa2050addb545efda5b4a20df2164cb9acc0bcbb2d4ab9d6fe3710c`。出典・ライセンスは[比較記録](DIARIZATION-ENGINE-COMPARISON.md)を参照。短語2話者fixtureの以前のRTFと混ぜず、今回同一fixtureの単独対照を使用する。
- Qwenは既存実worker／1.7B decoder-INT8（encoder/prefill F32）、float32全入力、Japanese、default presence gate／6threads、1persistent worker。新model取得／調整／ASR補正なし。pinned model source provenance照合＋全model/cache hashesの前後一致を確認。
- Qwen first-speech warmupは別記録、request4.872〜5.145秒／model load0.580〜0.671秒。以後のpaced速度から除外。cppはmodelをepoch前にロードするが推論自体はprewarmしていない。raw planの旧`warmup_before_measurement`はQwenの意味で、最終harnessでは明示的に分離した。
- Qwenは入力EOF時刻10/20/30/40/50/60秒にserial requestをrelease。**本体VADの可変区間／短い頻回request／長い30秒request／draftやminutes LLMの追加負荷を再現したものではない。** ライブcallbackやUIやStop-save全経路も未接続。
- 事前固定criteria：両RTF<1、cpp最終push lag<120ms、Qwen開始待ち<1秒／最後drain<10秒、ASR errorなし。これを全4条件で満たすが、最大暫定label遅延や意味品質の受入基準ではない。試験exit0と品質合格を混同しない。
- 60秒のみ。数十分／多時間運転・同時発話／自然会議／熱・他app負荷／pause-resumeや障害復帰／異なるCPUは未検証。今回FLEURSの単一source proxy58.43%は全cpp条件で同じで、人手DERではなく品質合格としない。
- 次の候補は長時間＋実VAD同等のrequest scheduleでの負荷試験。リアルタイム本体接続はこれとは別のユーザー判断が必要。

## 再現・記録

```bash
cd meetily
# 各modeを順番に実行。raw出力directoryは新規でなければ拒否する。
for mode in cpp asr both; do
  nix-shell benchmarks/qwen3-asr/shell.nix --run \
    "timeout 240s python3 scripts/probe_cpp_qwen_concurrency.py \
      --runtime ../runtime --mode $mode \
      --output-dir ../runtime/diarization-engines/concurrency-$mode-new"
done
nix-shell -p python3 --run \
 'python3 -m unittest discover -s scripts -p "test_probe*diarization*.py" -v && \
  python3 -m unittest discover -s scripts -p "test_probe_cpp_qwen_concurrency.py" -v'
```

- Raw：`../runtime/diarization-engines/concurrency-{cpp-r1,asr-r1,both-r1,both-r2}/`。result／plan／cpp全events／ASR全response／memory samples／warmup／cleanupを保持。公開音声のraw誤認識を改変しない。
- Logs：`../logs/diar-cpp-qwen-{cpp-r1,asr-r1,both-r1,both-r2}.log`。元のPythonがPATHにないためNix shellを使用。
- 機械可読要約：`evidence/diarization/cpp-qwen-concurrency.json`（raw/result hashes・pin・policy・cleanup・比較一致）。要約生成初回はASR-onlyに存在しないoverlap keyを読んでKeyError、集計のみ修正。実推論の失敗ではなく、raw runを再計測や上書きしていない。
- pure新3＋既存13＝16tests PASS。process終了／一時WAV削除を全runで確認。cpp基礎harnessには任意共有epochだけを追加し、通常の単独呼出しの挙動は維持。

Meetily本体／frontend／ASR policy／DB／録音／app lock／表示中preview／OS音声設定／既存保存音声は変更しない。実mic、desktop収録、物理再生、cloud推論、資格情報アクセス、新規model download、app releaseなし。
