//! Import/retranscription use the same Qwen adapter, with job-owned state.
//! A failed/cancelled job cannot unload a live worker or change saved selection.
use super::{
    engine,
    provider::TranscriptionProvider,
    qwen_model,
    qwen_provider::{self, QwenProvider},
};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager, Runtime};
static BUSY: AtomicBool = AtomicBool::new(false);
pub fn is_busy() -> bool {
    BUSY.load(Ordering::SeqCst)
}
struct Lease;
impl Drop for Lease {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::SeqCst);
    }
}
pub struct BatchSession {
    provider: QwenProvider,
    _lease: Lease,
    pub language: String,
}
impl BatchSession {
    pub async fn open<R: Runtime>(
        app: &AppHandle<R>,
        provider: Option<&str>,
        model: Option<&str>,
        language: Option<&str>,
        cancelled: &AtomicBool,
    ) -> Result<Self, String> {
        if provider.is_some_and(|p| p != "qwen") {
            return Err("Only Qwen ASR is supported; select 1.7B or 0.6B".into());
        }
        let language = qwen_provider::language(
            language.or(crate::get_language_preference_internal().as_deref()),
        )?
        .to_string();
        let model = match model {
            Some(m) => qwen_model::validate(m)?.to_string(),
            None => engine::selected_model(app).await?,
        };
        let _lock = crate::audio::common::acquire_engine_lifecycle_lock().await;
        if crate::audio::recording_commands::is_recording().await || super::qwen_setup::is_running()
        {
            return Err("Stop recording/setup before importing or retranscribing audio".into());
        }
        if BUSY
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err("Another audio import or retranscription is running".into());
        }
        let lease = Lease;
        let settings = qwen_provider::settings_for(app, &model)?
            .ok_or("Set up the selected Qwen model first")?;
        if settings.model()? != model {
            return Err("Qwen model directory does not match selected model".into());
        }
        let cache = app
            .path()
            .app_cache_dir()
            .map_err(|e| e.to_string())?
            .join("qwen-batch-audio");
        let provider = QwenProvider::new(settings, cache)?;
        tokio::select! {
            result = provider.connect() => result?,
            _ = wait_cancelled(cancelled) => return Err("Transcription cancelled".into()),
        }
        Ok(Self {
            provider,
            _lease: lease,
            language,
        })
    }
    pub async fn transcribe(
        &self,
        audio: Vec<f32>,
        cancelled: &AtomicBool,
    ) -> Result<String, String> {
        cancellable(&self.provider, audio, &self.language, cancelled).await
    }
}
async fn wait_cancelled(cancelled: &AtomicBool) {
    while !cancelled.load(Ordering::SeqCst) {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}
async fn cancellable(
    provider: &dyn TranscriptionProvider,
    audio: Vec<f32>,
    language: &str,
    cancelled: &AtomicBool,
) -> Result<String, String> {
    if cancelled.load(Ordering::SeqCst) {
        return Err("Transcription cancelled".into());
    }
    tokio::select! {
        result = provider.transcribe(audio, Some(language.into())) => result.map(|r| r.text).map_err(|e| e.to_string()),
        _ = wait_cancelled(cancelled) => Err("Transcription cancelled".into()),
    }
}
#[cfg(test)]
mod tests {
    use super::super::provider::{TranscriptResult, TranscriptionError};
    use super::*;
    struct Pending;
    #[async_trait::async_trait]
    impl TranscriptionProvider for Pending {
        async fn transcribe(
            &self,
            _: Vec<f32>,
            _: Option<String>,
        ) -> Result<TranscriptResult, TranscriptionError> {
            std::future::pending().await
        }
        async fn is_model_loaded(&self) -> bool {
            true
        }
        async fn get_current_model(&self) -> Option<String> {
            None
        }
        fn provider_name(&self) -> &'static str {
            "fixture"
        }
    }
    #[tokio::test]
    async fn cancellation_interrupts_inflight_inference() {
        let flag = AtomicBool::new(false);
        let cancel = async {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            flag.store(true, Ordering::SeqCst);
        };
        let (result, _) = tokio::join!(cancellable(&Pending, vec![0.0; 1600], "ja", &flag), cancel);
        assert!(result.unwrap_err().contains("cancelled"));
    }
}
