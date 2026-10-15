# parakeet.cppのCPU遅延診断（2026-10-14）

> 後続の承認済みQwen同時試験は [CPP-QWEN-CONCURRENCY.md](CPP-QWEN-CONCURRENCY.md)。公開反復60秒で2回ともthroughput追従したが、cppの暫定label遅延は最大約3.3秒へ増加。本稿の「Qwen競合未検証」は単独CPU診断時点の履歴であり、長時間／本体workflow／自然会議の未検証は引き続き残る。

## 結論・前回評価の訂正

**主因は今回のNixビルドでCPU最適化指定が除外されたこと。parakeet.cpp固有のリアルタイム不能を示したものではない。**

前回の`build-cpu`はCMake／Ninja上では`GGML_NATIVE=ON`／`-march=native`だった。しかしNixのGCC wrapperが`NIX_ENFORCE_NO_NATIVE`によりその指定を除外し、実ライブラリのAVX2／FMA／F16Cはすべて0だった。最初のbuildログにもwarningが残っている。設定と実際のコンパイル結果を混同した前回の「cppは不合格、rsだけが速度候補」という評価を訂正する。

元の結果／build／timeoutは削除せず、**汎用x86-64ビルドの対照**として保存する。SSE2等のbaseline／コンパイラ自動vectorizationまで一切ないという意味ではない。

同じrevision・ggml patches・モデルbytes・公開PCM・low preset・CPU4threads・120ms供給を維持し、別ディレクトリ`build-znver3`で明示的に`-march=znver3 -mtune=znver3`を指定。Nixのpurity設定自体は変更しない。

| 条件 | 旧汎用build | 明示Zen3 build |
|---|---:|---:|
| Q8／同60秒paced・計算時間 | 173.675秒 | 32.185秒、fresh再実行33.633秒 |
| 同・処理RTF | 2.895 | **0.536／0.561** |
| 同・paced wall（供給待機を含む） | 180.049秒 | 60.906／60.845秒 |
| 同・最大push完了遅れ | 116.070秒 | 0.739／0.783秒 |
| F16／同12秒fast prefix・計算時間 | 40.122秒 | **1.554秒** |
| F16／同60秒paced | 500秒timeout、reportなし | 計算27.707秒／RTF **0.462**／wall60.696秒 |

Q8は同60秒で約5.2〜5.4倍、F16は**同12秒prefixに限って**約25.8倍高速化。F16の旧60秒timeoutとのspeedupは算出しない。12秒はcacheが短く、RTF0.129をfull60秒／長時間へ外挿しない。

Q8順／再実行／逆順、F16順の各60秒は終了時のpush遅れ約0で供給へ追従。ただし処理はchunk単位にblockし、即時ラベル確定ではない。

## 原因の証拠と実行経路

- `../logs/diar-engines-cpp-build.log`に`Skipping impure flag -march=native because NIX_ENFORCE_NO_NATIVE is set`。今回の`diar-cpp-cpu-diagnosis-build.log`にも同じpreprocessor対照結果を保存。
- 同じGCC wrapperで`g++ -march=native -dM -E -x c++ -`は`__AVX2__`／`__FMA__`／`__F16C__`なし。`-march=znver3`ではすべてあり。
- Nix wrapperの`bin/g++`128〜141行が`-m*=native`を除外。CPU／OSを変更した問題ではない。
- pinned source：top `CMakeLists.txt`36〜44行はnative／llamafileを既定ON。ggml CPUのCMakeはnative時に`-march=native`を渡す。旧cacheのAVX2／FMA／F16C別指定はOFFなので、nativeを除外されると代替指定が残らない。
- **両buildのCMakeCacheにはAVX2/FMA/F16C=OFFのまま**でも、明示`CMAKE_C_FLAGS`／`CMAKE_CXX_FLAGS`が新buildを最適化する。cacheのbooleanだけから機械語を判断しない。
- DLL feature getter：旧SSE3/AVX/AVX2/FMA/F16C=0、新=1。AVX512は両方0、llamafileは両方1。llamafile ONだけでは高速kernelが利用可能とは限らない。
- `third_party/ggml/src/ggml-cpu/llamafile/sgemm.cpp`のQ8 caseはAVX系macroがなければtinyBLAS quantized pathを使用できない。F16でもF16C/vector conversion等の高速経路がcompile条件に依存する。F16Cは半精度変換命令であり、Zen3のnative FP16算術を意味しない。
- owned public-fixture processをGDBで起動し、実際に`ggml_compute_forward_mul_mat → llamafile_sgemm → tinyBLAS_Q0_AVX<block_q8_0,…>::gemm4xN<2>`へ到達。**`vpmaddubsw %ymm3,%ymm5,%ymm3`で停止して1命令実行**を確認。シンボル存在／feature表示だけではない。3 worker＋mainの4threadsも観測。
- GDB log：`../logs/diar-cpp-znver3-gdb-q8.log`。再現script：`benchmarks/diarization-engines/verify-q8-simd.gdb`。pinned GCC/ggmlの内部mangled symbolに依存し、本体ABI／一般的なprofilerではない。全処理の時間配分をsampling profileしたわけではない。

