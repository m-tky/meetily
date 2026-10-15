use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;
use anyhow::Result;
use log::{info, warn, error};
use tauri::{AppHandle, Runtime, Emitter};
use tokio::sync::mpsc;
use serde::{Serialize, Deserialize};
use std::path::PathBuf;

use super::recording_state::AudioChunk;
use super::audio_processing::create_meeting_folder;
use super::incremental_saver::IncrementalAudioSaver;

/// Structured transcript segment for JSON export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub id: String,
    pub text: String,
    pub audio_start_time: f64, // Seconds from recording start
    pub audio_end_time: f64,   // Seconds from recording start
    pub duration: f64,          // Segment duration in seconds
    pub display_time: String,   // Formatted time for display like "[02:15]"
    pub confidence: f32,
    pub sequence_id: u64,
    /// Input source only. Missing on legacy/mixed/imported segments.
    #[serde(default)]
    pub speaker: Option<String>,
}

/// Meeting metadata structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingMetadata {
    pub version: String,
    pub meeting_id: Option<String>,
    pub meeting_name: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub duration_seconds: Option<f64>,
    pub devices: DeviceInfo,
    pub audio_file: String,
    pub transcript_file: String,
    pub sample_rate: u32,
    pub status: String,  // "recording", "completed", "error"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub microphone: Option<String>,
    pub system_audio: Option<String>,
}

/// New recording saver using incremental saving strategy
/// Session-bound transcript writer. It survives manager removal during Stop;
/// it can never redirect a late result into the next recording's folder.
#[derive(Clone)]
pub(crate) struct TranscriptSink {
    segments: Arc<Mutex<Vec<TranscriptSegment>>>,
    folder: Option<PathBuf>,
}

impl TranscriptSink {
    #[cfg(test)]
    pub(crate) fn test_in(folder: PathBuf) -> Self {
        Self { segments: Arc::new(Mutex::new(Vec::new())), folder: Some(folder) }
    }
    pub(crate) fn snapshot(&self) -> Result<Vec<TranscriptSegment>> {
        self.segments.lock().map(|s| s.clone())
            .map_err(|_| anyhow::anyhow!("Failed to lock transcript segments"))
    }
    pub(crate) fn folder(&self) -> Option<PathBuf> { self.folder.clone() }
    pub(crate) fn same_session(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.segments, &other.segments)
    }
    pub(crate) fn add(&self, segment: TranscriptSegment) -> Result<()> {
        let mut segments = self.segments.lock()
            .map_err(|_| anyhow::anyhow!("Failed to lock transcript segments"))?;
        if let Some(existing) = segments.iter_mut().find(|s| s.sequence_id == segment.sequence_id) {
            *existing = segment;
        } else {
            segments.push(segment);
        }
        // Independent input VADs can finish in a different order than they began.
        // Keep stable evidence IDs but persist/reload by the shared audio timeline.
        segments.sort_by(|a, b| a.audio_start_time.total_cmp(&b.audio_start_time)
            .then(a.sequence_id.cmp(&b.sequence_id)));
        if let Some(folder) = &self.folder {
            // Serialize under the same lock as upserts: older snapshots must not
            // race a newer write and replace it on disk.
            Self::write(folder, &segments)?;
        }
        Ok(())
    }

    fn write(folder: &PathBuf, segments: &[TranscriptSegment]) -> Result<()> {
        use std::io::Write;
        let bytes = serde_json::to_vec_pretty(&serde_json::json!({
            "version": "1.0", "segments": segments,
            "last_updated": chrono::Utc::now().to_rfc3339(),
            "total_segments": segments.len()
        }))?;
        let mut temp = tempfile::NamedTempFile::new_in(folder)?;
        temp.write_all(&bytes)?;
        temp.as_file().sync_all()?;
        temp.persist(folder.join("transcripts.json"))?;
        #[cfg(unix)]
        std::fs::File::open(folder)?.sync_all()?;
        Ok(())
    }
}

