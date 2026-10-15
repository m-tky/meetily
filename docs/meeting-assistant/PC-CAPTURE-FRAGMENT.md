# PC取得単位100msの指定・隔離検証

後続：ユーザーがUI成功を報告し、遅延計画を承認。ADJUST_LATENCY比較でPC配送遅延を改善したが、本体dualには別の起動時Mic供給差が残る。最新候補／失敗結果／残作業は[DUAL-LATENCY-REPAIR.md](DUAL-LATENCY-REPAIR.md)。以下はfragment-only段階の記録。

## 決定と実装

引継ぎ後、ユーザーはask_userで「100ms指定を実装・検証」を明示選択。実mic・physical playback・表示中previewには触れず、公開FLEURS＋private仮想入力だけを使用。

`audio/capture/pulse.rs`に公開C ABIの`pa_buffer_attr`（5×u32）を追加し、mono float32／48kHzの`fragsize=19200bytes`（100ms）を`pa_stream_connect_record`へ渡す。maxlength／tlength／prebuf／minreqはUINT32_MAXでserver既定を維持。既存10秒read安全上限／ring上限／DONT_MOVE／no fallback／1.2秒wait／Mic処理／PC正規化／ASR／VAD／resamplerは変更なし。

**ADJUST_LATENCYは指定しない。** source／OSのlatency設定を変更せず、配送fragmentだけを要求する。参考はlocal libpulse17 public headers `pulse/def.h`のpa_buffer_attr説明：flagなしfragsizeはsource latencyを変えない。serverは要求をclamp可能。READY後のnegotiated fragment／maxlengthを数値だけnative logへ記録する。

## 回帰と候補

- native Pulse3＋mixing14＋pipeline12＋normalizer10＋VAD7＝46PASS。新testは100ms bytes、残り属性UINT32_MAX、ABI20bytes／align4／fragsize offset16を検証。
- source regression18PASS＋既定DSP2expectedFAILを保持。exit0は既知のsaturation／alias failure再現を含む、品質合格ではない。
- Python構文／rustfmt check／git diff --check成功。最初のsource regression commandはNix shell外のpython3不在で失敗し、`source-regressions-no-shell.log`へ保持してshell内再実行。
- embedded debug／no-bundle候補：`../runtime/nixos-input-fragment-candidate` → `/nix/store/5whvrynpmdk77mvcz35kp9hnwpvc8vvw-meetily-local-candidate`。
- inputs：`../runtime/input-fragment-build-Kb3cFS/inputs`。UI static exportは既存隔離`input-ui-build-9kXlUb/frontend/out`を使用。built input／stage equalityとdev binary復元equalityを確認。通常`.next`／`out`、旧候補、表示中previewは未変更。

## 公開音声／UI end-to-end結果

固定日本語11.1秒FLEURSを−20dB float32にし、private Pulse `-n`／null sinks／fresh HOME/XDG/SQLite/DBus/Xvfbで自動DOM選択・保存・browser reload・Start/Stop・native reopen。Dual micはprivate zero信号のみ。再生latency設定は既定のまま（50ms controlで合格を作らない）。

| run | ASR segments | 保存秒 | stall | late samples | sampled PC buffer latency |
|---|---:|---:|---:|---:|---:|
| PC-only | 1 | 12.41925 | 0 | 0 | 1.621125秒 |
| dual | 2 | 12.197625 | 0 | 0 | 1.621104秒 |
| 独立dual repeat1 | 2 | 11.946625 | 0 | 0 | 1.621125秒 |
| 独立dual repeat2 | 1 | 12.01025 | 0 | 0 | 1.621125秒 |

全4runsでrequested／negotiated fragmentは19200bytes、negotiated maxlength4194304bytes。UI設定保持・録音中lock・native再起動、ASR非空、event/raw/SQLite一致、completion、有限非zero音声／decode warning0、temporary input／checkpoint cleanup、owned helpers終了が成功。error eventsは全0。

**全体判定は4runsともFAIL（exit1）を保持。** 新`--expect-fragment-100ms`診断はfragment一致に加え、再生2秒後のPC buffer latencyが1.2秒未満を要求したが、数値は約1.62秒のまま。テストを緩めてPASSにはしない。従来判定の個別指標も保存し、今回の診断値を隠さない。

配送fragment指定／この3回のdualで警告なしは確認できたが、latency表示値の低下・全失敗原因の証明・hardware timestamp同期・全fixture保持・意味精度・実二入力／長時間の安定受入は未確認。buffer latencyの単一snapshotはcallback間隔や全end-to-end latencyを直接証明しない。Source latency表示は0。source latency調整flag／wait延長／最大buffer変更は別decision gate。

ログprefix：`../logs/meetily-input-fragment-`の`native-tests`、`source-regressions[-no-shell]`、`candidate-build/stage`、`public-single`、`public-dual`、`public-dual-repeat1/2`。内容はaggregate／booleanだけtoolsへ出し、音声／transcript／native log本文はexportしない。

## 表示への反映

この後ユーザーが「保存済み・閉じたので切り替えてよい」を明示選択。既存helperのready=false／owner不在を確認し、同じ`dual-preview-spocggm1`／:0／4453へ新候補を反映した。

最初の起動は選択済み`EchoCancel_Speaker.monitor`非列挙で安全停止。失敗ログ`preview-resume-missing-device`を保持。OS metadataではEchoCancel_Mic／EchoCancel_Speaker.monitor双方が不在、別sourceやdefaultへ自動変更しない。`preview_meeting_inputs.py --show-missing-devices`という明示UI-onlyモードを追加し、選択IDを保持したままmissing表示＋Start disabledをDOMで確認して初めてREADYとする。既存strict modeの機器必須assertは維持。

再起動は新binary／embedded origin／missing-device previewを確認してREADY。owned host capture streams0、recording=false（helper assert）、自動録音／再生なし。モデル／言語／入力IDは従来のまま。UIで機器が見つからない表示が出るため、OS側の機器復帰またはユーザーによる明示選択が必要。OSの音声構成／default／volumeを変更してはいない。release作成なし。

追加ログ：`meetily-input-fragment-preview-{resume-missing-device,resume,audit}.log`。auditはaggregate bool／countのみ、既存録音・transcript／native log本文をtoolsへ出していない。Python compile／diff check成功。
