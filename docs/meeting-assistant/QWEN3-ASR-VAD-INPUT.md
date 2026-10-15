# 承認された1・2のみ：公式wrapper parityとVAD入力音量/量子化診断

ユーザーは「1,2だけやって」と明示承認。追加で認識後補正も**考慮する**よう依頼した。今回はVAD-onlyで、ASR推論0、非量子化ASR比較（前提案3）・補正実装・cloud推論・worker/app/model既定変更は行っていない。公開PCM/合成noiseのみ、capture/playbackなし。

## 1. 実際の公式Python/Torch wrapperとRustは一致した

前工程と同じ公式Silero revision `1e261b036686cd0017d500ee96acd1c4ba572a9d` / ONNX SHA256 `1a153a22f4509e292a94e67d6f9b85e8deb25b4988682b7e174c65279d8788e3`。download済みutils_vad.py/weightsのGit blobとSHA256を再照合し、**公式file全体を変更せずimport**してOnnxWrapper.audio_forwardを実行。偽Torch/NumPy shim/別実装のreferenceではない。

- official Python：Torch2.8.0+cpu、NumPy2.2.6、ONNX Runtime1.23.2、force_onnx_cpu=True（実session providerもCPUのみ）。inter/intra1。
- same-runtime Rust：既存current_vad_probe binaryを**同じCPU wheelのC API library**で実行。compile/source/ASRは不変。
- historical Rust：既存Nix ONNX Runtime1.27.1の保存済みprobabilitiesとも別に比較。runtime version差を隠さず記録。
- 既存89+254=343windows（unique WAV341）、gain none/peak20/peak100 → **1023sequences /296,667frames**。
- same wheel：**全frame probabilityがbit同一、最大差0**。
- historical Nix：最大絶対差 **0.0000025033950805664062**、事前固定absolute tolerance1e-5以内。positive .5/.7、minimum >90msの全gate intervalsも一致。
- 独立2passのofficial/Rust raw arraysもそれぞれbyte同一。

これは16k/512new+64context/state[2,1,128]/scalar srのbounded-file契約の確認。8k、連続stream/batch切替、実会議品質やASRの一致は主張しない。前工程のquiet missesは、今回の範囲ではRust wrapperのport差では説明できない。

### reference依存の隔離と初回setup失敗

同じNixでPython Torch/ORTを追加する初案は、triton/LLVM等のsource buildへ入り600sでtimeout。partial logと試したshell sourceはrepo外に保持し、NN parity結果へ混ぜない。代わりに独立`runtime/qwen3-asr/vad-reference-venv`へpublic CPU wheelsを固定した。CUDA Torchを入れず、app/既存ASR venv/locksは変更していない。

`vad-reference-requirements.txt`は実freeze（全17packages）。CPU wheel ORTのhashとNix ORT hash、実version/providerをreportに保存。これにより、runtime差を除いたport比較とruntime差込みの過去evidence照合を分離できた。

## 2. 閾値を固定し、音量補正と追加PCM16量子化を分離した

推論前plan固定：**positive .7 / negative .55 / min speech >90ms**。threshold grid・結果後のretuningはしない。ASRへ変更PCMを渡していない（今回はASR自体を呼ばない）。原WAVを上書きせず、診断float32入力をGit外に保存。

事前に指定した5profiles：

- none
- peak20/peak100/peak300：peak target .1、gain下限1、各上限20/100/300。全fileのpeakを使うため**offline control**。
- slow-rms100：32ms blockのRMS target .02、gain上限100、初期1、上昇time constant1s/下降.05s、現在blockのpeak guard .95。現在/過去blockのみで計算し、低energyを捨てるhard gateなし。live capture/latency試験はしていない。

対象 **575windows ×5profiles =2875sequences**：

1. 全ての既存失敗を含む343known windows：speech288/negative55。
2. 新しい100windows：30FLEURS ×(-50dB /新seed white SNR5 /bursts SNR0)=90speech、synthetic8negative +2silence。seed9182029、tones113/226/452Hz。完全source holdoutではない。
3. 33sources（FLEURS30+既存のいいえ/車/本）×(-40/-60dB)×float32/PCM16 =132量子化対照。**clean source自体は元からPCM16**で、追加attenuation後にもう一度PCM16へ丸めるか、float32で保持するかだけを変えた。float32の録音masterを取り戻した試験ではない。

同一double attenuation値から両branchを生成し、PCM16は従来同様nearest-even rounding。各prepared f32le fileのSHA256/RMS/peakを推論前保存。zeroになったcaseも削除しない。

### presence結果（未検出数、negativeは全profileで0）

|VAD gain|既存speech288|新条件speech90|追加-40dB float32 /33|追加-40dB PCM16 /33|
|---|---:|---:|---:|---:|
|none|23|21|—|—|
|peak20|6|9|3|6|
|peak100|4|8|**0**|**4**|
|peak300|3|9|**0**|**3**|
|slow-rms100|7|9|**0**|**7**|

既存6missの単純なgain100化では、3件を救う一方で別のEN -40dBを新たに落とす（net6→4）。gainは大きければ単調に良くなるわけではない。slow gainの立上りも万能ではなかった。**どのprofileも全speechを救えておらず、今回は採用しない。**

