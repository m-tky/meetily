use super::super::recording_saver::DeviceInfo;
use super::*;
use serde_json::json;

async fn schema(pool: &SqlitePool) {
    sqlx::query("CREATE TABLE IF NOT EXISTS meetings (id TEXT PRIMARY KEY, title TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, folder_path TEXT)")
        .execute(pool).await.unwrap();
    sqlx::query("CREATE TABLE IF NOT EXISTS transcripts (id TEXT PRIMARY KEY, meeting_id TEXT NOT NULL REFERENCES meetings(id), transcript TEXT NOT NULL, timestamp TEXT NOT NULL, audio_start_time REAL, audio_end_time REAL, duration REAL, speaker TEXT)")
        .execute(pool).await.unwrap();
}
async fn pool() -> SqlitePool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    schema(&pool).await;
    pool
}
fn fixture(root: &Path, name: &str) -> PathBuf {
    let folder = root.join(name);
    fs::create_dir_all(folder.join(".checkpoints")).unwrap();
    let metadata = MeetingMetadata {
        version: "1.0".into(),
        meeting_id: None,
        meeting_name: Some("Synthetic saved meeting".into()),
        created_at: "2026-01-01T00:00:00Z".into(),
        completed_at: None,
        duration_seconds: None,
        devices: DeviceInfo {
            microphone: None,
            system_audio: None,
        },
        audio_file: "audio.mp4".into(),
        transcript_file: "transcripts.json".into(),
        sample_rate: 48000,
        status: "recording".into(),
    };
    fs::write(
        folder.join("metadata.json"),
        serde_json::to_vec(&metadata).unwrap(),
    )
    .unwrap();
    let segment = TranscriptSegment {
        id: "seg_1".into(),
        text: "決定を保留しました".into(),
        audio_start_time: 1.0,
        audio_end_time: 2.0,
        duration: 1.0,
        display_time: "00:01".into(),
        confidence: -1.0,
        sequence_id: 1,
        speaker: Some("system".into()),
    };
    fs::write(
        folder.join("transcripts.json"),
        serde_json::to_vec(&json!({"version":"1.0","segments":[segment],"total_segments":1}))
            .unwrap(),
    )
    .unwrap();
    let before = json!({"id":"item-1","kind":"question","text":"AI draft","status":"active","sources":["seg_1"],"owner":null,"due":null});
    let mut after = before.clone();
    after["text"] = json!("User saved note");
    fs::write(folder.join("live-minutes.json"), serde_json::to_vec(&json!({"schema_version":1,"revision":3,"fingerprint":"synthetic","model":"mock","updated_at":"2026-01-01T00:00:00Z","items":[after],"user_edits":[{"base_revision":2,"edited_at":"2026-01-01T00:00:00Z","before":before,"after":after}]})).unwrap()).unwrap();
    fs::write(
        folder.join(".checkpoints/audio_chunk_000.mp4"),
        b"synthetic unvalidated checkpoint",
    )
    .unwrap();
    fs::create_dir(folder.join("live-minutes-revisions")).unwrap();
    fs::write(
        folder.join("live-minutes-revisions/000004.json"),
        b"unaccepted synthetic archive",
    )
    .unwrap();
    folder
}
#[tokio::test]
async fn scan_counts_wave_and_legacy_without_touching_checkpoint_bytes() {
    let dir = tempfile::tempdir().unwrap(); let root = dir.path();
    let folder = fixture(root, "Synthetic-Wave-Scan");
    let wav = folder.join(".checkpoints/audio_chunk_001.wav");
    super::super::checkpoint_audio::write_float_checkpoint(&wav, &[0.1;480], 48000).unwrap();
    fs::write(folder.join(".checkpoints/audio_chunk_002.wav.tmp"), b"partial ignored").unwrap();
    let before = files(&folder);
    let db = pool().await;
    let found = candidate(&db, root).await;
    assert_eq!(found.checkpoint_count, 2);
    assert!(found.warnings.iter().any(|w| w.contains("remain untouched")));
    assert_eq!(files(&folder), before);
}

