# Qwen3-ASR: 5650GE CPU実測と最適化検証

状態: **独立benchmark**。既存Meetily ASR/録音/provider/schema/lockfileは変更なし。ユーザーはINT8方向を選択し、実CPU最適化の確認と16bit比較を追加依頼。モデルサイズ/本体採用は未決定。実マイク、desktop音声、会議、cloud推論は使用していない。

## 測定条件

- Ryzen 5 PRO 5650GE (Zen 3、6 physical cores / 12 SMT)、NixOS x86_64、約29GiB RAM・swapなし。desktop appsは動いており専用benchmark hostではない。governorは既存の`performance`、THPは既存の`madvise`。OS設定は変更していない。
- 公開Google FLEURS validation先頭10件/言語: 日本語138.66秒/525正規化文字、英語80秒/175語、タイ語100.5秒/1002文字。合計319.16秒。Conneau et al. (2022), CC-BY-4.0。API snapshotのdataset Git revisionは取得できず、row ID、原文、各音声SHA256を固定。Thaiはvalidation Parquetの同じ先頭10行から抽出。
- CPU-only、モデルprocessは順番に1つだけ。loadとuntimed warm-upをRTFから除外。精度は各音声1回、2/4/8秒prefixは各言語3回、pacedは各言語1clipのみ。18GiB address-space cap。
- NFKC/lowercase/Unicode punctuation・whitespace除去のCER、日本語/タイ語はcodepoint、英語WERは空白語。数字・同義語・意味の補正なし。無音はspeech CER/RTFから分離しraw出力を保持。prefixは精度採点しない。
- 下表は**修正済みLP64 OpenBLAS・6 threads共通**。RTFは音声時間で加重した全言語合算。RSSはprocess high-water mark（load/warm-up込み）、モデルサイズ/常駐RAM一般値ではない。

## 16bitとINT8（確定したfixture結果）

| 保存/実行形態 | サイズ | 日本語CER | 英語WER | タイ語CER | 合算RTF | 最大RSS GiB |
|---|---:|---:|---:|---:|---:|---:|
| C、非量子化BF16/F32 | 0.6B | 8.00% | 6.86% | 6.09% | 0.278 | 2.71 |
| Rust、decoder INT8・encoder/prefill F32 | 0.6B | 9.14% | 6.86% | 5.49% | 0.186 | 3.30 |
| C、非量子化BF16/F32 | 1.7B | 3.05% | 5.14% | 6.09% | 0.610 | 6.62 |
| Rust、decoder INT8・encoder/prefill F32 | 1.7B | 3.05% | 5.14% | 6.49% | 0.419 | 7.30 |

言語別RTF (JA/EN/TH): BF16 0.6B **0.219/0.232/0.395**、INT8 0.6B **0.164/0.171/0.228**。1.7Bはevidenceのscorer groupsを参照。

- 16bitは**公式のBF16重み**。CPU runtimeはF32へ展開/蓄積する。5650GEにはBF16/VNNI/FP16のネイティブdot演算がない。これは全層16bit演算やFP16の測定ではない。
- Rust INT8はdecoderの重みとsingle-token matvec activationを8bit化し、INT32で蓄積、F32 scaleを適用。encoder/prefill、attention/normalisation等はF32。全モデルINT8ではない。
- CとRustのruntime実装も違うため、表の差を量子化**だけ**の因果効果とはしない。各runtimeの現在の実用候補の比較。
- INT8でもメモリは減らなかった。BF16 maps・F32 prefill cache等を持つため、decoderのbit数だけで総RSSを予測しない。
- 初期のCandle F32 pilotsはCより遅く高RSSだった。C++ mixed-Q4は速いが精度が下がり、converterの原BF16 revisionも未公開。ユーザーのINT8選択を勝手にQ4へ変更しない。

## 実CPU向け最適化の検証

`target-cpu=native`だけを根拠にしない。