float32 branch -40dBならpeak100で33/33検出、同じsource/attenuationでも追加PCM16 branchは29/33。peak300では33/33 vs30/33。この比較は、音量だけでなく**増幅前の情報損失が検出結果に影響する**ことを示す。高gainで元の波形情報を復元したわけではない。

-60dBではpeak100がfloat32 miss5/33 vs PCM16 miss14/33、peak300が1/33 vs13/33。PCM16 branchの7sourcesは完全zeroになっており、VAD/増幅/認識後補正で元speechを復元できない。ここはlatent sourceにspeechがあった診断labelで、zero-inputに発話検出を強制する受入基準ではない。

両gain passの全2875probability/gain arraysはbyte同一。known343×3既存gain =1029vectorsはstep1 official probabilitiesと完全一致。全入力を保持し、gain上限変更に合わせてscoringを変えていない。

### 実装への含意（未採用）

将来は、captureで得た精度を保ち、音量補正前にわざわざPCM16へ再量子化しない経路を候補にする。ただし**実captureが今回と同じ情報損失を起こすとは未確認**。ADCで既に失われた情報もfloat32変換で戻らない。workerのPCM16契約やapp captureを今は変更しない。synthetic negative65窓の成功をreal noise/会議採用へ読み替えない。

## 認識後補正をどう位置づけるか（考慮のみ）

後処理は有用な候補だが、役割を分ける：

- VAD未検出でASRを呼ばなかった発話は、後補正だけでは復元できない。
- 表記/句読点/固有名詞の補正は**提案として別欄**へ。raw text・原windowへの参照・変更理由・履歴を保持する。VAD probabilityを文字転記のconfidenceとして使わない。
- `いい`→`いいえ`、肯定/否定、数値/日時/担当者/決定の変更は意味を反転させ得る。contextから自然な文章を生成できても、実際に発話された証明にはならない。自動確定せず要確認/未確定を維持する。
- 無音/noiseの`はい。`をLLMが会話として整えてしまう危険もある。語blacklistや全文rewriteで隠さない。
- 補正品質は将来、raw ASR scoreとcorrected scoreを**別々に**評価し、誤った意味変更も数える。referenceや期待語をhintとして与えて試験をgreenにしない。

今回は補正コード/LLM呼出/クラウド送信を追加していない。補正layerや意味変更の自動採用は別途ユーザー判断が必要。

## 再現・証拠

既存pinned Nix shell（worker README参照）内でreference-only venvを使用：

```sh
export UV_CACHE_DIR=/home/user/Code/meeting-assistant-research/runtime/qwen3-asr/vad-reference-cache
export UV_PYTHON_DOWNLOADS=never
uv venv --python "$(command -v python3)" ../runtime/qwen3-asr/vad-reference-venv-new
uv pip install --python ../runtime/qwen3-asr/vad-reference-venv-new/bin/python --index-url https://download.pytorch.org/whl/cpu --no-deps 'torch==2.8.0+cpu'
uv pip install --python ../runtime/qwen3-asr/vad-reference-venv-new/bin/python -r benchmarks/qwen3-asr/vad-reference-requirements.txt
# 以下は成功時に実際に使った既存reference venv。新venvで再現する場合はpath置換。
../runtime/qwen3-asr/vad-reference-venv/bin/python benchmarks/qwen3-asr/reference_current_vad.py ../runtime/qwen3-asr --tag official-vad-parity-r3
../runtime/qwen3-asr/vad-reference-venv/bin/python benchmarks/qwen3-asr/compare_vad_gain.py ../runtime/qwen3-asr --tag vad-gain-r3 --prepare-only
../runtime/qwen3-asr/vad-reference-venv/bin/python benchmarks/qwen3-asr/compare_vad_gain.py ../runtime/qwen3-asr --tag vad-gain-r3 --measure-only
```

runtime：`diagnostics/official-vad-parity-r1/r2`、`vad-gain-r1/r2`。repeat gainはplan/manifestだけfresh dirへコピー、same prepared float pathsを再照合して逐次実行。全original/audio/f32/probability arrays・packages/cacheはGit外。

repo evidence：`evidence/qwen3-asr/worker/vad-input/`。全parity/gain details、frozen plan、compact summary、tests、source/model/probe/runtime/全prepared input hashと不変証明を保存。FLEURS CC-BY4（Conneau et al.2022）、いいえ/Zsrtrgh CC-BY-SA4、車/葵心 CC0、本/Fukujinzuke CC-BY4の出典・metadataは前工程のrobustness/validation evidenceに保持し、今回のfloat/attenuation/normalization派生を記録。

Rust15+helper2/Python18/Node14、306endpoint-reference cases、fmt/compile成功。new Python6はparity/gate差/coverage/pairs/causal gain/zero gain/clipping/quantization-lossを検証。ASR品質の成功を示すtestsではない。既存worker/core/source/modelとapp/locks不変、採用・会議/自然相槌/real noise/live/Windows/長時間受入は引き続き保留。
