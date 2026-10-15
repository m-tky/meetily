use super::*;
#[tokio::test]
async fn malformed_open_is_not_retried_and_journal_sources_are_not_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let files = ["meeting_minutes.sqlite", "meeting_minutes.sqlite-wal", "meeting_minutes.sqlite-shm"];
    for name in files { fs::write(dir.path().join(name), name.as_bytes()).unwrap(); }
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let result = open_preserving(async {
        calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err(sqlx::Error::Protocol("malformed synthetic database".into()))
    }).await;
    assert!(result.err().unwrap().to_string().contains("malformed"));
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    for name in files { assert_eq!(fs::read(dir.path().join(name)).unwrap(), name.as_bytes()); }
}
