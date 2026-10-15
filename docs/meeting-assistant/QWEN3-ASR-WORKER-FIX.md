# 独立workerのVAD/context修正（2026-10-12）

ユーザーの「修正して」で、調査後のrepairを実施。範囲は独立Linux bounded-file workerのみ。Meetily本体の録音/ASR/VAD/provider/DB/既存locks、モデル選択、cloud資格情報は不変。capture/playbackなし。public FLEURS/合成音声のみローカルCPUで逐次処理。

## 変更

1. **実際のneural forwardを512 samples/32msに固定。** `silero_rs::VadSession.process()`は内部480へ再分割するので使わず、pinned same v4 ONNXの`forward()`を呼ぶ。単にcallerのchunk長を変える修正ではない。外部cache/engineにパッチを当てず、Cargo.lock/重みも不変。
2. `worker/src/vad.rs`：official v4 trigger/temp-end hysteresisをsample座標でport。正threshold .50/負 .35、min speech >90ms、500ms silent hold、前300ms/後200msをworker設定として明示。asymmetric padが重なればmerge。公式Pythonの全default設定と同一ではない。MIT noticeを`worker/licenses/Silero-MIT.txt`に保持。
3. new neural state per request。末尾はVADの最後の512frameのみzero-pad、原sample endpointへclamp。ASRのPCMは変えない。VAD-only最大20倍gainは維持し、RMS-onlyによる発話判定/強制fallback/語のblacklistはない。
4. **defaultはpresence-only full bounded input。** 未検出→ASR0/空文字/no flush。発話があればoriginal `[0, real_len)`を一回認識（30秒上限）。内部pauseと前後文脈を削らない。新`--crop-context`は同じ修正VAD/workerでfirst-to-last gate envelopeだけ渡す診断mode。ready/result metadataで`vad_frame_samples:512`と`recognition_context`を識別。
5. 短区間reviewはfull input durationでなくgate evidenceで判定。原text保持、confirmed=false、request/response ID、single-flight、timeout/kill/restart、model常駐/fresh recognition session、LP64 fail-closedは維持。

これは会議を30秒ごとに必ず切る運用policyではない。file prototypeの入力上限で、live segmentation/低遅延draft/Windowsは別工程。

## 同一workerで全30clip比較（raw採点、不正な除外なし）

|方式|JA CER|EN WER|TH CER|無音5s|
|---|---:|---:|---:|---|
|corrected original ungated 6threads baseline|3.05%|5.14%|6.49%|`はい。`|
|旧480 + crop envelope worker（履歴）|2.86%|6.29%|10.48%|空/ASR0|
|**修正512 + full original PCM（default）**|**3.05%**|**5.14%**|**6.49%**|**空/ASR0**|
|修正512 + diagnostic crop envelope|2.48%|5.71%|10.18%|空/ASR0|

**修正defaultの発話30件はbaselineのtextと全件文字列一致。** clippingによるoutput/number-rendering regressionはこの固定corpusで解消。Geometryを直すだけでcrop問題まで治るわけではない。rawCER/WERの定義変更/数詞一括正規化/レビュー区間削除はしていない。旧Thai悪化を全部発話欠落と解釈しない（調査reportの分類を維持）。同じ結果の1passは全入力のdeterminism/会議意味品質を保証しない。

推論時間はload/VADを除きfresh session allocationを含むprototype診断。formal warm-up benchmark/latency claimではなく、速度改善を主張しない。RSSは未測定。二つのmodel workerは逐次実行。reference-test bridgeの短いcompile/checkはモデル品質比較に関係しないが、正式performance測定では分離が必要。

## fixture / unit / reference確認

- Rust15 tests、supervisor4、metrics5成功。Rustfmt/Python compile成功。
- official v4 trigger/temp-end scalar referenceとの**306** seeded/edge probability列比較成功。worker設定のmin/pad/mergeをreferenceにも明示した。これはsynthetic endpoint parityで、実音声ground truthではない。
- 無音（発話前/後）/white noise4水準/50&440Hz計8negative：**空/ASR0**。最初のnegativeではASRモデルloadなし。最後にさらに無音を送ってcarryなし確認。
- 原日本語/前後silence追加/-20dB/-40dB/30秒窓の長いsilence tail：この1clipのreference sentenceを保持（正規化比較はfixture限定。上のcorpusはraw text同一比較）。長tail caseは原11.1秒+18.9秒zeroで入力480000sample、末尾contextを切らず認識。
- 旧workerが未検出だった500ms cropは**検出・ASR1**、`この場合。`としてraw保持/short_speech_uncertain。未整列cropで正解transcriptは未確定、真の「はい」の品質合格とはしない。
- 元128001sample（512の倍数でない）のEOFをASRまで完全保持。入力途中でcutした語音のtext復元は保証しない。

## 残る限界

**後続評価で追加品質不合格を確認。** 合成burst noiseの`はい。`誤生成、quiet/noisy ENの未検出、短語`いいえ`の無音padによる`家。`への変化を再現した。[QWEN3-ASR-ROBUSTNESS.md](QWEN3-ASR-ROBUSTNESS.md)が最新の採用判断。このページの「回帰合格」は元の限定fixtureに限る。

純無音抑制と限定corpusの精度保護は確認したが、VAD誤検出/quiet speechの未検出/背景音中の短相槌/認識モデルの実音声hallucinationは残り得る。presence-onlyはspeechを含むwindowのnoiseを除去しない。自然音でreference外の「はい」が出ても、実発話かどうか別確認なしに削除しない。LLMに補正させて根拠を捏造しない。

**worker修正fixtureの回帰は合格、本体採用・会議品質受入は未達。** 本物の相槌・会話/混在language/noise、queue/時間窓/終了遅延、Windows、二時間運転を追加評価してから接続する。

## 証拠・コマンド

コード/手順：`benchmarks/qwen3-asr/worker/README.md`。current public raw output/report：`evidence/qwen3-asr/worker/fixed/`。過去の失敗は既存evidence/外部exploratoryに残した。外部logs：`qwen-worker-512-fix-acceptance.log`、`qwen-worker-512-extended-fixtures.log`、`qwen-worker-512-reference.log`、final static log。

`evaluate_worker.py --tag fixed-full`と`--tag fixed-crop --crop-context`を同じbinaryで逐次実行し、既存metrics.mjsで各31rowsを採点。labels/modeが不一致なら拒否する。重み/PCM hashesは既存provenance/manifestに固定。新artifact名で履歴を上書きしていない。

handoff後の最終再確認：`cargo fmt --check`、Rust15/Python4/metrics5、306 endpoint reference casesを再実行して成功（`../logs/qwen-worker-handoff-verification.log`）。保存済みevidence/source/binaryのSHA256をprovenanceと照合し、全31rowsのmetadataとspeech30件のbaseline raw text一致も再確認した。この再確認ではASR全corpusの再推論は行っていない。