1. **ISAとbuild:** corrected Rust `RUSTFLAGS="-C target-cpu=znver3"`、release opt-level=3、thin LTO、codegen-units=1。Cも`-O3 -march=znver3`。Nixが初期Cの`-march=native`を除去したため明示targetへ修正している。AVX2/FMAのCPU/compile featuresを確認。host-tuned binaryを他CPUへそのまま配布しない。
2. **INT8の実際の命令:** 上流`kernels/avx.rs`の経路はAVX2でsigned i8をi16へ拡張し、`vpmaddwd`でINT32 dotを行う。非飽和のexact dot。新binaryの対象関数をdisassembleして`vpmovsxbw`/`vpmaddwd`を確認。
3. **実推論で使用:** GDBで公開Japanese fixtureのASR warm-upを実行し、`matvec_int8`内の`vpmaddwd`にtemporary breakpointを置いた。実際にhitし1命令stepした。main+5 Rust workersがこの関数、5 OpenMP workersがbarrier待ちのsnapshotを取得。sourceが存在するだけ/未使用symbolだけという確認ではない。GDB実行は診断のみ、途中終了するのでRTF/精度には使わない。
4. **数値検証:** 同じznver3/LP64 release buildでupstream lib tests **32/32成功**。INT8 matvec/argmaxのscalar-referenceテストも含む。これもモデル意味品質の保証ではない。
5. **BLAS:** actual binaryがlinkするOpenBLAS 0.3.33を照会し、`DYNAMIC_ARCH NO_AFFINITY USE_OPENMP Zen MAX_THREADS=64`、runtime core **Zen**、実thread count **6**を確認。C headerの`sizeof(blasint)`も4 byte。Zen familyのAVX2 kernelを選択しており、AVX512/BF16ハードウェアを誤って前提にしていない。
6. **thread/tuning:** INT8は1/3/4/5/6/8/12 threads、C BF16は3/6/12をscreen。6は3/12より速いが、0.6Bの5/6は近く、追加の交互順比較では5が同等～数%速くCPU時間も低い。1.7Bは交互比較で6が5より約1.6%速い。CPU affinityを0..5へ固定すると両サイズで遅くなり採用しない。強制OMP `ACTIVE`はCPU消費を増やし、`PASSIVE`は0.6Bで遅く1.7Bでほぼ同速。盲目的なpinning/最大thread/ACTIVE指定はしない。詳細はevidenceのpilot inference/CPU seconds。

5 threadsの全30件再測定も完了: 0.6BはJA CER **8.38%** / EN WER **8.57%** / TH CER **5.99%**、RTF **0.181**。1.7Bは**2.86% / 5.14% / 6.19%**、RTF **0.439**。thread数を変えると出力も変わったため、速さだけで最終qualityが同じと推定しない（浮動小数点経路等の影響は未切り分け）。同じ5/6設定のpilot各clipは交互runを合わせた6回で同一出力だったが、全30件/長会議の再現性検証ではない。比較の主表は6 threads共通を保つ。

結論: **INT8 AVX2の実使用、Zen BLASのruntime選択、ABI、thread挙動を確認した、5650GEに適合するCPU経路**。ただし全探索で世界最速/理論ピークに達したという結論ではない。scalar/unit/fixture testsからWindowsや長会議を推測しない。

### 発見・修正したABI不一致

Nix x86_64のOpenBLASデフォルトは`USE64BITINT` (ILP64)。一方、上流Rust CBLAS FFIは`m/n/k/lda/ldb/ldc: i32` (LP64)。観測上動いていてもABIとして不正。

- 初期Rust数値を`results/exploratory/invalid-blas-abi/`へ保存し、推奨・表から除外。以前伝えたINT8 RTF 0.156/0.371は確定値として使わない。
- benchmark shellを`pkgs.openblas.override { blas64=false; }`へ変更し再ビルド・全比較を再実行。
- runnerは`openblas_get_config()`に`USE64BITINT`があれば**モデルload前に拒否**。実際にold ILP64 libを優先loadして拒否する負のテストも成功。
- 旧CはC headerの整数型とlibraryが一致しておりABI違反ではなかった。ただし新表はCもLP64へ再ビルドし同じ環境で測った。旧記録は`exploratory/c-ilp64-header-correct/`。

