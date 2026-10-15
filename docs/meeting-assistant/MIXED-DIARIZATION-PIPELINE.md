# 入力別音声保持＋混合音声1系統の話者分離：選択と第1段階検証

> 最新判断で話者分離の採用を取り下げ、入力別文字起こしのみへ変更。[SOURCE-TRANSCRIPTION.md](SOURCE-TRANSCRIPTION.md) が現行方針。本書は過去の検証記録として保持する。

## ユーザー選択・境界

ユーザーはC++を選び、同時実行試験後、マイク・PC音声を別保持または混合して話者分離し、その発話区間を文字起こしへ渡す案を提示。方式選択で**「入力別音声を保持し、混合音声で話者分離（推奨）」**を明示選択した。

選択した検証構成：

```text
mic lane ─┐   同じsample時間軸で入力別音声を保持
          ├→ mixed PCM → cpp Q8 diarization（1系統）
PC lane ──┘        └→ 既存VAD／文脈保持ASR → 時間範囲による暫定speaker候補付与
```

まず既存VAD／Qwenの認識経路を維持し、公開fixtureでデータ契約と候補付与を検証する。**第1段階は隔離prototype＋offline trace replayであり、本体パイプライン／録音／DB／GUIの変更ではない。** diarだけでVADを置換、細切れASR、話者不明の音声破棄、同時発話を一人へ強制、元raw text編集を行わない。

## 実施した第1段階

- 既存公開Lingua Libre2話者短語反復`turns.wav`60秒／mono16k float32を使用。fixture referenceのsource1を人工mic lane、source2を人工PC laneへ割り当て、間の無音は両laneゼロ。**実機のmic／PCや同時発話ではない。** referenceは人工lane生成だけに使用し、話者候補付与にはcppの出力だけを使う。
- laneそれぞれをpublic専用WAVとして外部runtimeに保持。両laneの合計が元mixed PCMと全float32 bytes一致、保存WAV roundtrip一致、60秒／960000samples共通origin0を確認。保管した公開WAVはこの試験のevidenceであり、実ユーザーの入力別録音保存を追加したものではない。
- 1mixed streamへcpp最適化Q8／low／CPU4threads、現在の実local Qwen1.7B decoder-INT8／JA／float32／CPU6threadsを同時実行。Qwenはdefault worker VAD presence gate／full-input10秒×6request（**本体ContinuousVadProcessorの可変endpoint scheduleではない**）。モデル・input gain・gate・default・補正不変。
- 結果：cpp RTF **0.683**、Qwen request RTF **0.573**、cpp最終push遅れ0.250ms、Qwen最大開始待ち0.086ms／Stop相当drain4.264秒、全6realASRcall／非空／error0。cpp cluster2／45segments／identity proxy98.70%（人手DERではない）。
- cpp最大一時push遅れ2.423秒／最大暫定label age **3.503秒**。sampled合算RSS7633.3MiB（PSSではない）。固定60秒throughput criteriaはPASSだが即時label表示／自然会議精度の受入ではない。
- cppで観測済みのvisible segmentsを各Qwen response完了時刻までtrace replayし、final segmentsとの候補範囲比較。全6区間でdelivery→finalの付与結果一致。ただしtentative／confirmed=falseを維持し、将来revision不可とは主張しない。
- **全6文字起こし窓でspeaker候補が2つ**。`single_speaker_id=null`とし、一方へ任意に割り当てない。Qwen textは区間全体で、word timestampがないため話者ごとのtext分割を捏造しない。
- VAD gate coverage内のspeaker不明samplesも保持。worker gateには前後padding／内部間隙が含まれ、diar labelがない部分は必ずしもspeech missではない。音声を削除しない、不明coverageを自然音声のmiss率と呼ばない。

## Prototype契約

`scripts/attach_diarization_labels.py`：

- sample origin／16k時間軸でQwen window-local VAD gateをabsolute座標へ変換。
- VAD gate unionとdiar label intervalを交差。匿名speaker IDのsetとcoverage／unknown／overlapを保持。
- speaker候補が1つでもunknown／overlapがあればsingle speakerにしない。複数turnのwindowでは、長い話者を勝者に選ばない。
- `raw_text`／`recognition_context:full_input`／元input lengthは変更しない。
- `provisional:true`／`confirmed:false`／`text_alignment:whole_asr_window_only`を明示。時間だけでword alignmentが成立したとしない。
- diar float秒は最近傍16k sampleへ丸める。engine10ms-bin proxyと人手DERは別。word-aligned精度は未検証。
- artificial overlap／unknown／single／multi-turn／absolute origin／gate union／invalid output拒否のpure7tests PASS。実同時発話の音源分離をしたわけではない。

## 次に必要な第2段階（まだ実装していない）

1. **ASR前の区間編成**：話者turn候補を扱いつつ前後余白と十分なcontextを保つ。short-word cropの既知誤認識を残し、全入力参照との比較で評価する。同じspeakerの統合でも途中sampleを黙って削除しない。
2. **不明／重複／revision**：不明音声は既存VAD経路へ残す。重複発話ではmixed音声だけから独立speechを復元できない。source laneの利用／エコー／同一人物の入力横断を区別する。
3. **backpressure／Stop**：cppが1秒超blockするのでcapture callback／UIから独立したowned worker、bounded queue、sample index、final flush／drain／failure時の元音声保持が必要。discardでリアルタイム成功に見せない。
4. **入力別保持の製品契約**：実アプリでの保持時間・一時ringか永続録音か・disk容量・削除／復旧／DB紐付けは未定。今回のpublic lane WAVをそのまま本番保存仕様にしない。
5. **検証**：可変VAD schedule、短い頻回／長い30秒request、長時間、重複・echo、ASR同時CPU競合、実native Stop-save-restart／障害。その後に本体へのopt-in接続とUI保存方式を判断する。

## 再現・証拠

```bash
cd meetily
nix-shell benchmarks/qwen3-asr/shell.nix --run \
 'timeout 240s python3 scripts/probe_cpp_qwen_concurrency.py \
   --runtime ../runtime --output-dir ../runtime/diarization-engines/pipeline-lanes-turns-new \
   --mode both --fixture turns --keep-fixture-lanes && \
  python3 scripts/attach_diarization_labels.py ../runtime/diarization-engines/pipeline-lanes-turns-new'
nix-shell -p python3 --run \
 'python3 -m unittest discover -s scripts -p "test_attach_diarization_labels.py" -v'
```

raw directoryは新規のみ、付与結果の上書きも拒否。`lane-mic.wav`／`lane-pc.wav`を保持し、一時ASR window WAVのみ削除、owned cpp／Qwen process終了を確認。

- Raw：`../runtime/diarization-engines/pipeline-lanes-turns-r1/`。
- Log：`../logs/diar-cpp-pipeline-lanes-r1.log`。
- 機械可読：`evidence/diarization/mixed-pipeline-stage1.json`。
- 先行負荷試験：[CPP-QWEN-CONCURRENCY.md](CPP-QWEN-CONCURRENCY.md)。入力source／license：[DIARIZATION-ENGINE-COMPARISON.md](DIARIZATION-ENGINE-COMPARISON.md)。

実mic／desktop収録／物理再生／cloud／資格情報アクセス／newmodel／default変更／app lock変更／本体releaseなし。表示中appは変更・再起動していない。既存保存会議／raw text／音声は編集しない。