fn files(folder: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    for entry in fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(files(&path));
        } else {
            out.push((path.clone(), fs::read(path).unwrap()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}
async fn candidate(pool: &SqlitePool, root: &Path) -> Candidate {
    scan(pool, root.to_path_buf(), None)
        .await
        .unwrap()
        .candidates
        .remove(0)
}
#[tokio::test]
async fn explicit_restore_reopens_after_sqlite_restart_without_changing_sources_or_edits() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("recordings");
    let folder = fixture(&root, "synthetic 会議");
    let original = files(&folder);
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(dir.path().join("index.sqlite"))
        .create_if_missing(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options.clone())
        .await
        .unwrap();
    schema(&pool).await;
    let c = candidate(&pool, &root).await;
    assert!(c.can_restore);
    let result = restore(&pool, root.clone(), c.id, c.fingerprint)
        .await
        .unwrap();
    assert!(!result.already_registered);
    assert_eq!(files(&folder), original);
    pool.close().await;
    let reopened = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    assert!(scan(&reopened, root, None)
        .await
        .unwrap()
        .candidates
        .is_empty());
    let saved: (String, String, Option<String>) =
        sqlx::query_as("SELECT transcript,id,speaker FROM transcripts WHERE meeting_id=?")
            .bind(&result.meeting_id)
            .fetch_one(&reopened)
            .await
            .unwrap();
    assert_eq!(saved.0, "決定を保留しました");
    assert!(saved.1.ends_with(":seg_1"));
    assert_eq!(saved.2.as_deref(), Some("system"));
    let resolved: (String,) = sqlx::query_as("SELECT folder_path FROM meetings WHERE id=?")
        .bind(result.meeting_id)
        .fetch_one(&reopened)
        .await
        .unwrap();
    let minutes = crate::live_minutes::document::read(Path::new(&resolved.0))
        .unwrap()
        .unwrap();
    assert_eq!(minutes.revision, 3);
    assert_eq!(minutes.items[0].text, "User saved note");
    assert_eq!(minutes.user_edits.len(), 1);
    assert_eq!(files(&folder), original);
}
#[tokio::test]
async fn stale_preview_and_corrupt_transcript_refuse_registration_without_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let folder = fixture(dir.path(), "session");
    let pool = pool().await;
    let c = candidate(&pool, dir.path()).await;
    fs::write(folder.join("transcripts.json"), b"interrupted JSON {").unwrap();
    assert!(restore(&pool, dir.path().into(), c.id, c.fingerprint)
        .await
        .unwrap_err()
        .contains("RECOVERY_CHANGED"));
    let current = candidate(&pool, dir.path()).await;
    assert!(!current.can_restore);
    assert!(
        restore(&pool, dir.path().into(), current.id, current.fingerprint)
            .await
            .is_err()
    );
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM meetings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0);
    assert_eq!(
        fs::read(folder.join("transcripts.json")).unwrap(),
        b"interrupted JSON {"
    );
}
#[tokio::test]
async fn db_failure_rolls_back_all_rows_and_sources_remain_retryable() {
    let dir = tempfile::tempdir().unwrap();
    let folder = fixture(dir.path(), "session");
    let original = files(&folder);
    let pool = pool().await;
    let c = candidate(&pool, dir.path()).await;
    sqlx::query("CREATE TRIGGER fail_recovery BEFORE INSERT ON transcripts BEGIN SELECT RAISE(ABORT,'synthetic database fault'); END").execute(&pool).await.unwrap();
    assert!(restore(
        &pool,
        dir.path().into(),
        c.id.clone(),
        c.fingerprint.clone()
    )
    .await
    .is_err());
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM meetings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0);
    assert_eq!(files(&folder), original);
    sqlx::query("DROP TRIGGER fail_recovery")
        .execute(&pool)
        .await
        .unwrap();
    assert!(restore(&pool, dir.path().into(), c.id, c.fingerprint)
        .await
        .is_ok());
    assert_eq!(files(&folder), original);
}
#[tokio::test]
async fn concurrent_retries_are_idempotent_and_segment_ids_do_not_collide_across_sessions() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path(), "a");
    fixture(dir.path(), "b");
    let pool = pool().await;
    let report = scan(&pool, dir.path().into(), None).await.unwrap();
    let a = &report.candidates[0];
    let b = &report.candidates[1];
    let (one, two) = tokio::join!(
        restore(
            &pool,
            dir.path().into(),
            a.id.clone(),
            a.fingerprint.clone()
        ),
        restore(
            &pool,
            dir.path().into(),
            a.id.clone(),
            a.fingerprint.clone()
        )
    );
    let one = one.unwrap();
    let two = two.unwrap();
    assert_eq!(one.meeting_id, two.meeting_id);
    assert_ne!(one.already_registered, two.already_registered);
    restore(
        &pool,
        dir.path().into(),
        b.id.clone(),
        b.fingerprint.clone(),
    )
    .await
    .unwrap();
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transcripts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 2);
}
#[tokio::test]
async fn archives_are_not_promoted_and_corrupt_canonical_does_not_destroy_saved_transcripts() {
    let dir = tempfile::tempdir().unwrap();
    let folder = fixture(dir.path(), "session");
    let pool = pool().await;
    fs::write(folder.join("live-minutes.json"), b"corrupt canonical {").unwrap();
    let original = files(&folder);
    let c = candidate(&pool, dir.path()).await;
    assert!(c.can_restore);
    assert!(c
        .warnings
        .iter()
        .any(|w| w.contains("Saved minutes cannot")));
    restore(&pool, dir.path().into(), c.id, c.fingerprint)
        .await
        .unwrap();
    assert_eq!(files(&folder), original);
    assert!(crate::live_minutes::document::read(&folder).is_err());
}
#[tokio::test]
async fn pagination_lists_all_folders_and_rejects_oversized_sources_and_unknown_ids() {
    let dir = tempfile::tempdir().unwrap();
    let pool = pool().await;
    for n in 0..101 {
        fixture(dir.path(), &format!("session-{n:03}"));
    }
    let first = scan(&pool, dir.path().into(), None).await.unwrap();
    assert_eq!(first.candidates.len(), 100);
    let next = scan(&pool, dir.path().into(), first.next_cursor)
        .await
        .unwrap();
    assert_eq!(next.candidates.len(), 1);
    assert!(next.next_cursor.is_none());
    let path = dir.path().join("session-100/transcripts.json");
    fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len((MAX_FILE + 1) as u64)
        .unwrap();
    let c = scan(&pool, dir.path().into(), Some("session-099".into()))
        .await
        .unwrap()
        .candidates
        .remove(0);
    assert!(!c.can_restore);
    assert!(c.warnings.iter().any(|w| w.contains("8 MiB")));
    assert!(restore(
        &pool,
        dir.path().into(),
        "../outside".into(),
        "anything".into()
    )
    .await
    .is_err());
}
#[cfg(unix)]
#[tokio::test]
async fn symlink_directories_and_source_files_are_not_followed() {
    let dir = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    fixture(external.path(), "external");
    std::os::unix::fs::symlink(
        external.path().join("external"),
        dir.path().join("shortcut"),
    )
    .unwrap();
    let pool = pool().await;
    assert!(scan(&pool, dir.path().into(), None)
        .await
        .unwrap()
        .candidates
        .is_empty());
    let folder = fixture(dir.path(), "session");
    fs::remove_file(folder.join("transcripts.json")).unwrap();
    std::os::unix::fs::symlink(
        external.path().join("external/transcripts.json"),
        folder.join("transcripts.json"),
    )
    .unwrap();
    let c = candidate(&pool, dir.path()).await;
    assert!(!c.can_restore);
    assert!(c.warnings.iter().any(|w| w.contains("non-symlink")));
}