fn accumulate(
    mut receiver: mpsc::UnboundedReceiver<AudioChunk>,
    saver: Option<Arc<AsyncMutex<IncrementalAudioSaver>>>,
    save_audio: bool,
    warning: Arc<dyn Fn(String) + Send + Sync>,
) -> tokio::task::JoinHandle<usize> {
    tokio::spawn(async move {
        let mut count = 0;
        let mut reported = false;
        while let Some(chunk) = receiver.recv().await {
            count += 1;
            if save_audio {
                if let Some(saver) = &saver {
                    if let Err(e) = saver.lock().await.add_chunk(chunk) {
                        error!("Recording checkpoint write failed: {e}");
                        if !reported {
                            warning(format!("Recording checkpoint could not be saved; check disk space. Buffered audio will be retried: {e}"));
                            reported = true;
                        }
                    }
                }
            }
        }
        count
    })
}
impl Drop for RecordingSaver {
    fn drop(&mut self) {
        if let Some(task) = self.accumulation.take() {
            task.abort();
        }
    }
}
pub struct RecordingSaver {
    incremental_saver: Option<Arc<AsyncMutex<IncrementalAudioSaver>>>,
    meeting_folder: Option<PathBuf>,
    meeting_name: Option<String>,
    metadata: Option<MeetingMetadata>,
    transcript_segments: Arc<Mutex<Vec<TranscriptSegment>>>,
    accumulation: Option<tokio::task::JoinHandle<usize>>,
    warning: Arc<dyn Fn(String) + Send + Sync>,
}

impl RecordingSaver {
    pub fn new() -> Self {
        Self {
            incremental_saver: None,
            meeting_folder: None,
            meeting_name: None,
            metadata: None,
            transcript_segments: Arc::new(Mutex::new(Vec::new())),
            accumulation: None,
            warning: Arc::new(|message| error!("{message}")),
        }
    }

    /// Set the meeting name for this recording session
    pub fn set_meeting_name(&mut self, name: Option<String>) {
        self.meeting_name = name;
    }

    /// Set device information in metadata
    pub fn set_device_info(&mut self, mic_name: Option<String>, sys_name: Option<String>) {
        if let Some(ref mut metadata) = self.metadata {
            metadata.devices.microphone = mic_name;
            metadata.devices.system_audio = sys_name;

            // Write updated metadata to disk if folder exists
            if let Some(folder) = &self.meeting_folder {
                let metadata_clone = metadata.clone();
                if let Err(e) = self.write_metadata(folder, &metadata_clone) {
                    warn!("Failed to update metadata with device info: {}", e);
                    (self.warning)(format!("Recording device metadata could not be saved: {e}"));
                }
            }
        }
    }

    pub(crate) fn transcript_sink(&self) -> TranscriptSink {
        TranscriptSink { segments: self.transcript_segments.clone(), folder: self.meeting_folder.clone() }
    }

    /// Add or update a structured transcript segment and atomically persist it.
    pub fn add_transcript_segment(&self, segment: TranscriptSegment) {
        if let Err(error) = self.transcript_sink().add(segment) {
            error!("Failed to persist transcript update: {}", error);
        }
    }

    /// Legacy method for backward compatibility - converts text to basic segment
    pub fn add_transcript_chunk(&self, text: String) {
        let segment = TranscriptSegment {
            id: format!("seg_{}", chrono::Utc::now().timestamp_millis()),
            text,
            audio_start_time: 0.0,
            audio_end_time: 0.0,
            duration: 0.0,
            display_time: "[00:00]".to_string(),
            confidence: 1.0,
            sequence_id: 0,
            speaker: None,
        };
        self.add_transcript_segment(segment);
    }

    /// Start accumulation with optional incremental saving
    ///
    /// # Arguments
    /// * `auto_save` - If true, creates checkpoints and enables saving. If false, audio chunks are discarded.
    pub fn set_warning_callback(&mut self, callback: impl Fn(String) + Send + Sync + 'static) {
        self.warning = Arc::new(callback);
    }

