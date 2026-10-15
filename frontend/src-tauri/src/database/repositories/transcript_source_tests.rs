use super::*;

#[tokio::test]
async fn existing_source_column_roundtrips_identical_text_both_inputs_and_legacy_after_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(dir.path().join("source.sqlite")).create_if_missing(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
        .connect_with(options.clone()).await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let mut segments = Vec::new();
    for (i, speaker) in [Some("mic"), Some("system"), None].into_iter().enumerate() {
        segments.push(TranscriptSegment {
            id: format!("s{i}"), text: "same utterance".into(), timestamp: "00:01".into(),
            audio_start_time: Some(if i < 2 { 0.0 } else { 2.0 }),
            audio_end_time: Some(if i < 2 { 1.0 } else { 3.0 }), duration: Some(1.0),
            speaker: speaker.map(str::to_string),
        });
    }
    let id = TranscriptsRepository::save_transcript(&pool, "synthetic source test", &segments, None).await.unwrap();
    pool.close().await;
    let reopened = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
        .connect_with(options).await.unwrap();
    let (rows, total) = crate::database::repositories::meeting::MeetingsRepository::get_meeting_transcripts_paginated(
        &reopened, &id, 10, 0).await.unwrap();
    assert_eq!(total, 3);
    assert_eq!(rows.iter().map(|r| r.speaker.as_deref()).collect::<Vec<_>>(), vec![Some("mic"), Some("system"), None]);
    assert!(rows.iter().all(|r| r.transcript == "same utterance"));
    let (first, _) = crate::database::repositories::meeting::MeetingsRepository::get_meeting_transcripts_paginated(
        &reopened, &id, 1, 0).await.unwrap();
    let (second, _) = crate::database::repositories::meeting::MeetingsRepository::get_meeting_transcripts_paginated(
        &reopened, &id, 1, 1).await.unwrap();
    assert_ne!(first[0].id, second[0].id, "equal-time sources must paginate without loss");
    assert_eq!(first[0].speaker.as_deref(), Some("mic"));
    assert_eq!(second[0].speaker.as_deref(), Some("system"));
    reopened.close().await;
}

#[test]
fn legacy_api_and_canonical_json_remain_unknown_not_microphone() {
    let api: TranscriptSegment = serde_json::from_value(serde_json::json!({
        "id":"old", "text":"raw", "timestamp":"00:00"
    })).unwrap();
    assert_eq!(api.speaker, None);
    let native: crate::audio::recording_saver::TranscriptSegment = serde_json::from_value(serde_json::json!({
        "id":"old", "text":"raw", "audio_start_time":0.0, "audio_end_time":1.0, "duration":1.0,
        "display_time":"00:00", "confidence":-1.0, "sequence_id":0
    })).unwrap();
    assert_eq!(native.speaker, None);
}
