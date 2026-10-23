use std::path::PathBuf;
use anyhow::{Result, anyhow};
use log::{info, warn, error};
use super::checkpoint_audio::{write_float_checkpoint, checkpoint_index};
use super::recording_state::AudioChunk;
use serde::{Serialize, Deserialize};

use super::ffmpeg::find_ffmpeg_path;

/// Audio data without device type (we only store mixed audio)
#[derive(Clone)]
struct AudioData {
    data: Vec<f32>,
    // sample_rate: u32,
}

/// Incremental audio saver that writes checkpoints every 30 seconds
/// to minimize memory usage and enable crash recovery
pub struct IncrementalAudioSaver {
    checkpoint_buffer: Vec<AudioData>,
    checkpoint_interval_samples: usize,  // 30s at 48kHz = 1,440,000 samples
    checkpoint_count: u32,
    checkpoints_dir: PathBuf,
    meeting_folder: PathBuf,
    sample_rate: u32,
}

impl IncrementalAudioSaver {
    /// Create a new incremental saver
    ///
    /// # Arguments
    /// * `meeting_folder` - Path to the meeting folder (contains .checkpoints/)
    /// * `sample_rate` - Sample rate of audio (typically 48000)
    pub fn new(meeting_folder: PathBuf, sample_rate: u32) -> Result<Self> {
        let checkpoints_dir = meeting_folder.join(".checkpoints");

        // Verify checkpoints directory exists
        if !checkpoints_dir.exists() {
            return Err(anyhow!("Checkpoints directory does not exist: {}", checkpoints_dir.display()));
        }

        Ok(Self {
            checkpoint_buffer: Vec::new(),
            checkpoint_interval_samples: sample_rate as usize * 30, // 30 seconds
            checkpoint_count: 0,
            checkpoints_dir,
            meeting_folder,
            sample_rate,
        })
    }

    /// Add an audio chunk to the buffer
    /// Automatically saves a checkpoint when buffer reaches 30 seconds
    pub fn add_chunk(&mut self, chunk: AudioChunk) -> Result<()> {
        let audio_data = AudioData {
            data: chunk.data,
            // sample_rate: chunk.sample_rate,
        };

        self.checkpoint_buffer.push(audio_data);

        // Calculate total samples in buffer
        let total_samples: usize = self.checkpoint_buffer
            .iter()
            .map(|c| c.data.len())
            .sum();

        // Save checkpoint when buffer reaches threshold (30 seconds)
        if total_samples >= self.checkpoint_interval_samples {
            self.save_checkpoint()?;
            self.checkpoint_buffer.clear();
        }

        Ok(())
    }

    /// Save current buffer as a checkpoint file
    fn save_checkpoint(&mut self) -> Result<()> {
        // Concatenate all chunks in buffer
        let audio_data: Vec<f32> = self.checkpoint_buffer
            .iter()
            .flat_map(|c| &c.data)
            .cloned()
            .collect();

        if audio_data.is_empty() {
            warn!("Attempted to save empty checkpoint, skipping");
            return Ok(());
        }

        // Generate checkpoint filename
        let checkpoint_path = self.checkpoints_dir
            .join(format!("audio_chunk_{:03}.wav", self.checkpoint_count));

        // Publish only a fully written/synced float checkpoint. Partial output
        // must not look recoverable after a crash. Create-new in a private
        // directory plus no-clobber publication protects prior checkpoints.
        let temp = tempfile::Builder::new().prefix(".encoding-").tempdir_in(&self.checkpoints_dir)?;
        let encoded = temp.path().join("checkpoint.wav");
        // The writer flushes and syncs its writable handle on every platform.
        write_float_checkpoint(&encoded, &audio_data, self.sample_rate)?;
        // Atomic no-clobber publication on the same filesystem. TempDir cleans
        // the private encoder copy; the published hard link owns the data.
        std::fs::hard_link(&encoded, &checkpoint_path)?;
        #[cfg(unix)] std::fs::File::open(&self.checkpoints_dir)?.sync_all()?;

        let duration_seconds = audio_data.len() as f32 / self.sample_rate as f32;
        self.checkpoint_count += 1;

        info!("Saved checkpoint {}: {:.2}s of audio ({} samples)",
              self.checkpoint_count,
              duration_seconds,
              audio_data.len());

        Ok(())
    }