    pub fn start_accumulation(
        &mut self,
        auto_save: bool,
        receiver: mpsc::UnboundedReceiver<AudioChunk>,
    ) -> Result<()> {
        let name = self.meeting_name.clone().ok_or_else(|| anyhow::anyhow!("Meeting name is required for durable saving"))?;
        // Fail before capture rather than silently running without a writable folder.
        self.initialize_meeting_folder(&name, auto_save)?;
        self.accumulation = Some(accumulate(receiver, self.incremental_saver.clone(), auto_save, self.warning.clone()));
        Ok(())
    }

    /// Initialize meeting folder structure and metadata
    ///
    /// # Arguments
    /// * `meeting_name` - Name of the meeting
    /// * `create_checkpoints` - Whether to create .checkpoints/ directory and IncrementalAudioSaver
    fn initialize_meeting_folder(&mut self, meeting_name: &str, create_checkpoints: bool) -> Result<()> {
        // Load preferences to get base recordings folder
        let base_folder = super::recording_preferences::get_default_recordings_folder();

        // Create meeting folder structure (with or without .checkpoints/ subdirectory)
        let meeting_folder = create_meeting_folder(&base_folder, meeting_name, create_checkpoints)?;

        // Only initialize incremental saver if checkpoints are needed (auto_save is true)
        if create_checkpoints {
            let incremental_saver = IncrementalAudioSaver::new(meeting_folder.clone(), 48000)?;
            self.incremental_saver = Some(Arc::new(AsyncMutex::new(incremental_saver)));
            info!("✅ Incremental audio saver initialized for meeting: {}", meeting_name);
        } else {
            info!("⚠️  Skipped incremental audio saver (auto-save disabled)");
        }

        // Create initial metadata
        let metadata = MeetingMetadata {
            version: "1.0".to_string(),
            meeting_id: None,  // Will be set by backend
            meeting_name: Some(meeting_name.to_string()),
            created_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
            duration_seconds: None,
            devices: DeviceInfo {
                microphone: None,  // Could be enhanced to store actual device names
                system_audio: None,
            },
            audio_file: if create_checkpoints { "audio.mp4".to_string() } else { "".to_string() },
            transcript_file: "transcripts.json".to_string(),
            sample_rate: 48000,
            status: "recording".to_string(),
        };

        // Write initial metadata.json
        self.write_metadata(&meeting_folder, &metadata)?;

        self.meeting_folder = Some(meeting_folder);
        self.metadata = Some(metadata);

        Ok(())
    }

    /// Write metadata.json to disk (atomic write with temp file)
    fn write_metadata(&self, folder: &PathBuf, metadata: &MeetingMetadata) -> Result<()> {
        use std::io::Write;
        let mut temp = tempfile::NamedTempFile::new_in(folder)?;
        serde_json::to_writer_pretty(&mut temp, metadata)?;
        temp.flush()?;
        temp.as_file().sync_all()?;
        temp.persist(folder.join("metadata.json"))?;
        #[cfg(unix)]
        std::fs::File::open(folder)?.sync_all()?;

        Ok(())
    }

    /// Write transcripts.json to disk (atomic write with temp file and validation)
    fn write_transcripts_json(&self, folder: &PathBuf) -> Result<()> {
        let segments = self.transcript_segments.lock()
            .map_err(|_| anyhow::anyhow!("Failed to lock transcript segments"))?;
        TranscriptSink::write(folder, &segments)
    }

    // in frontend/src-tauri/src/audio/recording_saver.rs
    pub fn get_stats(&self) -> (usize, u32) {
        if let Some(ref saver) = self.incremental_saver {
            if let Ok(guard) = saver.try_lock() {
                (guard.get_checkpoint_count() as usize, 48000)
            } else {
                (0, 48000)
            }
        } else {
            (0, 48000)
        }
    }

