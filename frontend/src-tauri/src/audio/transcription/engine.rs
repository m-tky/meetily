//! Qwen-only engine selection shared by live recording and batch jobs.
use super::{provider::TranscriptionProvider, qwen_model, qwen_provider};
use std::sync::Arc;
use tauri::{AppHandle, Manager, Runtime};

#[derive(Clone)]
pub enum TranscriptionEngine {
    Provider(Arc<dyn TranscriptionProvider>),
}
impl TranscriptionEngine {
    pub async fn is_model_loaded(&self) -> bool {
        match self {
            Self::Provider(p) => p.is_model_loaded().await,
        }
    }
    pub async fn get_current_model(&self) -> Option<String> {
        match self {
            Self::Provider(p) => p.get_current_model().await,
        }
    }
    pub fn provider_name(&self) -> &str {
        match self {
            Self::Provider(p) => p.provider_name(),
        }
    }
}
pub async fn selected_model<R: Runtime>(app: &AppHandle<R>) -> Result<String, String> {
    let config = crate::api::api::api_get_transcript_config(app.clone(), app.state(), None).await?;
    match config {
        Some(c) => qwen_model::configured(&c.provider, &c.model),
        None => Ok(qwen_model::LARGE.into()),
    }
}
pub async fn validate_transcription_model_ready<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(), String> {
    if super::batch::is_busy() || super::qwen_setup::is_running() {
        return Err("Wait for audio import/retranscription/setup to finish".into());
    }
    qwen_provider::validate_language()?;
    qwen_provider::initialize_model(app, &selected_model(app).await?)
        .await
        .map(|_| ())
}
pub async fn get_or_init_transcription_engine<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<TranscriptionEngine, String> {
    qwen_provider::validate_language()?;
    Ok(TranscriptionEngine::Provider(
        qwen_provider::initialize_model(app, &selected_model(app).await?).await?,
    ))
}
