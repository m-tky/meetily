//! Reprocess saved audio through the same Qwen adapter used by live recording/import.
use super::{
    common::{create_transcript_segments, split_segment_at_silence, write_transcripts_json},
    constants::AUDIO_EXTENSIONS,
    decoder::decode_audio_file,
    transcription::batch::BatchSession,
    vad::get_speech_chunks_with_progress,
};
use crate::state::AppState;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use tauri::{AppHandle, Emitter, Manager, Runtime};
const VAD_REDEMPTION_TIME_MS: u32 = 2000;
static RETRANSCRIPTION_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
static RETRANSCRIPTION_CANCELLED: AtomicBool = AtomicBool::new(false);
struct RetranscriptionGuard;
impl RetranscriptionGuard {
    fn acquire() -> Result<Self, String> {
        RETRANSCRIPTION_IN_PROGRESS
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "Retranscription already in progress".to_string())?;
        RETRANSCRIPTION_CANCELLED.store(false, Ordering::SeqCst);
        Ok(Self)
    }
}
impl Drop for RetranscriptionGuard {
    fn drop(&mut self) {
        RETRANSCRIPTION_IN_PROGRESS.store(false, Ordering::SeqCst);
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetranscriptionProgress {
    pub meeting_id: String,
    pub stage: String,
    pub progress_percentage: u32,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetranscriptionResult {
    pub meeting_id: String,
    pub segments_count: usize,
    pub duration_seconds: f64,
    pub language: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetranscriptionError {
    pub meeting_id: String,
    pub error: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetranscriptionStarted {
    pub meeting_id: String,
    pub message: String,
}
pub fn is_retranscription_in_progress() -> bool {
    RETRANSCRIPTION_IN_PROGRESS.load(Ordering::SeqCst)
}
pub fn cancel_retranscription() {
    RETRANSCRIPTION_CANCELLED.store(true, Ordering::SeqCst);
}
fn check_cancel() -> Result<()> {
    if RETRANSCRIPTION_CANCELLED.load(Ordering::SeqCst) {
        Err(anyhow!("Retranscription cancelled"))
    } else {
        Ok(())
    }
}
fn find_audio_file(folder: &Path) -> Result<PathBuf> {
    for name in [
        "audio.mp4",
        "audio.m4a",
        "audio.wav",
        "audio.mp3",
        "audio.flac",
        "audio.ogg",
        "recording.mp4",
        "audio.mkv",
        "audio.webm",
        "audio.wma",
    ] {
        let path = folder.join(name);
        if path.is_file() {
            return Ok(path);
        }
    }
    for entry in std::fs::read_dir(folder)?.flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| AUDIO_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        {
            return Ok(path);
        }
    }
    Err(anyhow!("No audio file found in: {}", folder.display()))
}
fn emit_progress<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
    stage: &str,
    progress: u32,
    message: &str,
) {
    let _ = app.emit(
        "retranscription-progress",
        RetranscriptionProgress {
            meeting_id: id.into(),
            stage: stage.into(),
            progress_percentage: progress,
            message: message.into(),
        },
    );
}
pub async fn start_retranscription<R: Runtime>(
    app: AppHandle<R>,
    meeting_id: String,
    meeting_folder_path: String,
    language: Option<String>,
    model: Option<String>,
    provider: Option<String>,
) -> Result<RetranscriptionResult> {
    let _guard = RetranscriptionGuard::acquire().map_err(|e| anyhow!(e))?;
    let result = run_retranscription(
        &app,
        &meeting_id,
        &meeting_folder_path,
        language,
        model,
        provider,
    )
    .await;
    match &result {
        Ok(value) => {
            let _ = app.emit("retranscription-complete", value);
        }
        Err(error) => {
            let _ = app.emit(
                "retranscription-error",
                RetranscriptionError {
                    meeting_id,
                    error: error.to_string(),
                },
            );
        }
    }
    result
}
async fn run_retranscription<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
    folder: &str,
    language: Option<String>,
    model: Option<String>,
    provider: Option<String>,
) -> Result<RetranscriptionResult> {
    let session = BatchSession::open(
        app,
        provider.as_deref(),
        model.as_deref(),
        language.as_deref(),
        &RETRANSCRIPTION_CANCELLED,
    )
    .await
    .map_err(|e| anyhow!(e))?;
    let folder = Path::new(folder);
    let audio = find_audio_file(folder)?;
    // Never accept a frontend-supplied folder for an unrelated saved meeting.
    let state = app.state::<AppState>();
    let stored: Option<String> =
        sqlx::query_scalar("SELECT folder_path FROM meetings WHERE id = ?")
            .bind(id)
            .fetch_optional(state.db_manager.pool())
            .await?
            .flatten();
    if stored
        .as_deref()
        .map(Path::new)
        .map(std::fs::canonicalize)
        .transpose()?
        != Some(std::fs::canonicalize(folder)?)
    {
        return Err(anyhow!("Meeting folder does not match the saved meeting"));
    }
    check_cancel()?;
    emit_progress(app, id, "decoding", 5, "Decoding audio file...");
    let path = audio.clone();
    let decoded = tokio::task::spawn_blocking(move || decode_audio_file(&path)).await??;
    let duration = decoded.duration_seconds;
    check_cancel()?;
    let samples = tokio::task::spawn_blocking(move || decoded.to_whisper_format()).await?;
    emit_progress(app, id, "vad", 20, "Detecting speech segments...");
    let app_vad = app.clone();
    let id_vad = id.to_string();
    let segments = tokio::task::spawn_blocking(move || {
        get_speech_chunks_with_progress(&samples, VAD_REDEMPTION_TIME_MS, |p, _| {
            emit_progress(
                &app_vad,
                &id_vad,
                "vad",
                20 + p / 20,
                "Detecting speech segments...",
            );
            !RETRANSCRIPTION_CANCELLED.load(Ordering::SeqCst)
        })
    })
    .await?
    .map_err(|e| anyhow!("VAD processing failed: {e}"))?;
    check_cancel()?;
    if segments.is_empty() {
        return Err(anyhow!(
            "No speech detected in audio file; existing transcripts were kept"
        ));
    }
    let segments: Vec<_> = segments
        .iter()
        .flat_map(|s| split_segment_at_silence(s, 25 * 16000))
        .collect();
    let mut texts = Vec::new();
    for (i, s) in segments.iter().enumerate() {
        check_cancel()?;
        emit_progress(
            app,
            id,
            "transcribing",
            25 + (i * 55 / segments.len()) as u32,
            &format!("Transcribing segment {} of {}...", i + 1, segments.len()),
        );
        if s.samples.len() < 1600 {
            continue;
        }
        let text = session
            .transcribe(s.samples.clone(), &RETRANSCRIPTION_CANCELLED)
            .await
            .map_err(|e| anyhow!(e))?;
        if !text.trim().is_empty() {
            texts.push((text, s.start_timestamp_ms, s.end_timestamp_ms));
        }
    }
    check_cancel()?;
    // Failure/empty recognition must not silently erase existing content.
    if texts.is_empty() {
        return Err(anyhow!(
            "Qwen returned no text; existing transcripts were kept"
        ));
    }
    let segments = create_transcript_segments(&texts);
    emit_progress(app, id, "saving", 80, "Saving transcripts...");
    let mut tx = state.db_manager.pool().begin().await?;
    sqlx::query("DELETE FROM transcripts WHERE meeting_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    for s in &segments {
        sqlx::query("INSERT INTO transcripts (id,meeting_id,transcript,timestamp,audio_start_time,audio_end_time,duration) VALUES (?,?,?,?,?,?,?)")
            .bind(&s.id).bind(id).bind(&s.text).bind(&s.timestamp).bind(s.audio_start_time).bind(s.audio_end_time).bind(s.duration).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    write_transcripts_json(folder, &segments)?;
    write_retranscription_metadata(
        folder,
        id,
        duration,
        audio
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio.mp4"),
    )?;
    emit_progress(app, id, "complete", 100, "Retranscription complete");
    Ok(RetranscriptionResult {
        meeting_id: id.into(),
        segments_count: segments.len(),
        duration_seconds: duration,
        language: Some(session.language.clone()),
    })
}
fn write_retranscription_metadata(
    folder: &Path,
    meeting_id: &str,
    duration_seconds: f64,
    audio_filename: &str,
) -> Result<()> {
    let path = folder.join("metadata.json");
    let now = chrono::Utc::now().to_rfc3339();
    let mut value = if path.exists() {
        serde_json::from_slice(&std::fs::read(&path)?)?
    } else {
        serde_json::json!({"version":"1.0","meeting_id":meeting_id,"created_at":now,"completed_at":now,"audio_file":audio_filename,"source":"retranscription"})
    };
    let object = value
        .as_object_mut()
        .ok_or_else(|| anyhow!("Invalid recording metadata"))?;
    object.insert(
        "duration_seconds".into(),
        serde_json::json!(duration_seconds),
    );
    object.insert("retranscribed_at".into(), serde_json::json!(now));
    object.insert("status".into(), serde_json::json!("completed"));
    object.insert(
        "transcript_file".into(),
        serde_json::json!("transcripts.json"),
    );
    object.remove("detected_summary_language");
    let temporary = folder.join(".metadata.json.tmp");
    std::fs::write(&temporary, serde_json::to_vec_pretty(&value)?)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}
#[tauri::command]
pub async fn start_retranscription_command<R: Runtime>(
    app: AppHandle<R>,
    meeting_id: String,
    meeting_folder_path: String,
    language: Option<String>,
    model: Option<String>,
    provider: Option<String>,
) -> Result<RetranscriptionStarted, String> {
    if is_retranscription_in_progress() {
        return Err("Retranscription already in progress".into());
    }
    let id = meeting_id.clone();
    tauri::async_runtime::spawn(async move {
        let _ =
            start_retranscription(app, id, meeting_folder_path, language, model, provider).await;
    });
    Ok(RetranscriptionStarted {
        meeting_id,
        message: "Retranscription started".into(),
    })
}
#[tauri::command]
pub async fn cancel_retranscription_command() -> Result<(), String> {
    if !is_retranscription_in_progress() {
        return Err("No retranscription in progress".into());
    }
    cancel_retranscription();
    Ok(())
}
#[tauri::command]
pub async fn is_retranscription_in_progress_command() -> bool {
    is_retranscription_in_progress()
}
#[cfg(test)]
#[path = "retranscription_tests.rs"]
mod retained_tests;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_preserves_existing_fields_and_audio() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("metadata.json");
        std::fs::write(
            &path,
            r#"{"title":"fixture","custom":12,"detected_summary_language":"en"}"#,
        )
        .unwrap();
        write_retranscription_metadata(d.path(), "id", 1.0, "audio.wav").unwrap();
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(v["custom"], 12);
        assert_eq!(v["title"], "fixture");
        assert!(v.get("detected_summary_language").is_none());
    }
    #[test]
    fn find_audio_prefers_canonical_and_rejects_empty() {
        let d = tempfile::tempdir().unwrap();
        assert!(find_audio_file(d.path()).is_err());
        std::fs::write(d.path().join("audio.wav"), b"fixture").unwrap();
        assert_eq!(
            find_audio_file(d.path()).unwrap(),
            d.path().join("audio.wav")
        );
    }
}
