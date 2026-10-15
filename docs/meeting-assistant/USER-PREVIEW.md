# ユーザー操作用：合成音声の隔離プレビュー

ユーザーが「いったん完成形をこちらで確かめたい」と依頼し、ask_userで **合成音声で操作確認** を選択。実mic/desktop/cloud/authの許可には拡張しない。実用版の完成候補を触るためのdev native起動であり、package／品質／完成全体の受入ではない。

## 今回の起動

`scripts/preview_synthetic.py` をNix shellからbackground起動。Xvfbではなくユーザー画面 `DISPLAY=:0` のXWaylandに実native appを表示。新規private HOME/XDG、private D-Bus、ALSA card999、hardware moduleのないprivate Pulse null sinkで初期化。実1.7B decoder-INT8／明示en、既存model/ORT pathsを隔離profileだけに設定。通常のユーザー設定・DB・認証はコピー／変更しない。

- 録音は**ユーザーがStartを押したときだけ**。helperは公開is_recordingを監視し、Start後2秒で8.656秒のeSpeak英文をprivate sinkへ一度だけ再生。Start/Stopやユーザー編集は自動クリックしない。物理speakerへ音は出ない。
- 新規生成の人工transcript／人工minutesを初期登録し、Meeting Detailsへ表示。これらはAI生成結果ではなく、編集・ID／根拠・履歴・JSON／Markdownを触るためのfixture。
- 録音時のASRは**本物のローカルQwen**。fixture応答やLLM/cloud生成を使わない。Generate Summary／継続LLM更新／ログインの確認は今回に含まない。
- 初期登録と設定のみWebDriver公開IPC／navigationで準備。以後ユーザーのphysical操作は未検証であり、ユーザー確認を待つ。SPA navigation/reload中の一時IPC切断ではhelperはアプリを閉じない。
- アプリ終了時、owned helper/driver/Pulse/D-Busだけ終了。録音中に強制終了する保存保証はないので、**Stop→保存完了を待ってから閉じる**。profile・Downloads・音声・DBは削除しない。

log：`../logs/meetily-user-preview.log`。最初のvisible起動は `../runtime/preview-gui-iqz33qyj`、READY／private monitor一つのみ／表示された人工transcriptとsaved minutes disclosure／screenshotを確認。アプリを開いただけで録音開始していない。初回版のminutes disclosureは閉じた状態、後続script版は初期生成時だけ開く。ユーザー操作の成功や新しい録音結果は未報告。Python compile／CLI help／git diff checkでscriptを確認。

## 操作

1. Meeting Detailsの **保存済み継続議事録（ローカル閲覧・編集・出力）** を開く。人工項目の編集→保存してAI更新から保護→JSON/Markdown出力を試す。未保存の変更を捨てず保存してから移動する。
2. HomeからStart recording。合成英文がprivate入力へ自動で一度再生される（speakerから聞こえなくて正常）。約20秒後にStopし、認識／保存の終了を待つ。実Qwenのraw出力を同meetingで確認する。初回loadやStop drainには待ち時間があり、UIのASR errorを隠して成功扱いしない。
3. Stop／保存完了後に閉じ、同profileを `--profile` で再起動すれば保存済みデータを再閲覧できる。人工minutesの元fixtureはresumeで再生成／上書きしない。同一profileの二重起動は拒否する。

## その後のユーザー確認／standalone候補

ユーザーから「問題なさそう」との操作確認報告。その後の続行で、まずNixOS向けの開発サーバー不要候補を選択。`scripts/preview_synthetic.py` のnavigationは現在のoriginを使うよう変更し、embedded frontendからdev URLへ戻らない。nix-shell／Nextなしのpackaged synthetic launcher、10分試験の結果と残る警告は [LINUX-STABILITY-PACKAGING.md](LINUX-STABILITY-PACKAGING.md)。下記は旧dev起動の手順として保持する。ユーザーの報告を全機能・自然会議／長時間の品質受入と解釈しない。

## 起動／再開

Next dev server `http://localhost:3118` とREADME記載のnative sidecars/bootstrapが必要。準備済weightsを使い、scriptはdownloadしない。repo rootのNix shellで：

```sh
python3 scripts/preview_synthetic.py \
  --binary ../murmur/target/debug/meetily \
  --bootstrap ../murmur/target/debug/examples/gui_fixture_bootstrap \
  --driver "$(command -v WebKitWebDriver)" --espeak "$(command -v espeak-ng)" \
  --runtime ../runtime --display :0 \
  --worker ../runtime/qwen3-asr/worker-target/release/qwen-speech-worker \
  --model ../runtime/qwen3-asr/artifacts/model-1.7B \
  --ort /nix/store/x46rnziynpalr4phm6yh0x46s42xq5kw-onnxruntime-1.27.1/lib/libonnxruntime.so
# 再開時のみ上のcommandに --profile ../runtime/preview-gui-iqz33qyj を追加。
```

再生を追加したい場合、起動中の同profile内 `play-synthetic.sh` を実行できる（private sink限定）。それ自体では録音を開始しない。ログインやクラウド／他アプリ音声／実マイクをこの試験の既定として追加しない。
