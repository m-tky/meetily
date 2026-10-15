# Qwen3 ForcedAligner：文字時刻・話者別本文の隔離検証

> 最新のユーザー判断は文字起こしのみ・話者分離なし。ForcedAlignerの採用計画は取り下げた。[SOURCE-TRANSCRIPTION.md](SOURCE-TRANSCRIPTION.md) が現行実装。本書の実測・失敗は履歴として保持する。

## 結論

**任意worker接続と4項目の試験は実施。文字単位版・公式版とも会議用途の採用は保留。**

文字単位版で時刻の潰れ／話者割当の失敗が出たため、追加依頼どおり公式 `qwen-asr==0.0.6` / `nagisa==0.2.11` でも同じ音声・同じraw ASR文を比較した。公式版は割当の代理指標が改善したが、短語反復・話者境界・重なり・ASRの文脱落は解消しない。ForcedAlignerは誤認識の検証器ではない。

本体／表示中app／入力route／録音／DB／defaults／既存app用worker／lock／OS設定／releaseは変更しない。実mic／desktop capture／物理再生／cloud推論なし。公開音声・syntheticのみ。過去の失敗を新しい成功で置換しない。

機械可読結果：[evidence/diarization/forced-alignment.json](evidence/diarization/forced-alignment.json)。raw音声／認識文／時刻／trace／provenanceは `../runtime/qwen3-asr/diagnostics/` に保持。232 artifact hashesを記録。

## optional worker

- `--aligner-dir DIR` と `transcribe` の任意 `align:true`、独立 `align` requestを追加。既定はalignなし。VAD・ASR model・full original input・30秒上限を維持。
- 日本語のみ、最大512 Unicode scalar。元textと文字offsetを保持。句読点・zero-duration・範囲外等の不明単位は無理に話者へ付与しない。
- ForcedAlignerは遅延ロード。ASRと別model/session。無音transcribeはASR／alignmentを実行しない。明示した提供文によるalignは未検証文として別扱い。crop-contextとのinline alignmentは拒否。
- `confirmed:false` / `recognition_verified:false`。timestampは上流LIS／補間後の値であり、整った数値でも認識の正しさは保証しない。
- 日本語文字単位はローカルRust実装の方式。**公式のnagisa語単位と同等ではない。** 公式controlはインストール済み公式モデル／前処理を変更せず、CPU float32/eager・local-onlyで実行。runtime／入力構築も異なるため、改善をnagisaだけの効果と断定しない。
- inline `transcribe + align` は応答まで後処理時間を含む。これを本体ASRの必須gateへ採用しない。並行試験はASR・alignment・cppを別owned processにして、ASR後にbounded queue（容量2）でalignへ渡す。
- 実験build：`../runtime/qwen3-asr/align-worker-target`。試験時binaryは `align-worker-tested-r2` に別保存。後のfmt-only rebuildでも全15fixtureのraw text／unitが一致。app用 `worker-target` は置換しない。

```json
{"op":"transcribe","id":"example-1","wav":"/absolute/public.wav","language":"Japanese","align":true}
{"op":"align","id":"example-2","wav":"/absolute/public.wav","text":"提供した未確認文","language":"Japanese"}
```

モデル `Qwen/Qwen3-ForcedAligner-0.6B`、revision `c7cbfc2048c462b0d63a45797104fc9db3ad62b7`、Apache-2.0。Git blob／LFS SHA256／sizeを検証。モデルはcheckout外。downloadの停止／timeoutと再開logも保持し、最終全file hashを再確認した。重複range／serial partialは診断artifactとして残る。

## 1・2：文字時刻／10〜30秒の話者別本文

初期12fixture：FLEURS日本語2文、公開人声「車」「本」の10／20／30秒交代と20秒逆順、いいえ原音／padding、eSpeak「はい」「うん」、重なり10秒、無音5秒。eSpeakは自然な相槌の正解ではない。

追加3fixture：完全なFLEURS文2件を22秒順／逆、30秒3文へ配置。男性metadataのvalidation0/id1519と女性metadataのvalidation34/id1612。元文を切らず、参照文はASRへ与えない。gender情報に基づく別sourceであり、人手の話者同定／自然会議ではない。初期plan／失敗をそのまま保持した別plan。

speaker IDは匿名。評価時だけsource区間との最大重複でclusterを対応付ける。単一speakerで80%以上被覆かつ他speakerとの接触なしのunitだけ割当、境界／複数／gap／flag付きはunknown。unknownも本文から削除しない。公式語単位のlabelを文字へ継承して代理誤り数を比較するが、**語を文字に分割した正確な時刻を捏造しない**。raw ASR文は別に全保存、公式前処理による句読点除去とraw本文の変更を混同しない。