    /// Stop and save using incremental saving approach
    ///
    /// # Arguments
    /// * `app` - Tauri app handle for emitting events
    /// * `recording_duration` - Actual recording duration in seconds (from RecordingState)
    pub async fn stop_and_save<R: Runtime>(
        &mut self,
        app: &AppHandle<R>,
        recording_duration: Option<f64>
    ) -> Result<Option<String>, String> {
        info!("Stopping recording saver");

        // Upstream pipeline has ended and closed the sender. Drain every queued
        // chunk; never turn off a boolean before reading the final queue.
        // Borrow the handle so cancellation leaves it owned for Drop to abort.
        if let Some(task) = self.accumulation.as_mut() {
            task.await.map_err(|e| format!("Recording save queue failed: {e}"))?;
        }
        self.accumulation = None;

        // Check if incremental saver exists (indicates auto_save was enabled)
        let should_save_audio = self.incremental_saver.is_some();

        if !should_save_audio {
            info!("⚠️  No audio saver initialized (auto-save was disabled) - skipping audio finalization");
            if let Some(folder) = &self.meeting_folder {
                self.write_transcripts_json(folder).map_err(|e| e.to_string())?;
                if let Some(mut metadata) = self.metadata.clone() {
                    metadata.status = "completed".into();
                    metadata.completed_at = Some(chrono::Utc::now().to_rfc3339());
                    metadata.duration_seconds = recording_duration;
                    self.write_metadata(folder, &metadata).map_err(|e| e.to_string())?;
                }
            }
            return Ok(None);
        }

        // Finalize incremental saver (merge checkpoints into final audio.mp4)
        let final_audio_path = if let Some(saver_arc) = &self.incremental_saver {
            let mut saver = saver_arc.lock().await;
            match saver.finalize().await {
                Ok(path) => {
                    info!("✅ Successfully finalized audio: {}", path.display());
                    path
                }
                Err(e) => {
                    error!("❌ Failed to finalize incremental saver: {}", e);
                    return Err(format!("Failed to finalize audio: {}", e));
                }
            }
        } else {
            error!("No incremental saver initialized - cannot save recording");
            return Err("No incremental saver initialized".to_string());
        };

        // Save final transcripts.json with validation
        if let Some(folder) = &self.meeting_folder {
            if let Err(e) = self.write_transcripts_json(folder) {
                error!("❌ Failed to write final transcripts: {}", e);
                return Err(format!("Failed to save transcripts: {}", e));
            }

            // Verify transcripts were written correctly
            let transcript_path = folder.join("transcripts.json");
            if !transcript_path.exists() {
                error!("❌ Transcript file was not created at: {}", transcript_path.display());
                return Err("Transcript file verification failed".to_string());
            }
            info!("✅ Transcripts saved and verified at: {}", transcript_path.display());
        }

        // Update metadata to completed status with actual recording duration
        if let (Some(folder), Some(mut metadata)) = (&self.meeting_folder, self.metadata.clone()) {
            metadata.status = "completed".to_string();
            metadata.completed_at = Some(chrono::Utc::now().to_rfc3339());

            // Use actual recording duration from RecordingState (more accurate than transcript segments)
            // Falls back to last transcript segment if duration not provided
            metadata.duration_seconds = recording_duration.or_else(|| {
                if let Ok(segments) = self.transcript_segments.lock() {
                    segments.last().map(|seg| seg.audio_end_time)
                } else {
                    None
                }
            });

            if let Err(e) = self.write_metadata(folder, &metadata) {
                error!("❌ Failed to update metadata to completed: {}", e);
                return Err(format!("Failed to update metadata: {}", e));
            }

            info!("✅ Metadata updated with duration: {:?}s", metadata.duration_seconds);
        }

        // Only clean checkpoints AFTER canonical transcripts and completion
        // metadata are durable. A metadata/disk failure keeps recovery material.
        if let Some(folder) = &self.meeting_folder {
            if let Err(e) = super::incremental_saver::cleanup_checkpoints(folder.to_string_lossy().into()).await {
                (self.warning)(format!("Recording saved but checkpoint cleanup failed: {e}"));
            }
        }

        // Emit save event with audio and transcript paths
        let save_event = serde_json::json!({
            "audio_file": final_audio_path.to_string_lossy(),
            "transcript_file": self.meeting_folder.as_ref()
                .map(|f| f.join("transcripts.json").to_string_lossy().to_string()),
            "meeting_name": self.meeting_name,
            "meeting_folder": self.meeting_folder.as_ref()
                .map(|f| f.to_string_lossy().to_string())
        });

        if let Err(e) = app.emit("recording-saved", &save_event) {
            warn!("Failed to emit recording-saved event: {}", e);
        }

        // Keep the session snapshot until its final owner is dropped. A worker
        // still finishing after a shutdown timeout must not overwrite the full
        // transcript file with only its late tail after a shared-vector clear.

        Ok(Some(final_audio_path.to_string_lossy().to_string()))
    }

