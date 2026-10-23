use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;
use log::{info, warn};
use anyhow::Result;

use crate::state::AppState;
use crate::database::repositories::setting::SettingsRepository;


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OnboardingStatus {
    pub version: String,
    pub completed: bool,
    pub current_step: u8,
    pub model_status: ModelStatus,
    pub last_updated: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ModelStatus {
    pub parakeet: String,  // "downloaded" | "not_downloaded" | "downloading"
    pub summary: String,   // Generic field for summary model (Qwen 3.5 or legacy Gemma variants)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_summary_model: Option<String>,
}

impl Default for OnboardingStatus {
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            completed: false,
            current_step: 1,
            model_status: ModelStatus {
                parakeet: "not_downloaded".to_string(),
                summary: "not_downloaded".to_string(),  // Changed from gemma
                selected_summary_model: None,
            },
            last_updated: chrono::Utc::now().to_rfc3339(),
        }
    }
}


/// Load onboarding status from store
pub async fn load_onboarding_status<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<OnboardingStatus> {
    // Try to load from Tauri store
    let store = match app.store("onboarding-status.json") {
        Ok(store) => store,
        Err(e) => {
            warn!("Failed to access onboarding store: {}, using defaults", e);
            return Ok(OnboardingStatus::default());
        }
    };

    // Try to get the status from store
    let status = if let Some(value) = store.get("status") {
        match serde_json::from_value::<OnboardingStatus>(value.clone()) {
            Ok(s) => {
                info!("Loaded onboarding status from store - Step: {}, Completed: {}",
                      s.current_step, s.completed);
                s
            }
            Err(e) => {
                warn!("Failed to deserialize onboarding status: {}, using defaults", e);
                OnboardingStatus::default()
            }
        }
    } else {
        info!("No stored onboarding status found, using defaults");
        OnboardingStatus::default()
    };

    Ok(status)
}

/// Save onboarding status to store
pub async fn save_onboarding_status<R: Runtime>(
    app: &AppHandle<R>,
    status: &OnboardingStatus,
) -> Result<()> {
    info!("Saving onboarding status: step={}, completed={}",
          status.current_step, status.completed);

    // Serialize status commits and reject delayed incomplete autosaves after
    // either completion route. Reset remains an explicit separate operation.
    static WRITE: std::sync::LazyLock<tokio::sync::Mutex<()>> = std::sync::LazyLock::new(|| tokio::sync::Mutex::new(()));
    let _write = WRITE.lock().await;
    // Get or create store
    let store = app.store("onboarding-status.json")
        .map_err(|e| anyhow::anyhow!("Failed to access onboarding store: {}", e))?;

    if stale_incomplete_status(store.get("status"), status) {
        return Ok(());
    }

    // Update last_updated timestamp
    let mut status = status.clone();
    status.last_updated = chrono::Utc::now().to_rfc3339();

    // Serialize status to JSON value
    let status_value = serde_json::to_value(&status)
        .map_err(|e| anyhow::anyhow!("Failed to serialize onboarding status: {}", e))?;

    // Save to store
    store.set("status", status_value);

    // Persist to disk
    store.save()
        .map_err(|e| anyhow::anyhow!("Failed to save onboarding store to disk: {}", e))?;

    info!("Successfully persisted onboarding status to disk");
    Ok(())
}

fn stale_incomplete_status(previous: Option<serde_json::Value>, status: &OnboardingStatus) -> bool {
    !status.completed && previous.and_then(|v| v.get("completed").and_then(|b| b.as_bool())) == Some(true)
}

fn qwen_completion_status(mut status: OnboardingStatus) -> OnboardingStatus {
    status.completed = true;
    status.current_step = 4;
    status
}

/// Reset onboarding status (delete from store)
pub async fn reset_onboarding_status<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<()> {
    info!("Resetting onboarding status");

    let store = app.store("onboarding-status.json")
        .map_err(|e| anyhow::anyhow!("Failed to access onboarding store: {}", e))?;

    // Clear the status key
    store.delete("status");

    // Persist deletion to disk
    store.save()
        .map_err(|e| anyhow::anyhow!("Failed to save onboarding store after reset: {}", e))?;

    info!("Successfully reset onboarding status");
    Ok(())
}