## 低遅延と終了時の訂正

2秒刻みの公開音声をwall clockで到着させる。取得待ちとcomputeを分離。6 threads、各言語1clipに限る。

| Rust INT8 | 初稿 (reconcile run) | prefix callback | 入力終了後、全発話再認識で確定するまで |
|---|---:|---:|---:|
| 0.6B | 2.65–2.67秒 | 7.11–7.35秒 | 1.85–3.09秒 |
| 1.7B | 3.58–3.86秒 | 8.74–9.56秒 | 6.03–10.29秒 |

- prefix callbackは「不変で正しい会議根拠」を意味しない。初期Japanese `海外`→`海岸`の固定誤り等を観測したため、utterance終端で独立offline認識を行い草案を訂正する比較を追加。
- 1.7Bはoffline RTF<1でもincremental再計算がリアルタイムを上回る例がある。無制限に累積会議全文を再処理してはならない。VAD/発話window/queue上限と長時間試験が必要。
- intermediate 32-token budgetをfinalにも使うとThai末尾が欠落。finalを512へ拡張して再実行。失敗を`exploratory/stream-final-32/`へ保存。512もあらゆる長文のEOS保証ではない。
- direct無音5秒ではBF16 0.6B `ええ。`、他の今回3構成 `はい。`。VADによる抑制は未検証。品質採点で無音を隠さない。

## 原型/採用の境界

現時点で本体には接続しない。モデルサイズを決めた後、独立Rust workerでbounded input/queue、request IDs、cancel/timeout/restart、model常駐、draft/utterance-final分離を検証する。2時間、Windows、混在言語、会話/noise、speaker identity、終了tail、手編集保護、録音復旧は別のacceptance。日本語CER3.05%の読上げ試験は会議の意味保証ではない。

## 再現と証拠

ソース/engine pin/model取得: [../../benchmarks/qwen3-asr/PLAN.md](../../benchmarks/qwen3-asr/PLAN.md)。FLEURS/モデル/third-party engine/buildはrepo外。公開音声の再生もしていない。

```sh
cd meetily
nix-shell -I nixpkgs=/nix/store/8vdpwi2yls679v4dnjcp3wynsq77jwhl-source benchmarks/qwen3-asr/shell.nix
export RUSTFLAGS="-C target-cpu=znver3"
export CARGO_TARGET_DIR=/home/user/Code/meeting-assistant-research/runtime/qwen3-asr/native-lp64-target
cargo build --release --locked --manifest-path benchmarks/qwen3-asr/native-runner/Cargo.toml
cargo test --release --locked --manifest-path ../runtime/qwen3-asr/rust-native/crates/qwen-asr/Cargo.toml -p qwen-asr --no-default-features --features blas --lib -- --test-threads=1
python3 benchmarks/qwen3-asr/run.py ../runtime/qwen3-asr --engine native --size 1.7B --threads 6 --suite accuracy
python3 benchmarks/qwen3-asr/cpu_probe.py ../runtime/qwen3-asr
python3 benchmarks/qwen3-asr/summarize.py ../runtime/qwen3-asr
node --test benchmarks/qwen3-asr/test-metrics.mjs
# GDB diagnostic: gdb/python3/binutils required, no other model process running.
OPENBLAS_NUM_THREADS=6 OMP_NUM_THREADS=6 python3 benchmarks/qwen3-asr/gdb_probe.py ../runtime/qwen3-asr
```

主要logs (repo外 `../logs/`): `qwen-lp64-validated-comparison.log`, `qwen-5650ge-optimization-screen.log`, `qwen-thread-confirmation.log`, `qwen-runtime-int8-gdb.log`, `qwen-five-thread-confirmed-accuracy.log`, `qwen-ilp64-rejection.stderr`。public-only summary/CPU evidenceは `evidence/qwen3-asr/`。一次JSONL、各run command、SHA256、stderr、assembly全文は `../runtime/qwen3-asr/` に保持。