以下はalnum正規化した参照文字数と **本文＋source話者の編集距離（unknown含む代理指標）**。人手の文字timestamp／DERではない。

| fixture | 参照文字 | ASR文字誤り | Rust文字版の話者付き誤り | 公式版の話者付き誤り |
|---|---:|---:|---:|---:|
| 短語交代10秒 | 6 | 0 | 4 | 4 |
| 短語交代20秒 | 14 | 0 | 12 | 7 |
| 短語交代30秒 | 21 | 0 | 18 | 12 |
| 短語20秒逆順 | 14 | 0 | 11 | 6 |
| 完全文22秒 | 78 | 32 | 37 | 33 |
| 完全文22秒逆順 | 78 | 0 | 33 | 16 |
| 完全文30秒 | 110 | 32 | 62 | 44 |

- 文字版は自然な単独文にもzero-durationが8/49・16/84、最初の文に範囲外1unit。元文字列は保持しているが、安全な話者割当には不足。
- 公式版は同じ単独文のalnum文字を45/46・79/80割当できた。ただしこれは時刻の人手精度ではない。
- 反復短語は公式でもzero-durationが残り、認識文が合っていても話者別本文は正しくならない。
- 完全文順／30秒はASRが1文32文字を落とす。逆順では全文認識できるが、話者付き誤りは公式でも16/78。alignmentはASRで欠落した文を復元しない。
- Nativeはfresh3runs（fmt後を含む）で全15fixtureのraw text／units一致。公式は33alignment条件×fresh2runsでtext／status／units一致。失敗も再現した。汎用determinism／品質受入ではない。

## 3：相槌・重なり・誤った認識文

- 原音「いいえ」は公式語単位で3文字を割当できるが、padding条件のraw ASRは「家。」のまま。公式はその誤文1文字にも時刻を返す。認識後補正なし。
- synthetic「はい」は公式で割当できるが「うん」はunknownのまま。自然な相槌の精度は未検証。
- 重なり10秒：Rustは本文6文字を全unknown。公式はalnum3文字中1文字を単一speakerへ付与するが、そのunitは既知の複数source区間と交差する。**cppが重なりを見落とすと保守的な合成規則でも誤帰属し得る**。成功として扱わない。
- 意図的な「山」は両実装とも正常そうな1.2〜1.44秒を返す。音声と文が一致した証拠ではない。
- 無音に提供文「はい」を明示alignすると、Rustはzero-duration、公式は0〜2秒の正の区間を返す。通常の無音transcribeはVADでskip／空のまま。提供文alignの成功statusをspeech検証へ使わない。

## 4：3モデルのCPU・memory・latency

5650GE（6core/12thread）／CPUのみ、cpp Zen3 Q8 low4threads、現在Qwen1.7B decoder-INT8 ASR6threads。同じpublic repeat60s、cpp120ms paced、ASR10秒×6、原文をalignへそのまま渡す。初回ロード／初回推論はwarmup.jsonに分離。これはprocess-coldでありOS page cacheをcold化した試験ではない。cold RSSは未測定。

| 構成 | cpp RTF | ASR request RTF | align request RTF | 合算RSS peak | cpp label age最大 |
|---|---:|---:|---:|---:|---:|
| ASR＋cpp対照 | .697 | .612 | — | 7.44GiB | 3.84秒 |
| ＋Rust文字版6threads | .776 | .576 | .283 | 9.87GiB | 4.69秒 |
| ＋公式6threads・paced3runs | .710〜.861 | .578〜.693 | .178〜.211 | 11.37〜11.69GiB | 3.65〜9.11秒 |
| ＋公式2threads・探索1run | .703 | .580 | .297 | 11.68GiB | 3.36秒 |
| ＋公式6threads・offline backlog | .952 | .879 | .462 | 11.70GiB | 12.07秒 |