    /// Finalize the recording: save final checkpoint, merge all checkpoints, cleanup
    ///
    /// Returns the path to the final merged audio.mp4 file
    pub async fn finalize(&mut self) -> Result<PathBuf> {
        info!("Finalizing incremental recording...");

        // Save final buffer if not empty
        if !self.checkpoint_buffer.is_empty() {
            info!("Saving final checkpoint with remaining {} chunks", self.checkpoint_buffer.len());
            self.save_checkpoint()?;
            self.checkpoint_buffer.clear();
        }

        if self.checkpoint_count == 0 {
            return Err(anyhow!("No audio checkpoints to merge - recording may have failed"));
        }

        // Concatenate lossless checkpoints and encode the final AAC once.
        let final_audio_path = self.meeting_folder.join("audio.mp4");
        self.merge_checkpoints(&final_audio_path).await?;

        // Keep checkpoints until RecordingSaver has also committed transcripts
        // and completed metadata. Final audio alone is not a complete save.

        info!("Finalized recording: {}", final_audio_path.display());

        Ok(final_audio_path)
    }

    /// Merge all checkpoint files into final audio.mp4 using FFmpeg concat
    /// Concatenate float32 checkpoints and perform the sole lossy AAC encode.
    async fn merge_checkpoints(&self, output: &PathBuf) -> Result<()> {
        info!("Merging {} checkpoints into final audio file...", self.checkpoint_count);

        // Create concat list file for FFmpeg
        let list_file = self.checkpoints_dir.join("concat_list.txt");
        let mut list_content = String::new();
        let mut paths = Vec::new();

        for i in 0..self.checkpoint_count {
            let wav = self.checkpoints_dir.join(format!("audio_chunk_{:03}.wav", i));
            let legacy = self.checkpoints_dir.join(format!("audio_chunk_{:03}.mp4", i));
            if wav.exists() && legacy.exists() { return Err(anyhow!("Duplicate checkpoint index {i}; originals retained")); }
            let checkpoint_path = if wav.exists() { wav } else { legacy };

            // Verify checkpoint exists
            if !checkpoint_path.exists() {
                return Err(anyhow!("Checkpoint file missing: {}", checkpoint_path.display()));
            }

            // Use absolute path for FFmpeg (required for safe mode)
            paths.push(checkpoint_path.clone());
            let abs_path = checkpoint_path.canonicalize()?;
            let path = abs_path.to_str().ok_or_else(|| anyhow!("Invalid UTF-8 checkpoint path"))?;
            if path.contains(['\n', '\r']) {
                return Err(anyhow!("Checkpoint path contains an unsupported line break"));
            }
            let escaped = path.replace('\'', "'\\''");
            list_content.push_str(&format!("file '{escaped}'\n"));
        }

        std::fs::write(&list_file, list_content)?;

        let ffmpeg_path = find_ffmpeg_path()
            .ok_or_else(|| anyhow!("FFmpeg not found. Please install FFmpeg to finalize recordings."))?;
        info!("Using FFmpeg at: {:?}", ffmpeg_path);

        // Run FFmpeg concat command
        // WAV transport retains every sample; continuous PTS avoids per-file time rounding.
        
        let parent = output.parent().ok_or_else(|| anyhow!("Missing audio output parent"))?;
        let temp = tempfile::Builder::new().prefix(".merging-").suffix(".mp4").tempfile_in(parent)?;
        let mut command = tokio::process::Command::new(ffmpeg_path);
        command.kill_on_drop(true);
        
        command.args(&[
            "-f", "concat",          // Use concat demuxer
            "-safe", "0",            // Allow absolute paths
            "-i", list_file.to_str().unwrap(),
        ]).args(merge_codec_args(&paths, self.sample_rate)?).args(&[
            "-y",                    // Overwrite output file
            temp.path().to_str().ok_or_else(|| anyhow!("Invalid temporary audio path"))?
        ]);

        // Hide console window on Windows to prevent CMD popup during finalization
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let ffmpeg_output = tokio::time::timeout(std::time::Duration::from_secs(120), command.output())
            .await.map_err(|_| anyhow!("FFmpeg merge timed out; checkpoints and previous audio were retained"))??;

        if !ffmpeg_output.status.success() {
            let stderr = String::from_utf8_lossy(&ffmpeg_output.stderr);
            error!("FFmpeg merge failed: {}", stderr);
            return Err(anyhow!("FFmpeg concat failed: {}", stderr));
        }

        // Verify output file was created
        if std::fs::metadata(temp.path())?.len() == 0 {
            return Err(anyhow!("Merged audio file is empty"));
        }
        // Keep write access for Windows FlushFileBuffers; do not reopen read-only.
        temp.as_file().sync_all()?;
        temp.persist(output)?;
        #[cfg(unix)] std::fs::File::open(parent)?.sync_all()?;

        info!("Successfully merged {} checkpoints → {}",
              self.checkpoint_count, output.display());

        Ok(())
    }

