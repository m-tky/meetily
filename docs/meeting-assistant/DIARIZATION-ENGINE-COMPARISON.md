# CPU話者分離エンジン比較：parakeet.cpp／parakeet-rs

> **2026-10-14訂正**：初回cpp buildの`-march=native`はNix wrapperに除外され、AVX2/FMA/F16Cが無効だった。明示Zen3 buildではQ8 RTF0.536〜0.566、F16 RTF0.462で60秒実時間供給に追従。以下の旧cppの失敗・数値は汎用build対照として保持し、engine固有の性能限界とはしない。原因／新結果／再現：[PARAKEET-CPP-CPU-DIAGNOSIS.md](PARAKEET-CPP-CPU-DIAGNOSIS.md)。

## 範囲・結論

ユーザーは4候補の試験を依頼した後、**「前２つだけでよい」**へ訂正。以後はparakeet.cpp／parakeet-rsだけをbuild・model取得・推論した。訂正前に他2候補のsource cloneまで行ったが、sherpa-onnx／pyannote-rsのmodel取得・build・推論はしていない。

- **速度：最適化cppもrsもCPU候補。** rsは60秒供給に追従、RTF0.538〜0.599。訂正後cpp Q8もRTF0.536〜0.566で追従。量子化・実装経路が違うため普遍的順位を断定しない。
- **精度：未合格。** 公開日本語2話者の同じ短語反復で、一部の話者2が話者1へ混同される誤りをfresh processで再現。逆順では改善するため、順序依存の失敗として保持。
- **初回の汎用cpp buildは不合格だったが、CPU最適化後は追従。** 旧RTF2.895／F16 prefix40.12秒を保持し、SIMD未有効の対照として扱う。
- 採用／リアルタイム本体接続はしていない。保存後手動実行の既選択はそのまま。本体ASR／録音／DB／app lock／表示中app／既存録音／OS音声設定を変更していない。
- 実マイク・desktop収録・物理再生・cloud音声推論・本体releaseなし。外部Release buildはbenchmarkだけ。

## 固定環境・artifact

5650GE／CPU-only／4 compute threads／推論は順次実行、同時buildなし。

