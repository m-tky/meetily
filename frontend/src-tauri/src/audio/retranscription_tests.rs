//! Retained upstream batch/storage regressions, independent of ASR engine.
use super::*;
#[test]
fn test_create_transcript_segments_empty() {
    assert!(create_transcript_segments(&[]).is_empty());
}
#[test]
fn test_create_transcript_segments_single() {
    let s = create_transcript_segments(&[("Hello world".into(), 0.0, 1500.0)]);
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].text, "Hello world");
    assert_eq!(s[0].audio_start_time, Some(0.0));
    assert_eq!(s[0].audio_end_time, Some(1.5));
    assert_eq!(s[0].duration, Some(1.5));
}
#[test]
fn test_create_transcript_segments_multiple() {
    let s = create_transcript_segments(&[
        ("First segment".into(), 0.0, 2000.0),
        ("Second segment".into(), 3000.0, 5000.0),
        ("Third segment".into(), 6500.0, 8000.0),
    ]);
    assert_eq!(s.len(), 3);
    for (i, text, start, end) in [
        (0, "First segment", 0.0, 2.0),
        (1, "Second segment", 3.0, 5.0),
        (2, "Third segment", 6.5, 8.0),
    ] {
        assert_eq!(s[i].text, text);
        assert_eq!(s[i].audio_start_time, Some(start));
        assert_eq!(s[i].audio_end_time, Some(end));
        assert_eq!(s[i].duration, Some(end - start));
    }
}
#[test]
fn test_create_transcript_segments_trims_whitespace() {
    let s = create_transcript_segments(&[("  Hello with spaces  ".into(), 0.0, 1000.0)]);
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].text, "Hello with spaces");
}
#[test]
fn test_create_transcript_segments_generates_unique_ids() {
    let s = create_transcript_segments(&[
        ("Segment one".into(), 0.0, 1000.0),
        ("Segment two".into(), 1000.0, 2000.0),
    ]);
    assert_eq!(s.len(), 2);
    assert_ne!(s[0].id, s[1].id);
    assert!(s.iter().all(|s| s.id.starts_with("transcript-")));
}
#[test]
fn test_cancellation_flag() {
    RETRANSCRIPTION_CANCELLED.store(false, Ordering::SeqCst);
    RETRANSCRIPTION_IN_PROGRESS.store(false, Ordering::SeqCst);
    assert!(!is_retranscription_in_progress());
    cancel_retranscription();
    assert!(RETRANSCRIPTION_CANCELLED.load(Ordering::SeqCst));
    RETRANSCRIPTION_CANCELLED.store(false, Ordering::SeqCst);
}
#[test]
fn test_vad_redemption_time_constant() {
    assert_eq!(VAD_REDEMPTION_TIME_MS, 2000);
}
#[test]
fn test_find_audio_file_common_candidates() {
    let d = tempfile::tempdir().unwrap();
    assert!(find_audio_file(d.path()).is_err());
    std::fs::write(d.path().join("audio.mp4"), b"fake").unwrap();
    assert_eq!(
        find_audio_file(d.path()).unwrap().file_name().unwrap(),
        "audio.mp4"
    );
}
#[test]
fn test_find_audio_file_non_mp4_extensions() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("audio.wav"), b"fake").unwrap();
    assert_eq!(
        find_audio_file(d.path()).unwrap().file_name().unwrap(),
        "audio.wav"
    );
}
#[test]
fn test_find_audio_file_fallback_scan() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("my_recording.flac"), b"fake").unwrap();
    std::fs::write(d.path().join("notes.txt"), b"text").unwrap();
    assert_eq!(
        find_audio_file(d.path()).unwrap().file_name().unwrap(),
        "my_recording.flac"
    );
}
#[test]
fn test_find_audio_file_priority_order() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("audio.m4a"), b"fake").unwrap();
    std::fs::write(d.path().join("audio.mp4"), b"fake").unwrap();
    assert_eq!(
        find_audio_file(d.path()).unwrap().file_name().unwrap(),
        "audio.mp4"
    );
}
#[test]
fn test_find_audio_file_empty_folder() {
    let d = tempfile::tempdir().unwrap();
    assert!(find_audio_file(d.path())
        .unwrap_err()
        .to_string()
        .contains("No audio file found"));
}
#[test]
fn test_find_audio_file_nonexistent_folder() {
    assert!(find_audio_file(Path::new("/nonexistent/path/12345")).is_err());
}
#[test]
fn test_audio_extensions_constant() {
    for ext in [
        "mp4", "m4a", "wav", "mp3", "flac", "ogg", "aac", "mkv", "webm", "wma",
    ] {
        assert!(AUDIO_EXTENSIONS.contains(&ext));
    }
    for ext in ["txt", "pdf"] {
        assert!(!AUDIO_EXTENSIONS.contains(&ext));
    }
}
#[test]
fn retranscription_metadata_replaces_stale_duration_without_losing_existing_fields() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("metadata.json");
    std::fs::write(&path,serde_json::to_vec(&serde_json::json!({"meeting_id":"existing-meeting","duration_seconds":2523.2,"audio_file":"recording.m4a","summary_language":"fr","detected_summary_language":"en","custom_field":"preserve me"})).unwrap()).unwrap();
    write_retranscription_metadata(
        d.path(),
        "ignored-for-existing-metadata",
        5046.4,
        "recording.m4a",
    )
    .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(v["duration_seconds"], 5046.4);
    assert_eq!(v["meeting_id"], "existing-meeting");
    assert_eq!(v["audio_file"], "recording.m4a");
    assert_eq!(v["summary_language"], "fr");
    assert_eq!(v["custom_field"], "preserve me");
    assert!(v.get("detected_summary_language").is_none());
}