    /// Get the meeting folder path
    pub fn get_meeting_folder(&self) -> &PathBuf {
        &self.meeting_folder
    }

    /// Get current checkpoint count
    pub fn get_checkpoint_count(&self) -> u32 {
        self.checkpoint_count
    }
}

// New recordings concatenate lossless PCM and encode AAC once. Legacy AAC
// recovery stays compatible but is explicitly partial: its per-file priming
// cannot be removed by stream-copy. Never silently mix incompatible codecs.
fn merge_codec_args(paths: &[PathBuf], sample_rate: u32) -> Result<Vec<String>> {
    if paths.is_empty() { return Err(anyhow!("No checkpoints")); }
    if paths.iter().all(|p| p.extension().is_some_and(|e| e == "mp4")) {
        return Ok(vec!["-c".into(), "copy".into()]);
    }
    if !paths.iter().all(|p| p.extension().is_some_and(|e| e == "wav")) || sample_rate == 0 {
        return Err(anyhow!("Mixed checkpoint codecs or invalid sample rate; originals retained"));
    }
    Ok(["-af", "asetpts=N/SR/TB", "-c:a", "aac", "-b:a", "192k",
        "-profile:a", "aac_low", "-movflags", "+faststart", "-movie_timescale"]
        .into_iter().map(str::to_string).chain([sample_rate.to_string()]).collect())
}

/// Audio recovery status for transcript recovery feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioRecoveryStatus {
    pub status: String, // "success" | "partial" | "failed" | "none"
    pub chunk_count: u32,
    pub estimated_duration_seconds: f64,
    pub audio_file_path: Option<String>,
    pub message: String,
}

