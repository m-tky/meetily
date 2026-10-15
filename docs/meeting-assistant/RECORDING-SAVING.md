# 録音保存の失敗表示・checkpoint保全（部分完了）

Linux実用版の保存経路を修正。再起動時のorphan一覧／DB再登録／全体GUI復旧はまだ未完。実mic/desktop収録・ASR/LLM/cloud requestなし。

## 保存の境界

- 保存先の初期化に失敗した場合、capture前に開始を拒否する。従来の「fallback flat structure」は実際には保存を保証していなかったため廃止。
- 会議folderにUUID suffixを付け、同じ題名・同じ分の別録音が既存metadata/audio/checkpointsを上書きしない。保存済folder_pathは変更しない。
- accumulatorの開始／停止boolean競合を削除。pipeline sender終了後にqueueを完全に読み、owned taskをawaitしてからfinalize。待機のキャンセルでもhandleをRecordingSaverに残し、Dropでabortする（detachしない）。
- checkpoint失敗はlogだけでなくrecording-save-warningを通知。同sessionでは連続checkpoint警告を抑制する。失敗時のbufferは既存IncrementalAudioSaverに残り、finalizeで再試行できる。
- checkpointはprivate temporary directoryでencode、file sync後に同filesystemのno-clobber hard linkで公開。未完成encodeはaudio_chunkとして公開しない。
- 結合はtemporary MP4→sync→atomic replacement。失敗時は以前のaudio.mp4とcheckpointsを保持。async FFmpegの120秒timeoutとkill-on-dropを追加。concat pathの空白・apostropheを保持する。
- completion metadataはunique temporary file／sync／atomic replacement。transcriptsとcompletion metadataが保存できた後だけ、従来の正常終了checkpoint cleanupを行う。
- audio saving無効でも、transcriptsとcompletion metadataは停止時に再保存する。音声chunksを捨てる既存選択は維持し、後から復元できるとは説明しない。

RecordingManagerは保存エラーを上位へ返す。Stopはリソース解放を完了するが、recording-stopped payloadにsave_succeeded / save_errorを付け、「録音停止」と「ファイル保存成功」を分離。frontendはstorage warning／最終保存失敗をpersistent toastで表示する。SQLite側のmeeting保存とは別の結果で、DB登録成功を音声保存成功の証拠にしない。native toastの実GUI動作は未確認。

## 合成検証

- accumulator開始／停止raceなくqueued1000chunksを処理。
- checkpoint directoryを一時的に除去し、警告1回＋buffer保持、修復後のfinalize成功。
- 同名folderを2つ生成し、元metadataを保持。
- invalid synthetic checkpointによるFFmpeg失敗で、以前のaudio.mp4とcheckpointを保持。
- synthetic float32 zerosでcheckpoint作成／結合（空白・apostropheを含むroot）。
- service mockでwarningとsave失敗payloadが変更されずUI側へ渡る。

Rust sink6＋incremental3＋minutes17＋transcription8成功（既存real Ollama1ignore）。frontend18tests、cargo check、TypeScript、pnpm10 production build、git diff check成功。locked/offline Cargo、analytics／Next telemetry無効。log: ../logs/meetily-save-drain-final.log。

初回はreserved empty temporary fileを既存overwrite拒否encoderへ渡しBroken pipeとなった。private temporary directory内の新規outputに修正して再検証。encoderのspawn/write/wait errorをpanic／unreaped childにしないよう修正した。

## 残る限界

- 再起動時にsession registryは消える。その後disk orphan scan／明示DB再登録を接続して合成検証した（[RECORDING-RECOVERY.md](RECORDING-RECOVERY.md)）。全体GUI・実crash／power loss／破損DB修復は未完。
- 原audio／transcript／canonical minutes／user edit historyを保持し、corrupt canonicalや未採用revision archiveを勝手に修復・昇格しない。
- 最後の未checkpoint bufferはRAM。process crash／長時間disk failure／memory pressureの無欠損は保証しない。queue/bufferの長時間障害時のbounded運用も未検証。
- checkpoint encodeは既存同期FFmpeg経路。完全にhungしたcheckpoint encoderの強制中断は今回保証していない（結合はasync timeout）。
- filesystem sync failure、power loss、GUI通知、保存失敗後のユーザー再試行、旧manual recovery/cleanup commandsの全体安全性は未検証。正常finalize単体ではcheckpointを削除しなくなったが、全復旧経路を監査済みとはしない。