    /// Get the meeting folder path (for passing to backend)
    pub fn get_meeting_folder(&self) -> Option<&PathBuf> {
        self.meeting_folder.as_ref()
    }

    /// Get accumulated transcript segments (for reload sync)
    pub fn get_transcript_segments(&self) -> Vec<TranscriptSegment> {
        if let Ok(segments) = self.transcript_segments.lock() {
            segments.clone()
        } else {
            Vec::new()
        }
    }

    /// Get meeting name (for reload sync)
    pub fn get_meeting_name(&self) -> Option<String> {
        self.meeting_name.clone()
    }
}

impl Default for RecordingSaver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod transcript_sink_tests {
    use super::*;
    fn segment(sequence: u64, text: &str) -> TranscriptSegment {
        TranscriptSegment {
            id: format!("seg_{sequence}"), text: text.into(),
            audio_start_time: 1.0, audio_end_time: 2.0, duration: 1.0,
            display_time: "00:01".into(), confidence: 0.0, sequence_id: sequence, speaker: None,
        }
    }
    #[test]
    fn source_metadata_survives_late_lane_results_and_json_reload() {
        let dir = tempfile::tempdir().unwrap();
        let sink = TranscriptSink::test_in(dir.path().into());
        let mut mic = segment(1, "same utterance");
        mic.speaker = Some("mic".into()); mic.audio_start_time = 4.0; mic.audio_end_time = 5.0;
        let mut pc = segment(2, "same utterance"); pc.speaker = Some("system".into());
        sink.add(mic).unwrap(); sink.add(pc).unwrap();
        let snapshot = sink.snapshot().unwrap();
        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot[0].sequence_id, 2, "late result is ordered by audio, not ASR completion");
        assert_eq!(snapshot[1].speaker.as_deref(), Some("mic"));
        let saved = disk(&dir.path().into());
        let reloaded: Vec<TranscriptSegment> = serde_json::from_value(saved["segments"].clone()).unwrap();
        assert_eq!(reloaded[0].speaker.as_deref(), Some("system"));
        assert_eq!(reloaded[1].text, "same utterance");
    }
    fn disk(folder: &PathBuf) -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(folder.join("transcripts.json")).unwrap()).unwrap()
    }
    #[tokio::test]
    async fn accumulation_drains_all_queued_chunks_without_start_or_stop_flag_races() {
        let (tx, rx) = mpsc::unbounded_channel();
        for n in 0..1000 { tx.send(AudioChunk { capture_timestamp: None, data: vec![0.0; 48], sample_rate: 48000, timestamp: 0.0,
            chunk_id: n, device_type: super::super::recording_state::DeviceType::System }).unwrap(); }
        drop(tx);
        assert_eq!(accumulate(rx, None, false, Arc::new(|_| {})).await.unwrap(), 1000);
    }
    #[tokio::test]
    async fn checkpoint_failure_warns_and_retained_buffer_can_finalize_after_repair() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let dir = tempfile::tempdir().unwrap(); let folder = dir.path().to_path_buf();
        std::fs::create_dir(folder.join(".checkpoints")).unwrap();
        let saver = Arc::new(AsyncMutex::new(IncrementalAudioSaver::new(folder.clone(), 48000).unwrap()));
        std::fs::remove_dir(folder.join(".checkpoints")).unwrap();
        let warnings = Arc::new(AtomicUsize::new(0)); let warned = warnings.clone();
        let (tx, rx) = mpsc::unbounded_channel();
        tx.send(AudioChunk { capture_timestamp: None, data: vec![0.0; 30 * 48000], sample_rate: 48000, timestamp: 0.0,
            chunk_id: 1, device_type: super::super::recording_state::DeviceType::System }).unwrap();
        drop(tx);
        assert_eq!(accumulate(rx, Some(saver.clone()), true, Arc::new(move |_| { warned.fetch_add(1, Ordering::SeqCst); })).await.unwrap(), 1);
        assert_eq!(warnings.load(Ordering::SeqCst), 1);
        std::fs::create_dir(folder.join(".checkpoints")).unwrap();
        let output = saver.lock().await.finalize().await.unwrap();
        assert!(std::fs::metadata(output).unwrap().len() > 0);
        assert!(folder.join(".checkpoints/audio_chunk_000.wav").exists());
    }
    #[test]
    fn same_meeting_title_does_not_reuse_existing_folder() {
        let dir = tempfile::tempdir().unwrap();
        let first = create_meeting_folder(&dir.path().to_path_buf(), "same title", false).unwrap();
        std::fs::write(first.join("metadata.json"), b"original").unwrap();
        let second = create_meeting_folder(&dir.path().to_path_buf(), "same title", false).unwrap();
        assert_ne!(first, second); assert_eq!(std::fs::read(first.join("metadata.json")).unwrap(), b"original");
    }
    #[test]
    fn late_tail_survives_removed_owner_without_writing_next_session() {
        let old_dir = tempfile::tempdir().unwrap();
        let next_dir = tempfile::tempdir().unwrap();
        let mut saver = RecordingSaver::new();
        saver.meeting_folder = Some(old_dir.path().to_path_buf());
        let old_sink = saver.transcript_sink();
        drop(saver); // Stop has removed the global recording manager.
        let mut next = RecordingSaver::new();
        next.meeting_folder = Some(next_dir.path().to_path_buf());
        next.transcript_sink().add(segment(1, "次の会議")).unwrap();
        old_sink.add(segment(1, "停止直前の発言")).unwrap();
        assert_eq!(disk(&old_dir.path().to_path_buf())["segments"][0]["text"], "停止直前の発言");
        assert_eq!(disk(&next_dir.path().to_path_buf())["segments"][0]["text"], "次の会議");
    }
    #[test]
    fn upserts_are_atomic_and_keep_other_segments() {
        let dir = tempfile::tempdir().unwrap();
        let mut saver = RecordingSaver::new();
        saver.meeting_folder = Some(dir.path().to_path_buf());
        let sink = saver.transcript_sink();
        sink.add(segment(1, "旧文")).unwrap();
        sink.add(segment(2, "末尾")).unwrap();
        sink.clone().add(segment(1, "更新文")).unwrap();
        let value = disk(&dir.path().to_path_buf());
        assert_eq!(value["version"], "1.0");
        assert_eq!(value["total_segments"], 2);
        assert_eq!(value["segments"][0]["text"], "更新文");
        assert_eq!(value["segments"][1]["text"], "末尾");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn disk_failure_retains_memory_and_next_write_recovers_all_segments() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("missing");
        let mut saver = RecordingSaver::new();
        saver.meeting_folder = Some(folder.clone());
        let sink = saver.transcript_sink();
        assert!(sink.add(segment(1, "保存再試行用")).is_err());
        assert_eq!(saver.get_transcript_segments().len(), 1);
        std::fs::create_dir(&folder).unwrap();
        sink.add(segment(2, "追加発言")).unwrap();
        assert_eq!(disk(&folder)["total_segments"], 2);
    }
}