/// Recover audio from checkpoint files
/// This is called by the transcript recovery system to merge audio chunks after a crash
#[tauri::command]
pub async fn recover_audio_from_checkpoints(
    meeting_folder: String,
    sample_rate: u32
) -> Result<AudioRecoveryStatus, String> {
    info!("Starting audio recovery for folder: {}", meeting_folder);

    let folder_path = PathBuf::from(&meeting_folder);
    let checkpoints_dir = folder_path.join(".checkpoints");
    let existing = folder_path.join("audio.mp4");
    if let Ok(metadata) = std::fs::symlink_metadata(&existing) {
        let regular = metadata.is_file() && metadata.len() > 0;
        return Ok(AudioRecoveryStatus {
            status: if regular { "existing" } else { "failed" }.into(), chunk_count: 0, estimated_duration_seconds: 0.0,
            audio_file_path: if regular { existing.to_str().map(str::to_string) } else { None },
            message: "Existing audio file was preserved without validation or replacement; checkpoints were kept".into(),
        });
    }

    // Check if checkpoints directory exists
    if !checkpoints_dir.exists() {
        info!("No checkpoints directory found at: {}", checkpoints_dir.display());
        return Ok(AudioRecoveryStatus {
            status: "none".to_string(),
            chunk_count: 0,
            estimated_duration_seconds: 0.0,
            audio_file_path: None,
            message: "No audio checkpoints found".to_string(),
        });
    }

    // Bounded, numeric ordering; do not silently omit unreadable entries,
    // follow symlinks, or assemble gaps as if a complete recording.
    let entries: Vec<_> = std::fs::read_dir(&checkpoints_dir).map_err(|e| e.to_string())?
        .take(4097).collect::<std::io::Result<Vec<_>>>().map_err(|e| e.to_string())?;
    if entries.len() > 4096 { return Err("Checkpoint enumeration exceeds bound; originals were retained".into()); }
    let mut numbered = Vec::new();
    for entry in entries {
        if !entry.file_type().map_err(|e| e.to_string())?.is_file() { continue; }
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(index) = checkpoint_index(&name) {
            numbered.push((index, entry));
        }
    }
    numbered.sort_by_key(|(index, _)| *index);
    if numbered.iter().enumerate().any(|(expected, (index, _))| expected != *index) {
        return Err("Checkpoint sequence contains gaps or duplicates; originals were retained".into());
    }
    let checkpoint_files: Vec<_> = numbered.into_iter().map(|(_, entry)| entry).collect();
    if checkpoint_files.is_empty() {
        return Ok(AudioRecoveryStatus { status: "none".into(), chunk_count: 0,
            estimated_duration_seconds: 0.0, audio_file_path: None,
            message: "No regular numbered checkpoint files found".into() });
    }

    let paths: Vec<_> = checkpoint_files.iter().map(|e| e.path()).collect();
    let legacy = paths.iter().all(|p| p.extension().is_some_and(|e| e == "mp4"));
    let codec_args = merge_codec_args(&paths, sample_rate).map_err(|e| e.to_string())?;
    let chunk_count = checkpoint_files.len() as u32;
    let estimated_duration = (chunk_count as f64) * 30.0; // 30 seconds per chunk

    info!("Found {} checkpoint files, estimated duration: {:.2}s", chunk_count, estimated_duration);

    // Create FFmpeg concat file
    let concat_file = tempfile::NamedTempFile::new_in(&checkpoints_dir).map_err(|e| e.to_string())?;
    let concat_file_path = concat_file.path();
    let mut concat_content = String::new();

    for entry in &checkpoint_files {
        let path = entry.path().canonicalize()
            .map_err(|e| format!("Failed to canonicalize path: {}", e))?;
        let path = path.to_str().ok_or("Unsupported checkpoint path encoding")?;
        if path.contains(['\n', '\r']) { return Err("Unsupported line break in checkpoint path".into()); }
        let escaped = path.replace('\'', "'\\''");
        concat_content.push_str(&format!("file '{escaped}'\n"));
    }

    std::fs::write(&concat_file_path, concat_content)
        .map_err(|e| format!("Failed to write concat file: {}", e))?;

    // Run FFmpeg to merge chunks
    let output_path = folder_path.join("audio.mp4");
    let output_path_str = output_path.to_str()
        .ok_or("Invalid output path")?
        .to_string();

    let ffmpeg_path = find_ffmpeg_path()
        .ok_or_else(|| "FFmpeg not found. Please install FFmpeg to recover audio.".to_string())?;
    info!("Using FFmpeg at: {:?}", ffmpeg_path);

    let temporary_audio = tempfile::Builder::new().prefix(".recovering-").suffix(".mp4")
        .tempfile_in(&folder_path).map_err(|e| e.to_string())?;
    let mut command = tokio::process::Command::new(ffmpeg_path);
    command.kill_on_drop(true);

    command.args(&[
        "-f", "concat",
        "-safe", "0",
        "-i", concat_file_path.to_str().unwrap(),
    ]).args(codec_args).args(&[
        "-y", // Only overwrite our own reserved temporary output
        temporary_audio.path().to_str().ok_or("Invalid temporary audio path")?
    ]);

    // Hide console window on Windows
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let ffmpeg_result = tokio::time::timeout(std::time::Duration::from_secs(120), command.output())
        .await.map_err(|_| "Audio recovery timed out; originals were retained".to_string())?;

    match ffmpeg_result {
        Ok(output) if output.status.success() => {
            if temporary_audio.as_file().metadata().map_err(|e| e.to_string())?.len() == 0 {
                return Err("Recovery produced empty audio; checkpoints were retained".into());
            }
            temporary_audio.as_file().sync_all().map_err(|e| e.to_string())?;
            temporary_audio.persist_noclobber(&output_path).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            std::fs::File::open(&folder_path).and_then(|f| f.sync_all()).map_err(|e| e.to_string())?;

            info!("Successfully recovered audio: {}", output_path_str);

            Ok(AudioRecoveryStatus {
                status: if legacy { "partial" } else { "success" }.to_string(),
                chunk_count,
                estimated_duration_seconds: estimated_duration,
                audio_file_path: Some(output_path_str),
                message: if legacy {
                    format!("Recovered {chunk_count} legacy AAC checkpoints by stream-copy; encoder padding/timing may remain. Originals retained")
                } else { format!("Recovered {chunk_count} float32 checkpoints; originals retained") },
            })
        }
        Ok(output) => {
            let error = String::from_utf8_lossy(&output.stderr);
            error!("FFmpeg recovery failed: {}", error);
            Ok(AudioRecoveryStatus {
                status: "failed".to_string(),
                chunk_count,
                estimated_duration_seconds: estimated_duration,
                audio_file_path: None,
                message: format!("FFmpeg failed: {}", error),
            })
        }
        Err(e) => {
            error!("Failed to run FFmpeg: {}", e);
            Ok(AudioRecoveryStatus {
                status: "failed".to_string(),
                chunk_count,
                estimated_duration_seconds: estimated_duration,
                audio_file_path: None,
                message: format!("Failed to run FFmpeg: {}", e),
            })
        }
    }
}

