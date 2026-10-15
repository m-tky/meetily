//! Opt-in Linux Qwen adapter. No cloud, shell command strings or model fallback.
//! A request owns its child while in flight: timeout/cancellation drops and kills
//! it, and the next request starts a fresh process. Failed audio is NOT retried.
use super::provider::{TranscriptResult, TranscriptionError, TranscriptionProvider};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    io::Write,
    path::PathBuf,
    process::Stdio,
    sync::{Arc, LazyLock},
    time::Duration,
};
use tauri::{Manager, Runtime};
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout},
    sync::Mutex,
};

pub const MODEL: &str = "Qwen3-ASR-1.7B-INT8";
const MAX_SAMPLES: usize = 30 * 16000;
const MAX_RESPONSE: usize = 128 * 1024;
static ENGINE: LazyLock<Mutex<Option<Arc<QwenProvider>>>> = LazyLock::new(|| Mutex::new(None));

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QwenSettings {
    pub worker_path: PathBuf,
    pub model_dir: PathBuf,
    pub ort_library: PathBuf,
}
impl QwenSettings {
    fn validate(&self) -> Result<(), String> {
        if !cfg!(target_os = "linux") {
            return Err("Qwen ASR currently supports Linux only".into());
        }
        for path in [&self.worker_path, &self.model_dir, &self.ort_library] {
            if !path.is_absolute() {
                return Err("Select absolute local Qwen paths".into());
            }
        }
        if !self.worker_path.is_file() || !self.ort_library.is_file() {
            return Err("Qwen worker / ONNX runtime library not found".into());
        }
        let bytes = std::fs::read(self.model_dir.join("config.json"))
            .map_err(|_| "Qwen model config.json not found")?;
        let config: Value =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid Qwen model config")?;
        let text = &config["thinker_config"]["text_config"];
        if config["model_type"] != "qwen3_asr"
            || text["hidden_size"] != 2048
            || text["num_hidden_layers"] != 28
        {
            return Err("Select the Qwen3-ASR 1.7B model directory".into());
        }
        for name in [
            "model-00001-of-00002.safetensors",
            "model-00002-of-00002.safetensors",
            "qwen-asr-int8.sidecar",
            "vocab.json",
            "merges.txt",
        ] {
            if !self.model_dir.join(name).is_file() {
                return Err(format!(
                    "Qwen prepared model is missing {name}; follow the local setup guide"
                ));
            }
        }
        Ok(())
    }
}