- [parakeet.cpp](https://github.com/mudler/parakeet.cpp) revision `5c9b0fbbd8e611d88e47d7e1ebae9bca9b447e8f`。
  - ggml submodule `e705c5fed490514458bdd2eaddc43bd098fcce9b`、upstreamのin-tree patch適用。主sourceを修正していない。
  - 初回Release `-O3`、GGML_NATIVE／GGML_LLAMAFILE ON。`-march=native`は設定上存在したがNix wrapperに除外され、実際はAVX2/FMA/F16C無効。CUDA／Vulkan／Metal／HIP OFF、追加CED／voice identification／server OFF。
  - C ABIでstandalone diarizationを測る。C ABIには専用thread setterがないため、このLinux試験だけexport済みC++ `pk::set_num_threads(4)`を使う。内部symbolは本体統合の安定ABIではない。
- [parakeet-rs](https://github.com/altunenes/parakeet-rs) revision `66bb38752ee899fcde8ac3d5e46b08dccb5a2db3`、v0.4.1。
  - `cpu,load-dynamic,sortformer`、default features OFF（不要なruntime自動取得／API28要求を避ける）。CPU4 intra／1 inter threads、明示Cpu provider。
  - 既存隔離ORT CPU wheel **1.23.2**、ort Rust API22、library SHA256 `718c3fb0785d5ea06061b92c32ce4ad35be1bd4c71ae6192efe06f1ab21f0f8a`。既存venvには変更なし。
  - 小さなRust driverは `benchmarks/diarization-engines/rust-probe/`。独立workspace／専用Cargo.lockでapp dependency treeには入れない。
- 両方ともNVIDIA Nemotron-3 Diarizationを利用。変換artifact／量子化は同一ではない。
  - cpp model repo `mudler/parakeet-cpp-gguf` revision `741158ae71e64ef5c89385862c18f777d07a97a1`。
    - Q8_0：108674624bytes、SHA256 `76c5bb1fb20d82706142ad32769b7ab496d2458489473a000fd7074c52ceec22`。
    - F16：200715584bytes、SHA256 `3dca41a92162af6edf96276c4d2c397b034b55abf9eb2662a58cc7bc8c872683`。
  - rs model repo `altunenes/parakeet-rs` revision `4d2a8bc71f5c896ec40faa59732e6716295edaf2`、`nemotron-3-diarization/nemotron3_diar_v3.onnx` **FP32**。
    - 400506656bytes、SHA256 `915e4fa23b0192ed9fadeb1cdd26847df986d50c92012d177be28d0343bbe03a`。
  - 全てHF pinned revisionのLFS size/hashと照合成功。取得したこのv3 exportはFP32で、INT8 v3は今回は取得／生成／実測していない。READMEの量子化説明からINT8実測済みと誤認しない。
  - engine codeはcpp MIT／rs MIT OR Apache-2.0、モデルはOpenMDW 1.1。再配布／本体採用のnoticeは別工程。

## 公平条件・計測

`scripts/probe_diarization_engines.py`で共通float32 mono16k WAVを生成し、元PCMとのbit-exact roundtripを確認。両方に**同じbytes**を120msずつ渡す。model load後にmonotonicで実時間供給、遅れてもsampleを捨てずbacklogを可視化。

両engineのupstream low presetは同じ：**chunk9／右context4／cache264／FIFO264／update222**（80ms grid）、名目buffer latency1.04秒。以前のNeMo-Speech.cpp試験（13/96/40等）とgeometryが異なるため、エンジン単体のspeedup ratioとして比較しない。

- RTF＝push処理＋finish処理／入力音声秒数、待機／loadは除外。wallは供給待機＋finishを含む。
- completion lateness＝push完了wall時刻−PCM到着予定音声時刻。持続増加と一時的blockingを分ける。
- cppはended segmentsに加え、active segmentsもqueryして現在表示可能なlabelを計測。rs `feed()`は絶対sample時刻のsegmentsを返す。
- segment rasterは10ms bin中央で判定。overlapはsetのまま保持し、勝手に一人へ決めない。終了時を越えるbinはclamp。
- 暫定label ageはfirst-visible speaker時刻−frame音声時刻（最終speakerがあるframesに限る）。**認識されない音声をこの遅延の分母から除いている**ので、精度が悪くても遅延だけ良く見える。確定label／ASR文字への付与遅延ではない。
- RSSはcpp Python供給を含む親process／rs native childのhigh-water mark。同じメモリ境界のモデル単体比較ではなく、Q8 vs FP32のprecision差もある。

## 初回比較結果（cppは汎用build対照）

| engine／model／fixture | 供給／秒 | 処理RTF | wall秒 | 最大push遅れ | RSS MiB | identity proxy |
|---|---:|---:|---:|---:|---:|---:|
| cpp Q8／2話者 | paced60 | **2.895** | 180.049 | **116.070秒** | 176.5 | 98.70% |
| cpp F16／同fixture先頭12秒 | fast12 | **3.343** | 40.125 | 非paced | 231.3 | 100%／380binsのみ |
| rs FP32／2話者・初回 | paced60 | **0.599** | 60.395 | 1.610秒 | 653.1 | **78.49%** |
| rs FP32／同2話者・fresh再実行 | paced60 | **0.544** | 60.396 | 0.805秒 | 616.8 | **78.49%** |
| rs FP32／話者の順序反転 | paced60 | **0.538** | 60.389 | 0.787秒 | 620.6 | 98.67% |
| rs FP32／FLEURS反復 | paced60 | **0.544** | 60.435 | 0.825秒 | 620.8 | 58.53%（RMS正解の限界あり） |

- cpp Q8の15/30/45/60秒時点push遅れ：1.562／23.765／67.192／115.471秒。名目1.04秒bufferがあってもCPUが追いつかない。
- cpp F16の60秒pacedは**500秒timeout、最終reportなし**。0行のログと失敗記録を保持。再試験を12秒prefixへ限定すると40.125秒で完了し、末期のpush約4.54秒。60秒の完了時間／精度は推測しない。
- rsは全60秒caseの最終push遅れ約0、flush約0.39〜0.44秒。first-runの一時的1.61秒遅れも隠さない。
- rsのfirst-visible speaker age：初回中央値1.163／最大2.690秒、同fixture再実行1.103／1.885秒、逆順1.079／1.867秒、FLEURS1.117／1.855秒。
- 無音／440Hz・振幅.05の各12秒：cpp Q8／rs FP32ともlabel／segment0。実noise/music/静かな実speechの検証ではない。
- 共通pure harness12tests PASS。新Rust driverのdurable pathで`cargo build --release --locked`成功。model実推論は上記表の範囲。

## 話者混同の再現・評価の限界

fixtureは既存公開Lingua Libreの「車」（葵心、CC0）／「本」（Fukujinzuke、CC BY 4.0）を4回ずつ交互反復＋500ms無音、60秒でsample切断。FLEURSは既存日本語読み上げ1clipを反復。出典・変換・source hashesは [NVIDIA-DIARIZATION-REALTIME.md](NVIDIA-DIARIZATION-REALTIME.md)、WAV hashesは `evidence/diarization/engine-comparison.json`。

**RMS>.005の10ms binsに限定したsource identity proxyで、人手annotated DERではない。** speakerラベルの全体permutationを許し、missing／重複label／collapseをpenaltyにする。声と単語が完全に対応し、実会議／同時発話／長い再登場の品質に一般化しない。

rs順方向の1850評価bins：話者1は1200bins正しくcluster0、話者2は252bins正しくcluster1、**385binsをcluster0へ混同**、13bins無label。fresh再実行の全raw segmentsと最終frame setsが完全一致して同じ混同を再現した。単なるlabel番号の違いではない。

逆順は1806bins／98.67%、cpp Q8は1850bins／98.70%。逆順成功で順方向の失敗を置換しない。FLEURSの58.53%はRMSのみのspeech正解範囲がbackgroundを含む可能性もあり、そのままspeech loss率へ読み替えない。

rs `feed()`はchunk境界で同じspeaker segmentを分割するため、cpp45 vs rs79というsegment数だけで過分離を判定しない。今回API上の既存非empty frame変更は0だが、source identityの正しさは別。

**未検証**：Qwen同時CPU競合、録音callback／UI／保存接続、日本語自然会議DER、overlap／相槌／遠距離／実noise、multi-hour／話者忘却。速度だけを理由に本体へ採用しない。次は同じ失敗fixtureを保持して混同原因・native／ONNXの一致を確認するか、既選択の保存後処理として評価するか、別途方針を決める。

## 再現・失敗保存

外部checkout／モデル／build／生events／全PCMは `../runtime/diarization-engines/`、条件は `plan.json`。大きなartifactはGitへ追加しない。

以下のcpp buildは**旧汎用対照の再現**。通常のZen3性能検証は診断記録の`build-znver3`＋`--require-simd`を使う。

```bash
# checkoutを上記revisionへpinした上で、cppの必要submoduleだけ取得。
cd ../runtime/diarization-engines/parakeet.cpp
git submodule update --init --depth 1 third_party/ggml
nix-shell -p cmake ninja --run 'cmake -S . -B build-cpu -G Ninja \
 -DCMAKE_BUILD_TYPE=Release -DPARAKEET_SHARED=ON -DPARAKEET_BUILD_SERVER=OFF \
 -DPARAKEET_WITH_CED=OFF -DPARAKEET_WITH_VOICEDETECT=OFF && cmake --build build-cpu -j6'
cd ../../../meetily
nix-shell shell.nix --run 'cargo build --release --locked \
 --manifest-path benchmarks/diarization-engines/rust-probe/Cargo.toml \
 --target-dir ../runtime/diarization-engines/probe-rs/target'
nix-shell shell.nix --run 'python3 -m unittest discover -s scripts -p "test_probe*diarization*.py" -v'
# fixtureは既存公開音声だけ。初回生成済みならhash一致を検査して再利用。
nix-shell shell.nix --run 'python3 scripts/probe_diarization_engines.py \
 --prepare --directory ../runtime/diarization-engines'
nix-shell shell.nix --run 'ORT_DYLIB_PATH=/home/user/Code/meeting-assistant-research/runtime/qwen3-asr/vad-reference-venv/lib/python3.12/site-packages/onnxruntime/capi/libonnxruntime.so.1.23.2 \
 python3 scripts/probe_diarization_engines.py --directory ../runtime/diarization-engines \
 --engine rs --binary ../runtime/diarization-engines/probe-rs/target/release/diar-engine-probe \
 --model ../runtime/diarization-engines/models/rs-f32.onnx --fixture turns --pace \
 --output ../runtime/diarization-engines/results/rs-recheck.json'
# cpp: --engine cpp --library ../runtime/diarization-engines/parakeet.cpp/build-cpu/libparakeet.so
#       --model ../runtime/diarization-engines/models/cpp-q8_0.gguf
```

model URLはHF repo／上記revision／filenameのresolve URL。初期並行downloadは240秒timeoutでrs fileが不完全だったが、resume後にsize/hash検査し、**完全一致後にだけ推論**。不完全fileをモデル不具合に数えない。

ログ `../logs/diar-engines-*`：各clone、cpp-submodule／cpp-build、rs-build／durable-rust-build、model-download／rs-model-resume／cpp-f16-download、fixture-prepare、cpp／rs-silence-smoke、cpp-low-turns／cpp-f16-low-turns（timeout）／cpp-f16-prefix-12、rs-low-turns／rs-low-turns-repeat／rs-low-reverse／rs-low-repeat／rs-low-tone、tests-and-cpp-tone。全試験process終了を確認。表示中appの停止／更新はしない。