/// Clean up checkpoint files after successful recording or recovery
/// This command is called by the frontend after successful save to clean up checkpoint files
#[tauri::command]
pub async fn cleanup_checkpoints(meeting_folder: String) -> Result<(), String> {
    info!("Cleaning up checkpoints for folder: {}", meeting_folder);

    let folder_path = PathBuf::from(&meeting_folder);
    let checkpoints_dir = folder_path.join(".checkpoints");

    if checkpoints_dir.exists() {
        std::fs::remove_dir_all(&checkpoints_dir)
            .map_err(|e| format!("Failed to remove checkpoints directory: {}", e))?;
        info!("Successfully cleaned up checkpoints directory");
    } else {
        info!("No checkpoints directory to clean up");
    }

    Ok(())
}

/// Check if a meeting folder has audio checkpoint files
/// Returns true if .checkpoints/ directory exists and contains .mp4 files
#[tauri::command]
pub async fn has_audio_checkpoints(meeting_folder: String) -> Result<bool, String> {
    let folder_path = PathBuf::from(&meeting_folder);
    let checkpoints_dir = folder_path.join(".checkpoints");

    // Check if checkpoints directory exists
    if !checkpoints_dir.exists() {
        return Ok(false);
    }

    // Recognize both the new float32 WAV and legacy AAC checkpoint names.
    let has_mp4_files = std::fs::read_dir(&checkpoints_dir)
        .map_err(|e| format!("Failed to read checkpoints directory: {}", e))?
        .filter_map(|entry| entry.ok())
        .any(|entry| {
            checkpoint_index(&entry.file_name().to_string_lossy()).is_some()
        });

    Ok(has_mp4_files)
}