複数の異なるggml buildを同じPython processへ同時ロードしない。共通SONAMEの衝突で別buildを観測し得るので、比較・getter確認は別processで行った。GDBの初回shell指定エラー、未使用dot関数／demangled template名で停止しなかった試行も別ログに保存し、速度計測に混ぜない。

## 速度・label遅延・精度は別

- Q8順：proxy98.70%を維持。旧→最適化後は最終6000bins中4binsのlabel setが異なり、bit-exact出力ではない。最適化後fresh再実行は**raw segments完全一致**。再実行を含め中央値label age1.089〜1.104秒、最大1.819〜1.863秒、RSS約176〜177MiB。
- Q8逆順：RTF0.566、proxy100%、中央値age1.086秒／最大1.861秒。
- F16順60秒：proxy**91.35%**、中央値age1.007秒／最大1.676秒、RSS264.5MiB。12秒prefixの100%を全60秒の品質へ読み替えない。量子化／モデル変換／浮動小数点経路が異なり、「高精度だから必ず話者判定も良い」としない。
- 最適化Q8の人工無音／440Hz tone各12秒はlabel／segment0。
- proxyは声と短語が固定対応したRMS>.005のsource identity評価で、人手DERではない。前回rs順方向の再現可能な混同は未修正。
- 今回CPUでは最適化cppもrsと同程度のthroughput候補。precision／RSS境界が異なるため、表から普遍的なengine順位を断定しない。
- 日本語自然会議、重複発話、長時間記憶、Qwen同時CPU競合、live callback/UI/保存は未検証。**採用・本体統合・リアルタイムworkflow変更ではない。** 保存後手動実行の既選択を維持。

## 再現・再発防止

前回pinned checkoutから、元のbuildを上書きせず別buildを作る（source／model pinは[比較記録](DIARIZATION-ENGINE-COMPARISON.md)）。

```bash
cd ../runtime/diarization-engines/parakeet.cpp
nix-shell -p cmake ninja --run 'cmake -S . -B build-znver3 -G Ninja \
 -DCMAKE_BUILD_TYPE=Release -DPARAKEET_SHARED=ON -DPARAKEET_BUILD_SERVER=OFF \
 -DPARAKEET_WITH_CED=OFF -DPARAKEET_WITH_VOICEDETECT=OFF \
 -DGGML_NATIVE=OFF -DCMAKE_C_FLAGS="-march=znver3 -mtune=znver3" \
 -DCMAKE_CXX_FLAGS="-march=znver3 -mtune=znver3" && cmake --build build-znver3 -j6'
cd ../../../meetily
nix-shell shell.nix --run 'python3 scripts/probe_diarization_engines.py \
 --directory ../runtime/diarization-engines --engine cpp \
 --library ../runtime/diarization-engines/parakeet.cpp/build-znver3/libparakeet.so \
 --model ../runtime/diarization-engines/models/cpp-q8_0.gguf \
 --fixture turns --pace --require-simd \
 --output ../runtime/diarization-engines/results/cpp-znver3-recheck.json'
# Optional actual-instruction check, launches only this owned diagnostic:
nix-shell -p python3 gdb --run 'gdb -batch \
 -x benchmarks/diarization-engines/verify-q8-simd.gdb \
 --args "$(command -v python3)" scripts/probe_diarization_engines.py \
 --directory ../runtime/diarization-engines --engine cpp \
 --library ../runtime/diarization-engines/parakeet.cpp/build-znver3/libparakeet.so \
 --model ../runtime/diarization-engines/models/cpp-q8_0.gguf \
 --fixture turns --limit-seconds 2 \
 --output ../runtime/diarization-engines/gdb-not-a-benchmark.json'
```

`-march=znver3`はこのZen3ホスト用で、portable配布仕様ではない。CUDA/Vulkan/Metal/HIP/BLASは今回もOFF。hostのNix環境変数やglobal compiler設定を変更しない。

harnessにDLLの`compiled_cpu_features`記録と任意`--require-simd`（AVX2/FMA/F16Cを**model load前**に検査）を追加。旧build拒否／新buildのnegative controls成功を確認。歴史的generic controlの再現も必要なのでguardを全runの強制defaultにはしない。

追加pure guard testを含め、比較7＋NVIDIA6＝13tests PASS。再開後に同じ13testsの成功を再確認し、以前の「比較13＋NVIDIA6＝19」という二重計上を元ログに合わせて訂正。機械可読要約：`evidence/diarization/cpp-cpu-diagnosis.json`。raw eventsは`../runtime/diarization-engines/results/cpp-znver3-*.json`、診断／build／testsログは`../logs/diar-cpp-*`。

変更はbenchmark harness／test／GDB script／記録と外部の別buildのみ。Meetily native／frontend／ASR／DB／録音／app lock／表示中app／既存保存音声／OS音声設定は変更しない。新規model取得、実mic、desktop収録、物理再生、cloud推論、app releaseもなし。
