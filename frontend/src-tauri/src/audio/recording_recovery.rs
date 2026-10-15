//! Conservative disk recovery: bounded discovery and explicit, DB-only indexing.
//! Never writes/deletes source files, merges audio, promotes archives, or calls models.
#[cfg(test)]
mod tests;
use super::recording_saver::{MeetingMetadata, TranscriptSegment};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const MAX_FILE: usize = 8 * 1024 * 1024;
const MAX_ENTRIES: usize = 4096;
const MAX_SCAN_BYTES: usize = 32 * 1024 * 1024;
const PAGE_SIZE: usize = 100;

#[derive(Clone, Serialize)]
pub struct Candidate {
    pub id: String,
    pub fingerprint: String,
    pub folder_name: String,
    pub title: String,
    pub created_at: Option<String>,
    pub original_status: Option<String>,
    pub transcript_count: usize,
    pub audio_present: bool,
    pub checkpoint_count: usize,
    pub minutes_present: bool,
    pub can_restore: bool,
    pub warnings: Vec<String>,
}
#[derive(Serialize)]
pub struct Scan {
    pub candidates: Vec<Candidate>,
    pub next_cursor: Option<String>,
    pub warnings: Vec<String>,
}
#[derive(Debug, Serialize)]
pub struct Restored {
    pub meeting_id: String,
    pub already_registered: bool,
    pub warnings: Vec<String>,
}
#[derive(Deserialize)]
struct TranscriptFile {
    version: String,
    segments: Vec<TranscriptSegment>,
    total_segments: usize,
}
struct Loaded {
    candidate: Candidate,
    folder: PathBuf,
    metadata: Option<MeetingMetadata>,
    segments: Vec<TranscriptSegment>,
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn id(folder: &Path) -> Result<String, String> {
    Ok(format!(
        "recovery-{}",
        digest(
            folder
                .to_str()
                .ok_or("Unsupported recording path encoding")?
                .as_bytes()
        )
    ))
}
fn regular_bytes(path: &Path, budget: &mut usize) -> Result<Option<Vec<u8>>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("Expected a regular, non-symlink file".into());
    }
    if metadata.len() > MAX_FILE as u64 {
        return Err("File exceeds 8 MiB recovery bound".into());
    }
    if metadata.len() > *budget as u64 {
        return Err("SCAN_BYTE_LIMIT".into());
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Linux O_NOFOLLOW: do not follow a swapped final-component symlink.
        options.custom_flags(0x20000);
    }
    let file = options.open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Not a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take((MAX_FILE.min(*budget) + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_FILE {
        return Err("File exceeds 8 MiB recovery bound".into());
    }
    if bytes.len() > *budget {
        return Err("SCAN_BYTE_LIMIT".into());
    }
    *budget -= bytes.len();
    Ok(Some(bytes))
}
fn directories(root: &Path) -> Result<(Vec<PathBuf>, bool), String> {
    let mut dirs = Vec::new();
    let mut limited = false;
    for (n, entry) in fs::read_dir(root).map_err(|e| e.to_string())?.enumerate() {
        if n >= MAX_ENTRIES {
            limited = true;
            break;
        }
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            dirs.push(entry.path());
        }
    }
    dirs.sort();
    Ok((dirs, limited))
}
fn probe_file(
    folder: &Path,
    name: &str,
    hash: &mut Sha256,
    budget: &mut usize,
    warnings: &mut Vec<String>,
) -> Result<Option<Vec<u8>>, String> {
    hash.update(name.as_bytes());
    match regular_bytes(&folder.join(name), budget) {
        Ok(bytes) => {
            hash.update(if bytes.is_some() {
                b"present"
            } else {
                b"missing"
            });
            if let Some(bytes) = &bytes {
                hash.update(bytes);
            }
            Ok(bytes)
        }
        Err(e) if e == "SCAN_BYTE_LIMIT" => Err(e),
        Err(e) => {
            hash.update(e.as_bytes());
            warnings.push(format!("{name}: {e}"));
            Ok(None)
        }
    }
}
fn load(folder: PathBuf, budget: &mut usize) -> Result<Loaded, String> {
    let mut hash = Sha256::new();
    let mut warnings = Vec::new();
    let metadata_bytes = probe_file(&folder, "metadata.json", &mut hash, budget, &mut warnings)?;
    let transcript_bytes = probe_file(
        &folder,
        "transcripts.json",
        &mut hash,
        budget,
        &mut warnings,
    )?;
    let minutes_bytes = probe_file(
        &folder,
        "live-minutes.json",
        &mut hash,
        budget,
        &mut warnings,
    )?;
    let metadata = metadata_bytes.as_ref().and_then(|bytes| {
        match serde_json::from_slice::<MeetingMetadata>(bytes) {
            Ok(m)
                if m.version == "1.0"
                    && chrono::DateTime::parse_from_rfc3339(&m.created_at).is_ok() =>
            {
                Some(m)
            }
            _ => {
                warnings.push("metadata.json is corrupt or unsupported; original preserved".into());
                None
            }
        }
    });
    if metadata_bytes.is_none() {
        warnings.push("No readable metadata.json; registration is disabled".into());
    }
    let mut transcripts_ok = false;
    let segments = transcript_bytes
        .as_ref()
        .and_then(|bytes| {
            let parsed = serde_json::from_slice::<TranscriptFile>(bytes).ok()?;
            let mut ids = HashSet::new();
            let mut sequences = HashSet::new();
            if parsed.version != "1.0"
                || parsed.total_segments != parsed.segments.len()
                || parsed.segments.len() > 50000
            {
                return None;
            }
            for s in &parsed.segments {
                if s.id.is_empty()
                    || s.id.len() > 512
                    || !ids.insert(&s.id)
                    || !sequences.insert(s.sequence_id)
                    || !s.audio_start_time.is_finite()
                    || !s.audio_end_time.is_finite()
                    || !s.duration.is_finite()
                    || s.audio_start_time < 0.0
                    || s.audio_end_time < s.audio_start_time
                    || s.duration < 0.0
                {
                    return None;
                }
            }
            transcripts_ok = true;
            Some(parsed.segments)
        })
        .unwrap_or_default();
    if !transcripts_ok {
        warnings.push(
            "No valid saved transcripts; registration is disabled (no speech is invented)".into(),
        );
    }
    if let Some(bytes) = &minutes_bytes {
        if let Err(e) = crate::live_minutes::document::decode(bytes) {
            warnings.push(format!(
                "Saved minutes cannot be opened: {e}; original and history are preserved"
            ));
        }
    } else if folder.join("live-minutes-revisions").exists() {
        warnings.push("Revision archives exist without readable canonical minutes; archives will NOT be promoted".into());
    }
    let audio_present =
        fs::symlink_metadata(folder.join("audio.mp4")).is_ok_and(|m| m.is_file() && m.len() > 0);
    let mut checkpoint_count: usize = 0;
    let checkpoints = folder.join(".checkpoints");
    if fs::symlink_metadata(&checkpoints).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink()) {
        for (n, entry) in fs::read_dir(&checkpoints)
            .map_err(|e| e.to_string())?
            .enumerate()
        {
            if n >= MAX_ENTRIES {
                warnings.push("Checkpoint scan limit reached; count is a lower bound".into());
                break;
            }
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if super::checkpoint_audio::checkpoint_index(&name).is_some()
                && entry.file_type().map_err(|e| e.to_string())?.is_file()
            {
                checkpoint_count += 1;
            }
        }
    }
    hash.update(checkpoint_count.to_le_bytes());
    hash.update([audio_present as u8]);
    if checkpoint_count > 0 {
        warnings.push(
            "Audio checkpoints remain untouched; this action does not merge or validate audio"
                .into(),
        );
    }
    if metadata.as_ref().is_some_and(|m| m.status != "completed") {
        warnings.push(
            "Recording did not commit completion metadata; saved speech may be incomplete".into(),
        );
    }
    let folder_name = folder
        .file_name()
        .ok_or("Missing folder name")?
        .to_string_lossy()
        .into_owned();
    let candidate = Candidate {
        id: id(&folder)?,
        fingerprint: format!("{:x}", hash.finalize()),
        title: metadata
            .as_ref()
            .and_then(|m| m.meeting_name.clone())
            .unwrap_or_else(|| folder_name.clone()),
        created_at: metadata.as_ref().map(|m| m.created_at.clone()),
        original_status: metadata.as_ref().map(|m| m.status.clone()),
        folder_name,
        transcript_count: segments.len(),
        audio_present,
        checkpoint_count,
        minutes_present: minutes_bytes.is_some(),
        can_restore: metadata.is_some() && transcripts_ok,
        warnings,
    };
    Ok(Loaded {
        candidate,
        folder,
        metadata,
        segments,
    })
}
async fn known_folders(pool: &SqlitePool) -> Result<Vec<(String, PathBuf)>, String> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, folder_path FROM meetings WHERE folder_path IS NOT NULL LIMIT 10001",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    if rows.len() > 10000 {
        return Err("Registered meeting scan limit reached; no recovery was attempted".into());
    }
    Ok(rows
        .into_iter()
        .map(|(id, p)| {
            let path = PathBuf::from(p);
            (id, fs::canonicalize(&path).unwrap_or(path))
        })
        .collect())
}
async fn scan(pool: &SqlitePool, root: PathBuf, cursor: Option<String>) -> Result<Scan, String> {
    match fs::symlink_metadata(&root) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Scan {
                candidates: vec![],
                next_cursor: None,
                warnings: vec![],
            })
        }
        Err(e) => return Err(e.to_string()),
        Ok(_) => {}
    }
    let known = known_folders(pool).await?;
    tokio::task::spawn_blocking(move || {
        let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
        let (dirs, limited) = directories(&root)?;
        let mut report = Scan {
            candidates: Vec::new(),
            next_cursor: None,
            warnings: Vec::new(),
        };
        if limited {
            return Err(
                "Recording directory exceeds 4096-entry scan bound; original files are untouched"
                    .into(),
            );
        }
        let mut budget = MAX_SCAN_BYTES;
        let mut examined = 0;
        let remaining: Vec<_> = dirs
            .into_iter()
            .filter(|d| {
                cursor
                    .as_ref()
                    .is_none_or(|c| d.file_name().unwrap().to_string_lossy().as_ref() > c.as_str())
            })
            .collect();
        for folder in &remaining {
            if examined >= PAGE_SIZE || (examined > 0 && budget < 3 * MAX_FILE) {
                break;
            }
            examined += 1;
            report.next_cursor = Some(folder.file_name().unwrap().to_string_lossy().into_owned());
            if known.iter().any(|(_, p)| p == folder) {
                continue;
            }
            match load(folder.clone(), &mut budget) {
                Ok(loaded)
                    if loaded.metadata.is_some()
                        || fs::symlink_metadata(folder.join("metadata.json")).is_ok()
                        || loaded.candidate.transcript_count > 0
                        || loaded.candidate.minutes_present
                        || loaded.candidate.audio_present
                        || loaded.candidate.checkpoint_count > 0 =>
                {
                    report.candidates.push(loaded.candidate)
                }
                Ok(_) => {}
                Err(e) => {
                    report.warnings.push(format!(
                        "{}: {e}",
                        folder.file_name().unwrap().to_string_lossy()
                    ));
                }
            }
        }
        if examined == remaining.len() {
            report.next_cursor = None;
        }
        Ok(report)
    })
    .await
    .map_err(|e| e.to_string())?
}
async fn restore(
    pool: &SqlitePool,
    root: PathBuf,
    candidate_id: String,
    fingerprint: String,
) -> Result<Restored, String> {
    let loaded = tokio::task::spawn_blocking(move || -> Result<Loaded, String> {
        let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
        let (dirs, limited) = directories(&root)?;
        if limited {
            return Err("Recording directory exceeds scan bound".into());
        }
        let folder = dirs
            .into_iter()
            .find(|p| id(p).is_ok_and(|id| id == candidate_id))
            .ok_or("Recovery candidate no longer exists; rescan")?;
        let mut budget = MAX_SCAN_BYTES;
        let loaded = load(folder, &mut budget)?;
        if loaded.candidate.fingerprint != fingerprint {
            return Err("RECOVERY_CHANGED: saved data changed; rescan before adding".into());
        }
        if !loaded.candidate.can_restore {
            return Err("No valid metadata/transcripts; original files were preserved".into());
        }
        Ok(loaded)
    })
    .await
    .map_err(|e| e.to_string())??;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    // Acquire SQLite's writer lock before checking existing registrations.
    sqlx::query("UPDATE meetings SET id=id WHERE 0")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, folder_path FROM meetings WHERE folder_path IS NOT NULL LIMIT 10001",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    if rows.len() > 10000 {
        return Err("Registered meeting scan limit reached".into());
    }
    for (meeting_id, path) in rows {
        let path = PathBuf::from(path);
        if fs::canonicalize(&path).unwrap_or(path) == loaded.folder {
            tx.rollback().await.map_err(|e| e.to_string())?;
            return Ok(Restored {
                meeting_id,
                already_registered: true,
                warnings: loaded.candidate.warnings,
            });
        }
    }
    let meeting_id = format!("meeting-{}", loaded.candidate.id);
    let metadata = loaded.metadata.as_ref().unwrap();
    let created = chrono::DateTime::parse_from_rfc3339(&metadata.created_at)
        .map_err(|e| e.to_string())?
        .with_timezone(&chrono::Utc);
    sqlx::query(
        "INSERT INTO meetings (id,title,created_at,updated_at,folder_path) VALUES (?,?,?,?,?)",
    )
    .bind(&meeting_id)
    .bind(&loaded.candidate.title)
    .bind(created)
    .bind(chrono::Utc::now())
    .bind(loaded.folder.to_str().ok_or("Invalid recording path")?)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    for segment in &loaded.segments {
        // SQL IDs are namespaced projections; canonical evidence IDs on disk
        // remain byte-for-byte unchanged, including IDs shared across sessions.
        sqlx::query("INSERT INTO transcripts (id,meeting_id,transcript,timestamp,audio_start_time,audio_end_time,duration,speaker) VALUES (?,?,?,?,?,?,?,?)")
            .bind(format!("{}:{}", meeting_id, segment.id)).bind(&meeting_id).bind(&segment.text).bind(&segment.display_time)
            .bind(segment.audio_start_time).bind(segment.audio_end_time).bind(segment.duration).bind(&segment.speaker)
            .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(Restored {
        meeting_id,
        already_registered: false,
        warnings: loaded.candidate.warnings,
    })
}
#[tauri::command]
pub async fn api_scan_recording_recovery(
    state: tauri::State<'_, AppState>,
    cursor: Option<String>,
) -> Result<Scan, String> {
    let _guard = super::common::acquire_engine_lifecycle_lock().await;
    if super::recording_commands::is_recording().await {
        return Err("Finish recording before disk recovery".into());
    }
    scan(
        state.db_manager.pool(),
        super::recording_preferences::get_default_recordings_folder(),
        cursor,
    )
    .await
}
#[tauri::command]
pub async fn api_restore_recording(
    state: tauri::State<'_, AppState>,
    candidate_id: String,
    fingerprint: String,
) -> Result<Restored, String> {
    let _guard = super::common::acquire_engine_lifecycle_lock().await;
    if super::recording_commands::is_recording().await {
        return Err("Finish recording before disk recovery".into());
    }
    restore(
        state.db_manager.pool(),
        super::recording_preferences::get_default_recordings_folder(),
        candidate_id,
        fingerprint,
    )
    .await
}