#[cfg(test)]
mod boundary_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use super::super::recording_state::DeviceType;

    #[tokio::test]
    async fn test_checkpoint_creation() {
        // Spaces and apostrophes in user-selected recording roots remain usable.
        let temp_dir = tempfile::Builder::new().prefix("meetily's test-").tempdir().unwrap();
        let meeting_folder = temp_dir.path().join("Test_Meeting");
        std::fs::create_dir_all(&meeting_folder).unwrap();
        std::fs::create_dir_all(meeting_folder.join(".checkpoints")).unwrap();

        let mut saver = IncrementalAudioSaver::new(
            meeting_folder.clone(),
            48000
        ).unwrap();

        // Add 60 seconds worth of audio (should create 2 checkpoints)
        for i in 0..120 {  // 120 chunks of 0.5s each
            let chunk = AudioChunk { capture_timestamp: None,
                data: vec![0.5f32; 24000],  // 0.5s at 48kHz
                sample_rate: 48000,
                timestamp: i as f64 * 0.5,  // timestamp in seconds
                chunk_id: i as u64,
                device_type: DeviceType::Microphone,
            };
            saver.add_chunk(chunk).unwrap();
        }

        // Verify 2 checkpoints created
        assert_eq!(saver.checkpoint_count, 2);

        // Finalize and verify merge
        let final_path = saver.finalize().await.unwrap();
        assert!(final_path.exists());

        // Recovery material survives finalization until metadata commit.
        assert!(meeting_folder.join(".checkpoints").exists());
    }

    #[tokio::test]
    async fn failed_merge_retains_previous_audio_and_checkpoints() {
        let dir = tempdir().unwrap(); let folder = dir.path().to_path_buf();
        std::fs::create_dir(folder.join(".checkpoints")).unwrap();
        std::fs::write(folder.join(".checkpoints/audio_chunk_000.mp4"), b"invalid synthetic checkpoint").unwrap();
        std::fs::write(folder.join("audio.mp4"), b"previous complete audio").unwrap();
        let mut saver = IncrementalAudioSaver::new(folder.clone(), 48000).unwrap();
        saver.checkpoint_count = 1;
        assert!(saver.finalize().await.is_err());
        assert_eq!(std::fs::read(folder.join("audio.mp4")).unwrap(), b"previous complete audio");
        assert!(folder.join(".checkpoints/audio_chunk_000.mp4").exists());
    }
    #[tokio::test]
    async fn legacy_recovery_preserves_existing_audio_and_checkpoints() {
        let dir = tempdir().unwrap(); let folder = dir.path().to_path_buf();
        std::fs::create_dir(folder.join(".checkpoints")).unwrap();
        std::fs::write(folder.join(".checkpoints/audio_chunk_000.mp4"), b"synthetic checkpoint").unwrap();
        std::fs::write(folder.join("audio.mp4"), b"original audio").unwrap();
        let result = recover_audio_from_checkpoints(folder.to_string_lossy().into(), 48000).await.unwrap();
        assert_eq!(result.status, "existing");
        assert_eq!(std::fs::read(folder.join("audio.mp4")).unwrap(), b"original audio");
        assert_eq!(std::fs::read(folder.join(".checkpoints/audio_chunk_000.mp4")).unwrap(), b"synthetic checkpoint");
    }
    #[tokio::test]
    async fn legacy_failed_merge_does_not_publish_partial_audio() {
        let dir = tempdir().unwrap(); let folder = dir.path().to_path_buf();
        std::fs::create_dir(folder.join(".checkpoints")).unwrap();
        std::fs::write(folder.join(".checkpoints/audio_chunk_000.mp4"), b"invalid synthetic checkpoint").unwrap();
        let result = recover_audio_from_checkpoints(folder.to_string_lossy().into(), 48000).await.unwrap();
        assert_eq!(result.status, "failed");
        assert!(!folder.join("audio.mp4").exists());
        assert_eq!(std::fs::read(folder.join(".checkpoints/audio_chunk_000.mp4")).unwrap(), b"invalid synthetic checkpoint");
        assert_eq!(std::fs::read_dir(folder.join(".checkpoints")).unwrap().count(),1);
    }
    #[tokio::test]
    async fn test_empty_recording() {
        let temp_dir = tempdir().unwrap();
        let meeting_folder = temp_dir.path().join("Empty_Test");
        std::fs::create_dir_all(&meeting_folder).unwrap();
        std::fs::create_dir_all(meeting_folder.join(".checkpoints")).unwrap();

        let mut saver = IncrementalAudioSaver::new(
            meeting_folder.clone(),
            48000
        ).unwrap();

        // Try to finalize without adding any chunks
        let result = saver.finalize().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No audio checkpoints"));
    }
}