/// Tauri commands for onboarding status
#[tauri::command]
pub async fn get_onboarding_status<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Option<OnboardingStatus>, String> {
    let status = load_onboarding_status(&app)
        .await
        .map_err(|e| format!("Failed to load onboarding status: {}", e))?;

    // Return None if it's the default (never saved before)
    // Check if we have any saved data by seeing if the store has the key
    let store = app.store("onboarding-status.json")
        .map_err(|e| format!("Failed to access store: {}", e))?;

    if store.get("status").is_none() {
        Ok(None)
    } else {
        Ok(Some(status))
    }
}

#[tauri::command]
pub async fn save_onboarding_status_cmd<R: Runtime>(
    app: AppHandle<R>,
    status: OnboardingStatus,
) -> Result<(), String> {
    save_onboarding_status(&app, &status)
        .await
        .map_err(|e| format!("Failed to save onboarding status: {}", e))
}

#[tauri::command]
pub async fn reset_onboarding_status_cmd<R: Runtime>(
    app: AppHandle<R>,
) -> Result<(), String> {
    reset_onboarding_status(&app)
        .await
        .map_err(|e| format!("Failed to reset onboarding status: {}", e))
}

#[tauri::command]
pub async fn complete_onboarding<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    model: String,
) -> Result<(), String> {
    let _ = model; // Legacy command delegates to verified ASR-only completion.
    complete_qwen_onboarding(app, state).await
}

/// Transcription-only setup must not start downloads, configure a summary
/// provider, or claim that Parakeet/summary weights have been downloaded.
#[tauri::command]
pub async fn complete_qwen_onboarding<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    if !crate::audio::transcription::qwen_provider::qwen_is_supported() {
        return Err("Qwen-only onboarding requires Linux or Windows x64".into());
    }
    let selected = SettingsRepository::get_transcript_config(state.db_manager.pool())
        .await.map_err(|e| e.to_string())?.ok_or("Select Qwen before completing setup")?;
    if selected.provider != "qwen" || crate::audio::transcription::qwen_model::validate(&selected.model).is_err() {
        return Err("Select the verified Qwen model before completing setup".into());
    }
    crate::audio::transcription::qwen_provider::qwen_init(app.clone()).await?;
    let status = qwen_completion_status(load_onboarding_status(&app).await.map_err(|e| e.to_string())?);
    save_onboarding_status(&app, &status).await.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qwen_completion_does_not_fabricate_model_downloads() {
        let mut original = OnboardingStatus::default();
        original.model_status.selected_summary_model = Some("existing-choice".into());
        for parakeet in ["not_downloaded", "downloading", "downloaded"] {
            original.model_status.parakeet = parakeet.into();
            let finished = qwen_completion_status(original.clone());
            assert!(finished.completed);
            assert_eq!(finished.current_step, 4);
            assert_eq!(finished.model_status.parakeet, parakeet);
            assert_eq!(finished.model_status.summary, "not_downloaded");
            assert_eq!(finished.model_status.selected_summary_model.as_deref(), Some("existing-choice"));
        }
    }

    #[test]
    fn delayed_autosave_cannot_undo_completion() {
        let incoming = OnboardingStatus::default();
        assert!(stale_incomplete_status(Some(serde_json::json!({"completed": true})), &incoming));
        assert!(!stale_incomplete_status(Some(serde_json::json!({"completed": false})), &incoming));
        assert!(!stale_incomplete_status(None, &incoming));
        assert!(!stale_incomplete_status(Some(serde_json::json!({"completed": true})), &qwen_completion_status(incoming)));
    }

    #[test]
    fn onboarding_status_deserializes_without_selected_summary_model() {
        let status: OnboardingStatus = serde_json::from_str(
            r#"{
                "version": "1.0",
                "completed": true,
                "current_step": 4,
                "model_status": {
                    "parakeet": "downloaded",
                    "summary": "downloaded"
                },
                "last_updated": "2026-05-30T00:00:00Z"
            }"#,
        )
        .expect("old onboarding status should remain compatible");

        assert_eq!(status.model_status.selected_summary_model, None);
    }
}
