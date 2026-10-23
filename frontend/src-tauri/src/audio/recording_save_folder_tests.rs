use super::*;

fn fixture_segment(text: &str) -> TranscriptSegment {
    TranscriptSegment {
        id: "fixture_0".into(), text: text.into(), audio_start_time: 0.0,
        audio_end_time: 1.0, duration: 1.0, display_time: "00:00".into(),
        confidence: -1.0, sequence_id: 0, speaker: Some("system".into()),
    }
}

#[test]
fn configured_folder_owns_metadata_and_transcript_sink() {
    let root = tempfile::tempdir().unwrap();
    let selected = root.path().join("日本語 O'Brien 空白").join("custom destination");
    let mut saver = RecordingSaver::new();
    saver.initialize_meeting_folder("synthetic fixture", false, &selected).unwrap();
    let folder = saver.get_meeting_folder().unwrap().clone();
    assert_eq!(folder.parent(), Some(selected.as_path()));
    assert!(folder.join("metadata.json").is_file());
    assert!(!folder.join(".checkpoints").exists());
    assert_eq!(saver.transcript_sink().folder(), Some(folder.clone()));
    saver.transcript_sink().add(fixture_segment("Synthetic private-folder evidence")).unwrap();
    assert!(folder.join("transcripts.json").is_file());
    assert_eq!(std::fs::read_dir(&selected).unwrap().count(), 1);
}

#[tokio::test]
async fn both_auto_save_modes_use_selected_folder_and_drain_owned_queue() {
    for auto_save in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected nested folder");
        let mut saver = RecordingSaver::new();
        saver.set_meeting_name(Some("synthetic fixture".into()));
        let (tx, rx) = mpsc::unbounded_channel();
        saver.start_accumulation(auto_save, rx, &selected).unwrap();
        let folder = saver.get_meeting_folder().unwrap();
        assert_eq!(folder.parent(), Some(selected.as_path()));
        assert_eq!(folder.join(".checkpoints").is_dir(), auto_save);
        drop(tx);
        assert_eq!(saver.accumulation.take().unwrap().await.unwrap(), 0);
    }
}

#[tokio::test]
async fn blocked_destination_fails_without_fallback_or_accumulation() {
    for auto_save in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("blocked");
        std::fs::write(&selected, b"preserve existing file").unwrap();
        let mut saver = RecordingSaver::new();
        saver.set_meeting_name(Some("synthetic fixture".into()));
        let (_tx, rx) = mpsc::unbounded_channel();
        assert!(saver.start_accumulation(auto_save, rx, &selected).is_err());
        assert!(saver.get_meeting_folder().is_none());
        assert!(saver.accumulation.is_none());
        assert_eq!(std::fs::read(&selected).unwrap(), b"preserve existing file");
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    }
}

#[test]
fn relative_destination_is_refused_before_creating_any_meeting() {
    for auto_save in [false, true] {
        let mut saver = RecordingSaver::new();
        let error = saver.initialize_meeting_folder("synthetic fixture", auto_save, &PathBuf::from("relative destination")).unwrap_err();
        assert!(error.to_string().contains("absolute"));
        assert!(saver.get_meeting_folder().is_none());
        assert!(saver.transcript_sink().folder().is_none());
    }
}

#[test]
fn late_sink_keeps_its_original_selected_folder_across_sessions() {
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join("first selected folder");
    let second = root.path().join("changed preference");
    let mut old = RecordingSaver::new();
    old.initialize_meeting_folder("same title", false, &first).unwrap();
    let old_sink = old.transcript_sink();
    let old_folder = old.get_meeting_folder().unwrap().clone();
    drop(old);
    let mut next = RecordingSaver::new();
    next.initialize_meeting_folder("same title", false, &second).unwrap();
    next.transcript_sink().add(fixture_segment("Next synthetic session")).unwrap();
    old_sink.add(fixture_segment("Late synthetic Stop tail")).unwrap();
    let read = |p: PathBuf| -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(p.join("transcripts.json")).unwrap()).unwrap()
    };
    assert_eq!(read(old_folder)["segments"][0]["text"], "Late synthetic Stop tail");
    assert_eq!(read(next.get_meeting_folder().unwrap().clone())["segments"][0]["text"], "Next synthetic session");
}
