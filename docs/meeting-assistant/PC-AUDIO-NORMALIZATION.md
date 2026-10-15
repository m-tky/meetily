# PC音声：上限付き自動正規化

## 合意・変更範囲

ユーザー「PC音声についても、正規化しておいて」。方式選択で**上限付き自動正規化**を明示選択：目標−23 LUFS／増幅最大＋20dB／滑らかなgain変化／無音保全／sample peak制限。マイクの既存処理は維持。新しいPC captureは混合前に調整され、**保存音声とASR入力の両方**に反映する。既存録音／OS speaker volume／model／VAD／resamplerは変更しない。

検証は直前の承認どおり、以前の日本語公開fixture＋private仮想入力で自動実行。実mic／physical speaker／通常profile／実desktop音／cloud／authは使用しない。releaseを作成しない。

## 実装

- 新 `frontend/src-tauri/src/audio/pc_normalizer.rs` の `PcLoudnessNormalizer`。PC sourceごとのstateを保持。
- mono48k capture後、mix前に適用。EBU R128 **momentary** loudness（400ms窓）を100ms入力blockごとに取得。累積global loudnessをsampleごとに計算する方式ではない。
- gain targetは−23 LUFS、最大＋20dB＝10倍、減衰下限−40dB。sampleごとに300ms指数平滑を行う。callback chunk sizeが変わっても同一sample列からbit-exactな出力。
- 分析block RMSが1e-4（−80dBFS）以下、またはloudnessが未確定／非有限ならtarget gainを1へ戻す。これはspeech/noise分類器ではなく、noise抑制の保証はない。声から無音への切替ではgainは滑らかに戻るが、zero samplesは常にzeroで、無音を音にしない。
- −1dBFSの**sample peak**（0.8912509）へclamp。reconstructed true peakやcodec overshootの保証ではない。mix後の既存hard saturationを直す変更でもない。
- lookahead／audio delay bufferなし。入力N samples→出力N samples、短い末尾も即出力。analysis bufferの残りは計測用であって未出力audioではない。非有限inputはzeroへsanitize。
- `pipeline.rs`はPC用normalizerをmic用 `LoudnessNormalizer`とは別fieldで保持。PCへHPF／RNNoiseは適用しない。初期化失敗はnative warning＋既存error通知で知らせる。micの既存HPF＋normalizer／RNNoise flagは変更しない。

**目標値を全音声に厳密達成する保証はない**。初期meter／gain平滑／上限／peak制限／極小信号保護があり、sourceが小さすぎれば＋20dBでも目標に届かない。すでに失われた音声／歪み／入力同期問題を修復しない。

## ネイティブ検証

`cargo test --locked --offline -p meetily --lib` filter：

- `audio::pc_normalizer::tests::` **10PASS**：silence/empty、quiet tone上限20dB、定常toneでtarget LUFS±0.5、loud tone減衰、transient sample peak、低noise上方gain禁止、chunking bit equality、非有限保護、sample count/zero tail、sample smoothing。
- `audio::pipeline::tests::` **12PASS**：既存sync/Stop/VAD回帰10＋new2。fake System `AudioCapture.process_audio_data`が実PC normalizer参照とexact一致、mic HPF／legacy normalizerの分離保持を確認。実deviceをopenしない。

log `../logs/meetily-pc-normalization-native-tests.log`。10＋12＝**22PASS**。実mic／認識品質／Windows確認ではない。

## ローカルdebug候補

embedded custom-protocol debug/no-bundle build＋Nix stage成功。

- GC-root `../runtime/nixos-pc-normalized-candidate` → `/nix/store/bvppjldsd7x01km16fjxfh1awbsjkkm2-meetily-local-candidate`。
- new inputsは `../runtime/pc-normalized-build-*/inputs`、path pointer `pc-normalized-build-path`。
- candidateのnative SHA256はbuild inputと一致。元dev binaryはtrap／atomic renameで復元しhash一致。以前の候補は保持。
- frontend rebuild／normal profile／visible appの終了・再起動／releaseなし。

logs `meetily-pc-normalized-candidate-build.log`／`-stage.log`、build dir内 `verification.json`。

**ユーザーが開いている旧確認画面は未更新**。この候補はprivate自動検証で使用したもの。既知のdegraded復帰bugは別件として残り、今回そのロジックを変更しない。

## 日本語公開fixture＋仮想音声での確認

`test_public_virtual_audio.py`へ `--pc-db`（0または−20）と `--expect-normalization`を追加。sourceは固定11.1秒FLEURS。−20dB版はFFmpegでdecode後floatへ0.1倍してraw float32としてprivate sinkへ流し、追加PCM16量子化／speaker gain変更をしない。元fixtureのhashは不変。

最終3runsはいずれもprivate null sinkのみ、local Qwen／ja、native初期化normalizer有効、ASR nonempty、save/completion/drain、event＝raw＝SQLite本文一致、decode warning0、temp input／checkpoint cleanup、stall0／late discard0／error events0、owned helpers終了、passed=true：

| input | profile | saved segments | saved duration | decoded RMS |
|---|---|---:|---:|---:|
| PCのみ／元音量 | public-virtual-aniztz7k | 1 | 11.681438s | 0.082402 |
| PCのみ／−20dB | public-virtual-sotb9y3u | 2 | 11.894813s | 0.052689 |
| 仮想dual（micはzero）／PC−20dB | public-virtual-asl4ictt | 2 | 11.994688s | 0.052589 |

logs `meetily-pc-normalized-public-original.log`／`-quiet.log`／`-dual-quiet.log`。録音内容／本文／native log行はtoolへ出さずaggregate/boolのみ。

最初の3runsは認識・保存を完了したが、harnessがrecording-state marker以降だけからnormalizer初期化messageを探してfalseになった。capture初期化messageはそのmarkerより前に出る。fresh一session profileの全logで初期化を確認するようharnessだけ修正し、**新規3runsを再実行して成功**。元のfalse resultは `*-init-boundary-failure.log` と初回profilesに保持。DSPをgreenにするため再調整していない。

これはquiet fixtureでも認識・保存が動くという限定確認で、CER／意味精度の改善や全level／noise／実dual/AECの合格ではない。sourceは同じでもsegment数1/2が変わり、前の認識とexact同一ではない。今回も復帰bugを誘発するcallback順は発生しなかった。

再現は [VIRTUAL-PUBLIC-CHECK.md](VIRTUAL-PUBLIC-CHECK.md) のcommandのbinary／workerを新rootへ変え、`--expect-normalization`、quietは`--pc-db -20`、dualは`--dual`を追加。空きDISPLAY/portだけを使い、real microphone input／normal appへ接続しない。