- pacedの平均owned CPUは対照5.60、Rust6.81、公式6threads約6.33〜6.99論理CPU相当。2threadsは5.79。CPU秒は `/proc` サンプル、RSSはresident合算でPSS／leak証明ではない。
- 10秒入力ごとにはASR終了後のalignが次ASR開始前に完了し、3者が同時in-flightの時間は0。resident3モデル＋cppとの交互競合は測ったが、3者計算同時と呼ばない。
- 別の **offline backlog stress** は全ASR入力をepochから利用可能にし、cppだけ実時間供給。3者request／cpp pushの重複23.49秒、平均8.59論理CPU相当。kernel-level profilerではない。live会議条件ではない。
- pacedのalign開始待ちは0.38ms未満、queue sample最大1。backlogでも19.12ms／最大1。ASR文字列は渡したまま保持。全owned processをreap、thread終了／一時window WAV削除。
- 平均RTFは各1未満でも、cpp最終push遅れは対照.535秒、Rust3.09秒、公式6threads .317〜7.84秒。事前の120ms最終push基準は **対照も含め不合格**。全体Stop drainはpaced約5.19〜8.80秒、backlog5.87秒。会議の低遅延合格としない。
- 旧raw resultの `final_stop_drain` はASR＋alignのみだった。原dataは保持し、aggregateはcpp／flushも含め訂正。旧epoch差は0〜20msのassertだけで値未保存のため、cppが支配する訂正値は20ms上界使用。新backlog resultは差を使った正確な全体drain。
- `three-model-official-backlog-r1` はCLIのflag忘れで実際はpaced。planを正としてpaced3runの1つに数え、stressは **`three-model-official-actual-backlog-r1`** のみ。名称から条件を推測しない。
- 公式2threadsの良い1runで6threadsの悪いrunを撤回しない。host variationとthread設定効果を分離するには追加の対照反復が必要。

## 再現・検証

```sh
nix-shell benchmarks/qwen3-asr/shell.nix
export RUSTFLAGS='-C target-cpu=znver3'
export CARGO_TARGET_DIR=/home/user/Code/meeting-assistant-research/runtime/qwen3-asr/align-worker-target
cargo test --release --locked --offline --manifest-path benchmarks/qwen3-asr/worker/Cargo.toml
cargo build --release --locked --offline --manifest-path benchmarks/qwen3-asr/worker/Cargo.toml
python3 benchmarks/qwen3-asr/evaluate_forced_alignment.py --runtime ../runtime \
  --directory ../runtime/qwen3-asr/diagnostics/forced-alignment-r1 --pass-id NEW_INTEGER
# 同じnative passのraw文を、既存の専用CPU venv／local-only公式wrapperへ渡す
../runtime/qwen3-asr/forced-aligner-reference-venv/bin/python \
  benchmarks/qwen3-asr/official_forced_alignment_reference.py \
  --model ../runtime/qwen3-asr/artifacts/model-forced-aligner-0.6B \
  --directory ../runtime/qwen3-asr/diagnostics/forced-alignment-r1 --native-pass NEW_INTEGER
python3 benchmarks/qwen3-asr/compare_forced_alignment.py \
  --directory ../runtime/qwen3-asr/diagnostics/forced-alignment-r1 --pass-id NEW_INTEGER
python3 scripts/probe_three_model_pipeline.py --runtime ../runtime --output-dir EXTERNAL_NEW_DIRECTORY \
  --aligner-threads 6 --official-alignment-worker ../runtime/qwen3-asr/official-aligner-worker
# 真の3者同時負荷stressには上記へ --offline-backlog を明示
```

`official-aligner-worker` は専用venv Pythonから `benchmarks/qwen3-asr/official_alignment_worker.py "$@"` をexecする外部shim。一般app workerの代替ではなく、bounded align専用の試験backend。公式package／source hash／モデルprovenanceは公式controlに記録。Python依存は専用venvのみ。

Rust17（14 library＋3 protocol）、Python35（supervisor5＋alignment7＋既存diarization/attribution23）、locked/offline release build、fmt、compile／JSON／hash checks成功。既存app用workerと実験workerの既定alignなしで公開文＋無音のtext／status／ASR calls一致、alignment欠如／全unconfirmedを確認。test総数の二重計上なし。

自然会議／人手の文字時刻／長時間／実入力lane／本体の可変VAD schedule／revision／GUI保存／議事録との同時負荷は未検証。次候補は公式の入力構築・語単位をRust側で照合することと、独立境界正解での検証。**本体採用・diarをASR gateへ変更・自動speaker確定は、この結果だけで行わない。**

Sources: [公式repository](https://github.com/QwenLM/Qwen3-ASR)、[ForcedAligner model](https://huggingface.co/Qwen/Qwen3-ForcedAligner-0.6B)、[公式aligner実装](https://github.com/QwenLM/Qwen3-ASR/blob/main/qwen_asr/inference/qwen3_forced_aligner.py)。