struct Worker {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    requests: usize,
}
async fn line<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<Value, String> {
    let mut bytes = Vec::new();
    loop {
        let available = reader
            .fill_buf()
            .await
            .map_err(|_| "Qwen stdout read failed")?;
        if available.is_empty() {
            return Err("Qwen worker exited / truncated response".into());
        }
        let take = available
            .iter()
            .position(|b| *b == b'\n')
            .map_or(available.len(), |p| p + 1);
        if bytes.len() + take > MAX_RESPONSE {
            return Err("Qwen response exceeds protocol bound".into());
        }
        let ended = available[take - 1] == b'\n';
        bytes.extend_from_slice(&available[..take]);
        reader.consume(take);
        if ended {
            return serde_json::from_slice(&bytes).map_err(|_| "Invalid Qwen JSON response".into());
        }
    }
}
fn validate_ready(v: &Value) -> Result<(), String> {
    if v["event"] != "ready"
        || v["protocol"] != 1
        || v["threads"] != 6
        || v["model"] != "Qwen3-ASR-1.7B"
        || v["precision"] != "decoder-INT8-encoder-prefill-F32"
        || v["recognition_context"] != "full_input"
        || v["vad_only"] != false
        || v["max_audio_seconds"] != 30
        || v["vad_frame_samples"] != 512
        || !v["audio_formats"]
            .as_array()
            .is_some_and(|a| a.iter().any(|x| x == "float32"))
    {
        return Err("Incompatible Qwen worker; install the float32-input Linux worker".into());
    }
    Ok(())
}
fn language(code: Option<&str>) -> Result<&'static str, String> {
    match code {
        Some("ja"|"Japanese")=>Ok("Japanese"), Some("en"|"English")=>Ok("English"), Some("th"|"Thai")=>Ok("Thai"),
        _=>Err("Qwen requires Japanese (ja), English (en), or Thai (th); automatic detection / translation is not enabled".into()),
    }
}
pub fn validate_language() -> Result<(), String> {
    language(crate::get_language_preference_internal().as_deref()).map(|_| ())
}
/// IEEE float WAV, including fact chunk. No PCM16 conversion / input gain.
fn float_wav(audio: &[f32]) -> Result<Vec<u8>, String> {
    if audio.is_empty()
        || audio.len() > MAX_SAMPLES
        || audio.iter().any(|x| !x.is_finite() || x.abs() > 1.0)
    {
        return Err(
            "Qwen expects finite mono16k float audio, at most30seconds, within [-1,1]".into(),
        );
    }
    let data = (audio.len() * 4) as u32;
    let mut bytes = Vec::with_capacity(58 + data as usize);
    bytes.extend(b"RIFF");
    bytes.extend((50 + data).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(18u32.to_le_bytes());
    bytes.extend(3u16.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(16000u32.to_le_bytes());
    bytes.extend(64000u32.to_le_bytes());
    bytes.extend(4u16.to_le_bytes());
    bytes.extend(32u16.to_le_bytes());
    bytes.extend(0u16.to_le_bytes());
    bytes.extend(b"fact");
    bytes.extend(4u32.to_le_bytes());
    bytes.extend((audio.len() as u32).to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(data.to_le_bytes());
    for x in audio {
        bytes.extend(x.to_le_bytes());
    }
    Ok(bytes)
}
fn response(v: &Value, id: &str) -> Result<TranscriptResult, String> {
    if v["id"] != id || v["confirmed"] != false {
        return Err("Qwen response ID/confirmation mismatch".into());
    }
    let text = v["text"].as_str().ok_or("Qwen response missing text")?;
    match v["status"].as_str() {
        Some("no_speech") if text.is_empty() && v["asr_calls"] == 0 => {}
        Some("recognized" | "review_required")
            if v["asr_calls"] == 1 && v["recognition_context"] == "full_input" => {}
        _ => {
            return Err(
                "Qwen transcription failed / invalid response contract; no model fallback".into(),
            )
        }
    }
    Ok(TranscriptResult {
        text: text.into(),
        confidence: None,
        is_partial: false,
    })
}

pub struct QwenProvider {
    settings: QwenSettings,
    cache: PathBuf,
    worker: Mutex<Option<Worker>>,
    timeout: Duration,
}
impl QwenProvider {
    pub fn new(settings: QwenSettings, cache: PathBuf) -> Result<Self, String> {
        settings.validate()?;
        std::fs::create_dir_all(&cache)
            .map_err(|_| "Cannot create private Qwen temporary directory")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| "Cannot protect Qwen temporary directory")?;
        }
        Ok(Self {
            settings,
            cache,
            worker: Mutex::new(None),
            timeout: Duration::from_secs(180),
        })
    }
    async fn spawn(&self) -> Result<Worker, String> {
        let mut child=tokio::process::Command::new(&self.settings.worker_path)
            .arg(&self.settings.model_dir).arg("6")
            .env("ORT_DYLIB_PATH",&self.settings.ort_library)
            .env("OPENBLAS_NUM_THREADS","6").env("OMP_NUM_THREADS","6").env("MKL_NUM_THREADS","6")
            .env("HF_HUB_DISABLE_TELEMETRY","1").env("DO_NOT_TRACK","1")
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true)
            .spawn().map_err(|_| "Cannot start Qwen worker; check executable permission and local runtime dependencies")?;
        let input = child.stdin.take().ok_or("Qwen stdin unavailable")?;
        let output = BufReader::new(child.stdout.take().ok_or("Qwen stdout unavailable")?);
        let mut worker = Worker {
            child,
            input,
            output,
            requests: 0,
        };
        let ready = tokio::time::timeout(Duration::from_secs(30), line(&mut worker.output))
            .await
            .map_err(|_| "Qwen ready handshake timed out")??;
        validate_ready(&ready)?;
        Ok(worker)
    }
    pub async fn connect(&self) -> Result<(), String> {
        let mut state = self.worker.lock().await;
        if state.is_none() {
            *state = Some(self.spawn().await?);
        }
        Ok(())
    }
    async fn shutdown(&self) {
        if let Some(mut worker) = self.worker.lock().await.take() {
            let _ = worker.child.kill().await;
        }
    }
    async fn infer(
        &self,
        audio: Vec<f32>,
        code: Option<String>,
    ) -> Result<TranscriptResult, String> {
        let lang = language(code.as_deref())?;
        let bytes = float_wav(&audio)?;
        let mut file = tempfile::NamedTempFile::new_in(&self.cache)
            .map_err(|_| "Cannot create Qwen temporary audio")?;
        file.write_all(&bytes)
            .map_err(|_| "Cannot write Qwen temporary audio")?;
        file.flush()
            .map_err(|_| "Cannot flush Qwen temporary audio")?;
        let mut state = self.worker.lock().await;
        let mut worker = match state.take() {
            Some(mut w) => {
                if w.requests < 1000
                    && w.child
                        .try_wait()
                        .map_err(|_| "Qwen process status failed")?
                        .is_none()
                {
                    w
                } else {
                    drop(w);
                    self.spawn().await?
                }
            }
            _ => self.spawn().await?,
        };
        worker.requests += 1;
        let id = format!("request-{}", worker.requests);
        let request = serde_json::to_vec(
            &json!({"op":"transcribe","id":id,"wav":file.path(),"language":lang}),
        )
        .map_err(|_| "Qwen request encoding failed")?;
        if request.len() + 1 > 4096 {
            return Err("Qwen request exceeds protocol bound".into());
        }
        let work = async {
            worker
                .input
                .write_all(&request)
                .await
                .map_err(|_| "Qwen stdin write failed")?;
            worker
                .input
                .write_all(b"\n")
                .await
                .map_err(|_| "Qwen newline write failed")?;
            worker
                .input
                .flush()
                .await
                .map_err(|_| "Qwen stdin flush failed")?;
            let value = line(&mut worker.output).await?;
            response(&value, &id)
        };
        let result = tokio::time::timeout(self.timeout, work)
            .await
            .map_err(|_| "Qwen request timed out; worker killed, failed audio was not retried")?;
        if result.is_ok() {
            *state = Some(worker);
        }
        // On error/cancellation owned worker drops (kill_on_drop) and the private
        // temporary audio unlinks. Next request starts clean; no silent retry.
        result
    }
}
#[async_trait]
impl TranscriptionProvider for QwenProvider {
    async fn transcribe(
        &self,
        audio: Vec<f32>,
        language: Option<String>,
    ) -> Result<TranscriptResult, TranscriptionError> {
        // The live pipeline may deliver speech bursts longer than the bounded
        // worker contract. Preserve every sample in consecutive <=30s requests.
        // No overlap, input gain, truncation, retry or transcript correction.
        if audio.is_empty() {
            return Err(TranscriptionError::AudioTooShort {
                samples: 0,
                minimum: 1,
            });
        }
        let mut texts = Vec::new();
        for window in audio.chunks(MAX_SAMPLES) {
            let result = self
                .infer(window.to_vec(), language.clone())
                .await
                .map_err(TranscriptionError::EngineFailed)?;
            if !result.text.is_empty() {
                texts.push(result.text);
            }
        }
        Ok(TranscriptResult {
            text: texts.join("\n"),
            confidence: None,
            is_partial: false,
        })
    }
    async fn is_model_loaded(&self) -> bool {
        true
    } // Prepared/ready; weights load lazily on actual speech.
    async fn get_current_model(&self) -> Option<String> {
        Some(MODEL.into())
    }
    fn provider_name(&self) -> &'static str {
        "Qwen3-ASR (local AI draft)"
    }
}
fn config_path<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_config_dir()
        .map_err(|_| "App config directory unavailable")?
        .join("qwen-asr.json"))
}
#[tauri::command]
pub fn qwen_is_supported() -> bool {
    cfg!(target_os = "linux")
}
#[tauri::command]
pub fn qwen_get_settings<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<QwenSettings>, String> {
    match std::fs::read(config_path(&app)?) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| "Invalid saved Qwen settings".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("Cannot read Qwen settings".into()),
    }
}
#[tauri::command]
pub async fn qwen_save_settings<R: Runtime>(
    app: tauri::AppHandle<R>,
    settings: QwenSettings,
) -> Result<(), String> {
    let _guard = crate::audio::common::acquire_engine_lifecycle_lock().await;
    if crate::audio::recording_commands::is_recording().await {
        return Err("Stop recording before changing Qwen settings".into());
    }
    settings.validate()?;
    let path = config_path(&app)?;
    let parent = path.parent().ok_or("Missing config parent")?;
    std::fs::create_dir_all(parent).map_err(|_| "Cannot create config directory")?;
    let mut file =
        tempfile::NamedTempFile::new_in(parent).map_err(|_| "Cannot create settings file")?;
    serde_json::to_writer(&mut file, &settings).map_err(|_| "Cannot encode Qwen settings")?;
    file.as_file()
        .sync_all()
        .map_err(|_| "Cannot sync Qwen settings")?;
    file.persist(path)
        .map_err(|_| "Cannot save Qwen settings")?;
    shutdown().await;
    Ok(())
}
pub async fn initialize<R: Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<Arc<QwenProvider>, String> {
    let mut engine = ENGINE.lock().await;
    if let Some(p) = &*engine {
        return Ok(p.clone());
    }
    let settings = qwen_get_settings(app.clone())?
        .ok_or("Configure the local Qwen worker/model/runtime in transcription settings first")?;
    let cache = app
        .path()
        .app_cache_dir()
        .map_err(|_| "App cache directory unavailable")?
        .join("qwen-audio");
    let provider = Arc::new(QwenProvider::new(settings, cache)?);
    provider.connect().await?;
    *engine = Some(provider.clone());
    Ok(provider)
}
pub async fn shutdown() {
    if let Some(provider) = ENGINE.lock().await.take() {
        provider.shutdown().await;
    }
}
#[tauri::command]
pub async fn qwen_init<R: Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    let _guard = crate::audio::common::acquire_engine_lifecycle_lock().await;
    validate_language()?;
    initialize(&app).await.map(|_| ())
}
#[tauri::command]
pub async fn qwen_has_available_models<R: Runtime>(app: tauri::AppHandle<R>) -> bool {
    qwen_get_settings(app)
        .ok()
        .flatten()
        .is_some_and(|s| s.validate().is_ok())
}
#[tauri::command]
pub async fn qwen_get_available_models<R: Runtime>(app: tauri::AppHandle<R>) -> Vec<Value> {
    if qwen_has_available_models(app).await {
        vec![json!({"name":MODEL,"status":"Available"})]
    } else {
        vec![]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "linux")]
    fn mock_provider(root: &std::path::Path, behavior: &str) -> QwenProvider {
        use std::os::unix::fs::PermissionsExt;
        let worker_path = root.join("mock-worker");
        let ready = json!({"event":"ready","protocol":1,"threads":6,"model":"Qwen3-ASR-1.7B",
            "precision":"decoder-INT8-encoder-prefill-F32","recognition_context":"full_input",
            "vad_only":false,"max_audio_seconds":30,"vad_frame_samples":512,"audio_formats":["float32"]});
        std::fs::write(
            &worker_path,
            format!("#!/bin/sh\nprintf '%s\\n' '{}'\n{}\n", ready, behavior),
        )
        .unwrap();
        std::fs::set_permissions(&worker_path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let cache = root.join("audio");
        std::fs::create_dir_all(&cache).unwrap();
        QwenProvider {
            settings: QwenSettings {
                worker_path,
                model_dir: root.into(),
                ort_library: root.join("fake-ort"),
            },
            cache,
            worker: Mutex::new(None),
            timeout: Duration::from_millis(150),
        }
    }
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn mock_worker_preserves_long_input_and_unlinks_audio() {
        let root = tempfile::tempdir().unwrap();
        let provider = mock_provider(
            root.path(),
            r#"i=0
while IFS= read -r request; do
  i=$((i+1))
  printf '{"id":"request-%s","confirmed":false,"text":"draft","asr_calls":1,"status":"recognized","recognition_context":"full_input"}\n' "$i"
done"#,
        );
        let result = provider
            .transcribe(vec![1e-6; MAX_SAMPLES + 1], Some("ja".into()))
            .await
            .unwrap();
        assert_eq!(result.text, "draft\ndraft");
        assert_eq!(provider.worker.lock().await.as_ref().unwrap().requests, 2);
        assert_eq!(std::fs::read_dir(&provider.cache).unwrap().count(), 0);
        provider.shutdown().await;
        assert!(provider.worker.lock().await.is_none());
    }
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn mock_timeout_crash_restart_and_cancel_do_not_retain_audio() {
        let root = tempfile::tempdir().unwrap();
        let provider = mock_provider(root.path(), "exec sleep 5");
        assert!(provider
            .infer(vec![0.0; 1], Some("ja".into()))
            .await
            .unwrap_err()
            .contains("timed out"));
        assert!(provider.worker.lock().await.is_none());
        assert_eq!(std::fs::read_dir(&provider.cache).unwrap().count(), 0);
        // Cancellation must also drop the owned child and private WAV.
        assert!(tokio::time::timeout(
            Duration::from_millis(30),
            provider.infer(vec![0.0; 1], Some("ja".into()))
        )
        .await
        .is_err());
        assert_eq!(std::fs::read_dir(&provider.cache).unwrap().count(), 0);
        assert!(provider.worker.lock().await.is_none());
        // Replace executable with a crashed worker, then a healthy one. The
        // failed audio isn't replayed; only this subsequent request succeeds.
        let _crashed_script = mock_provider(root.path(), "exit 7");
        assert!(provider
            .infer(vec![0.0; 1], Some("ja".into()))
            .await
            .is_err());
        assert!(provider.worker.lock().await.is_none());
        let _healthy_script = mock_provider(
            root.path(),
            r#"IFS= read -r request
printf '%s\n' '{"id":"request-1","confirmed":false,"text":"","asr_calls":0,"status":"no_speech"}'"#,
        );
        assert!(provider
            .infer(vec![0.0; 1], Some("ja".into()))
            .await
            .unwrap()
            .text
            .is_empty());
        provider.shutdown().await;
        assert_eq!(std::fs::read_dir(&provider.cache).unwrap().count(), 0);
    }
    #[test]
    fn float_transport_preserves_tiny_samples_and_checks_bounds() {
        let values = [1e-6f32, -2e-6, 0.0];
        let wav = float_wav(&values).unwrap();
        assert_eq!(&wav[20..22], &3u16.to_le_bytes());
        assert_eq!(
            &wav[58..],
            values
                .iter()
                .flat_map(|x| x.to_le_bytes())
                .collect::<Vec<_>>()
        );
        for bad in [
            vec![],
            vec![f32::NAN],
            vec![1.01],
            vec![0.0; MAX_SAMPLES + 1],
        ] {
            assert!(float_wav(&bad).is_err());
        }
    }
    #[test]
    fn response_and_language_fail_closed() {
        assert!(language(None).is_err());
        assert!(language(Some("auto")).is_err());
        assert_eq!(language(Some("ja")).unwrap(), "Japanese");
        let v = json!({"id":"x","confirmed":false,"text":"家。","asr_calls":1,"status":"review_required","recognition_context":"full_input"});
        assert_eq!(response(&v, "x").unwrap().text, "家。");
        assert!(response(&v, "other").is_err());
        let bad =
            json!({"id":"x","confirmed":true,"text":"fake","asr_calls":0,"status":"no_speech"});
        assert!(response(&bad, "x").is_err());
    }
    #[tokio::test]
    async fn framing_rejects_oversize_truncated_and_invalid_json() {
        for bytes in [
            vec![b'x'; MAX_RESPONSE + 1],
            b"{}".to_vec(),
            b"not json\n".to_vec(),
        ] {
            let mut reader = BufReader::new(std::io::Cursor::new(bytes));
            assert!(line(&mut reader).await.is_err());
        }
    }
}
