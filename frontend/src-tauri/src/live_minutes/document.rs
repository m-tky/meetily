//! Local canonical minutes editing/read/export. No model or network calls.
use super::{atomic_write, markdown, validate, Item, Items, Snapshot, Status};
use crate::{database::repositories::meeting::MeetingsRepository, state::AppState};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
};
const MAX_FILE: usize = 8 * 1024 * 1024;
static WRITE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserEdit {
    pub base_revision: u64,
    pub edited_at: String,
    pub before: Item,
    pub after: Item,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemPatch {
    pub text: String,
    pub status: Status,
    pub owner: Option<String>,
    pub due: Option<String>,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    Session { id: String },
    Meeting { id: String },
}

pub(crate) fn read(folder: &Path) -> Result<Option<Snapshot>, String> {
    let file = match std::fs::File::open(folder.join("live-minutes.json")) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("Cannot read saved minutes: {e}")),
    };
    let mut bytes = Vec::new();
    file.take((MAX_FILE + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_FILE {
        return Err("Saved minutes exceeds size bound".into());
    }
    decode(&bytes).map(Some)
}
/// Validate canonical bytes without reopening a recovery source or repairing it.
pub(crate) fn decode(bytes: &[u8]) -> Result<Snapshot, String> {
    if bytes.len() > MAX_FILE { return Err("Saved minutes exceeds size bound".into()); }
    let snapshot: Snapshot =
        serde_json::from_slice(&bytes).map_err(|e| format!("Unreadable existing minutes: {e}"))?;
    if snapshot.schema_version != 1 || snapshot.revision == 0 {
        return Err("Unsupported saved minutes schema/revision".into());
    }
    let ids: HashSet<String> = snapshot.items.iter().map(|i| i.id.clone()).collect();
    if ids.len() != snapshot.items.len()
        || snapshot
            .user_edits
            .iter()
            .any(|e| e.before.id != e.after.id || !ids.contains(&e.after.id))
    {
        return Err("Invalid saved minutes item/edit history".into());
    }
    Ok(snapshot)
}
pub(super) fn cached(folder: &Path, fingerprint: &str) -> Result<Option<Snapshot>, String> {
    let _guard = WRITE_LOCK.lock().map_err(|_| "Minutes write lock failed")?;
    let latest = read(folder)?;
    if let Some(snapshot) = latest {
        if snapshot.fingerprint == fingerprint {
            atomic_write(
                &folder.join("live-minutes.md"),
                markdown(&snapshot).as_bytes(),
            )?;
            return Ok(Some(snapshot));
        }
    }
    Ok(None)
}
pub(super) fn protect(mut items: Items, edits: &[UserEdit]) -> Items {
    // Apply in chronological order; latest explicit user edit owns the full item.
    for edit in edits {
        if let Some(item) = items.items.iter_mut().find(|i| i.id == edit.after.id) {
            *item = edit.after.clone();
        } else {
            items.items.push(edit.after.clone());
        }
    }
    items
}
fn save(folder: &Path, mut snapshot: Snapshot) -> Result<Snapshot, String> {
    let revisions = folder.join("live-minutes-revisions");
    std::fs::create_dir_all(&revisions).map_err(|e| e.to_string())?;
    // A failed canonical write can leave an archived candidate. Preserve it;
    // never overwrite an existing revision when retrying another edit.
    let mut highest = snapshot.revision;
    for (count, entry) in std::fs::read_dir(&revisions)
        .map_err(|e| e.to_string())?
        .enumerate()
    {
        if count > 100_000 {
            return Err("Minutes revision directory exceeds bound".into());
        }
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_some_and(|x| x == "json") {
            if let Some(n) = path
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.parse::<u64>().ok())
            {
                highest = highest.max(n);
            }
        }
    }
    snapshot.revision = highest.checked_add(1).ok_or("Minutes revision overflow")?;
    snapshot.updated_at = chrono::Utc::now().to_rfc3339();
    let bytes = serde_json::to_vec_pretty(&snapshot).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_FILE {
        return Err("Minutes/edit history exceeds size bound; nothing was overwritten".into());
    }
    let mut archive = tempfile::NamedTempFile::new_in(&revisions).map_err(|e| e.to_string())?;
    archive.write_all(&bytes).map_err(|e| e.to_string())?;
    archive.as_file().sync_all().map_err(|e| e.to_string())?;
    archive
        .persist_noclobber(revisions.join(format!("{:06}.json", snapshot.revision)))
        .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    std::fs::File::open(&revisions)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    atomic_write(&folder.join("live-minutes.json"), &bytes)?;
    // Canonical commit survives projection failure. Read/re-export repairs it
    // without applying the edit twice or performing a model request.
    atomic_write(&folder.join("live-minutes.md"), markdown(&snapshot).as_bytes())
        .map_err(|e| format!("MINUTES_PROJECTION_FAILED: canonical revision {} was saved; reload and re-export Markdown to recover: {e}", snapshot.revision))?;
    Ok(snapshot)
}
pub(super) fn commit_generated(
    folder: &Path,
    items: Items,
    source_ids: &HashSet<String>,
    fingerprint: String,
    model: String,
) -> Result<Snapshot, String> {
    let _guard = WRITE_LOCK.lock().map_err(|_| "Minutes write lock failed")?;
    // Re-read AFTER slow inference so an edit saved during it is never lost.
    let latest = read(folder)?;
    let edits = latest
        .as_ref()
        .map(|s| s.user_edits.clone())
        .unwrap_or_default();
    let items = protect(items, &edits);
    validate(&items, source_ids, latest.as_ref())?;
    save(
        folder,
        Snapshot {
            schema_version: 1,
            revision: latest.as_ref().map_or(0, |s| s.revision),
            fingerprint,
            model,
            updated_at: String::new(),
            items: items.items,
            user_edits: edits,
        },
    )
}
pub(super) fn edit(
    folder: &Path,
    expected_revision: u64,
    item_id: &str,
    patch: ItemPatch,
) -> Result<Snapshot, String> {
    if patch.text.trim().is_empty()
        || patch.text.len() > 8000
        || patch.owner.as_ref().is_some_and(|s| s.len() > 512)
        || patch.due.as_ref().is_some_and(|s| s.len() > 512)
    {
        return Err("Invalid edit: text is required (<=8000 bytes), owner/due <=512 bytes".into());
    }
    let _guard = WRITE_LOCK.lock().map_err(|_| "Minutes write lock failed")?;
    let mut snapshot = read(folder)?.ok_or("No saved live minutes")?;
    if snapshot.revision != expected_revision {
        return Err(format!(
            "MINUTES_REVISION_CONFLICT: current revision {}; reload before saving (draft kept)",
            snapshot.revision
        ));
    }
    let item = snapshot
        .items
        .iter_mut()
        .find(|i| i.id == item_id)
        .ok_or("Minutes item not found")?;
    let before = item.clone();
    item.text = patch.text;
    item.status = patch.status;
    item.owner = patch.owner;
    item.due = patch.due;
    let after = item.clone();
    snapshot.user_edits.push(UserEdit {
        base_revision: expected_revision,
        edited_at: chrono::Utc::now().to_rfc3339(),
        before,
        after,
    });
    save(folder, snapshot)
}
async fn folder(pool: &sqlx::SqlitePool, target: Target) -> Result<PathBuf, String> {
    match target {
        Target::Session { id } => super::session::SESSIONS
            .capture(&id, false)?
            .sink
            .folder()
            .ok_or("No current meeting folder".into()),
        Target::Meeting { id } => {
            let meeting = MeetingsRepository::get_meeting_metadata(pool, &id)
                .await
                .map_err(|e| e.to_string())?
                .ok_or("Meeting not found")?;
            meeting
                .folder_path
                .filter(|p| !p.trim().is_empty())
                .map(PathBuf::from)
                .ok_or("Meeting has no local recording folder".into())
        }
    }
}
#[tauri::command]
pub async fn api_read_live_minutes(
    state: tauri::State<'_, AppState>,
    target: Target,
) -> Result<Option<Snapshot>, String> {
    read(&folder(state.db_manager.pool(), target).await?)
}
#[tauri::command]
pub async fn api_edit_live_minutes(
    state: tauri::State<'_, AppState>,
    target: Target,
    expected_revision: u64,
    item_id: String,
    patch: ItemPatch,
) -> Result<Snapshot, String> {
    edit(
        &folder(state.db_manager.pool(), target).await?,
        expected_revision,
        &item_id,
        patch,
    )
}
#[derive(Serialize)]
pub struct Export {
    pub json: String,
    pub markdown: String,
    pub revision: u64,
}
fn export(folder: &Path) -> Result<Export, String> {
    let _guard = WRITE_LOCK.lock().map_err(|_| "Minutes write lock failed")?;
    let snapshot = read(folder)?.ok_or("No saved live minutes")?;
    let markdown = markdown(&snapshot);
    atomic_write(&folder.join("live-minutes.md"), markdown.as_bytes())?;
    Ok(Export {
        json: serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?,
        markdown,
        revision: snapshot.revision,
    })
}
#[tauri::command]
pub async fn api_export_live_minutes(
    state: tauri::State<'_, AppState>,
    target: Target,
) -> Result<Export, String> {
    export(&folder(state.db_manager.pool(), target).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item() -> Item {
        Item {
            id: "i1".into(),
            kind: super::super::Kind::Decision,
            text: "AI original".into(),
            status: Status::Active,
            sources: vec!["s1".into()],
            owner: None,
            due: None,
        }
    }
    fn generated(folder: &Path, items: Vec<Item>, fingerprint: &str) -> Snapshot {
        commit_generated(
            folder,
            Items { items },
            &HashSet::from(["s1".into(), "s2".into()]),
            fingerprint.into(),
            "synthetic".into(),
        )
        .unwrap()
    }
    fn patch(text: &str) -> ItemPatch {
        ItemPatch {
            text: text.into(),
            status: Status::Active,
            owner: Some("User-entered owner".into()),
            due: None,
        }
    }
    #[test]
    fn edits_survive_ai_rewrite_omission_and_reload_export() {
        let dir = tempfile::tempdir().unwrap();
        let first = generated(dir.path(), vec![item()], "1");
        let edited = edit(dir.path(), first.revision, "i1", patch("User text")).unwrap();
        assert_eq!(edited.revision, 2);
        assert_eq!(edited.user_edits[0].before.text, "AI original");
        assert_eq!(
            cached(dir.path(), "1").unwrap().unwrap().items[0].text,
            "User text"
        );
        assert!(std::fs::read_to_string(dir.path().join("live-minutes.md"))
            .unwrap()
            .contains("User text"));
        let mut ai = item();
        ai.text = "Attempted overwrite".into();
        let updated = generated(dir.path(), vec![ai], "2");
        assert_eq!(updated.items[0].text, "User text");
        assert_eq!(updated.user_edits.len(), 1);
        let omitted = generated(dir.path(), vec![], "3");
        assert_eq!(omitted.items[0].text, "User text");
        // Reading uses disk only: independent of any in-memory session after restart.
        let loaded = read(dir.path()).unwrap().unwrap();
        assert_eq!(loaded.revision, 4);
        let output = export(dir.path()).unwrap();
        assert!(output.markdown.contains("User text"));
        assert!(output.json.contains("AI original"));
        assert_eq!(output.revision, 4);
        assert_eq!(loaded.items[0].sources, ["s1"]);
    }
    #[test]
    fn stale_revision_invalid_patch_and_unknown_id_do_not_write() {
        let dir = tempfile::tempdir().unwrap();
        let first = generated(dir.path(), vec![item()], "1");
        edit(dir.path(), first.revision, "i1", patch("new")).unwrap();
        let before = std::fs::read(dir.path().join("live-minutes.json")).unwrap();
        assert!(edit(dir.path(), 1, "i1", patch("stale"))
            .unwrap_err()
            .starts_with("MINUTES_REVISION_CONFLICT"));
        assert!(edit(dir.path(), 2, "unknown", patch("x")).is_err());
        assert!(edit(dir.path(), 2, "i1", patch(" ")).is_err());
        assert_eq!(
            std::fs::read(dir.path().join("live-minutes.json")).unwrap(),
            before
        );
    }
    #[test]
    fn archive_collision_is_preserved_and_projection_failure_is_recoverable() {
        let dir = tempfile::tempdir().unwrap();
        generated(dir.path(), vec![item()], "1");
        let revisions = dir.path().join("live-minutes-revisions");
        std::fs::write(revisions.join("000002.json"), b"orphan candidate").unwrap();
        std::fs::remove_file(dir.path().join("live-minutes.md")).unwrap();
        std::fs::create_dir(dir.path().join("live-minutes.md")).unwrap();
        assert!(edit(dir.path(), 1, "i1", patch("durable edit")).is_err());
        assert_eq!(read(dir.path()).unwrap().unwrap().revision, 3);
        assert_eq!(
            std::fs::read(revisions.join("000002.json")).unwrap(),
            b"orphan candidate"
        );
        std::fs::remove_dir(dir.path().join("live-minutes.md")).unwrap();
        assert!(export(dir.path())
            .unwrap()
            .markdown
            .contains("durable edit"));
        assert_eq!(read(dir.path()).unwrap().unwrap().user_edits.len(), 1);
    }
    #[tokio::test]
    async fn saved_meeting_resolves_after_session_registry_is_gone() {
        let dir = tempfile::tempdir().unwrap();
        generated(dir.path(), vec![item()], "1");
        edit(dir.path(), 1, "i1", patch("reopened user edit")).unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("CREATE TABLE meetings (id TEXT PRIMARY KEY, title TEXT, created_at TEXT, updated_at TEXT, folder_path TEXT)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO meetings VALUES ('m1', 'synthetic', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', ?)")
            .bind(dir.path().to_str().unwrap()).execute(&pool).await.unwrap();
        let resolved = folder(&pool, Target::Meeting { id: "m1".into() })
            .await
            .unwrap();
        assert_eq!(
            read(&resolved).unwrap().unwrap().items[0].text,
            "reopened user edit"
        );
        assert!(export(&resolved)
            .unwrap()
            .markdown
            .contains("reopened user edit"));
        assert!(folder(
            &pool,
            Target::Meeting {
                id: "unknown".into()
            }
        )
        .await
        .is_err());
        pool.close().await;
    }
    #[test]
    fn old_schema_loads_and_corrupt_or_unknown_schema_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("live-minutes.json"), br#"{"schema_version":1,"revision":1,"fingerprint":"x","model":"old","updated_at":"x","items":[]}"#).unwrap();
        assert!(read(dir.path()).unwrap().unwrap().user_edits.is_empty());
        std::fs::write(dir.path().join("live-minutes.json"), b"broken").unwrap();
        assert!(read(dir.path()).is_err());
        std::fs::write(dir.path().join("live-minutes.json"), br#"{"schema_version":2,"revision":1,"fingerprint":"x","model":"old","updated_at":"x","items":[]}"#).unwrap();
        assert!(read(dir.path()).unwrap_err().contains("Unsupported"));
        assert!(
            serde_json::from_value::<Target>(serde_json::json!({"kind":"path","id":"/tmp/x"}))
                .is_err()
        );
    }
}
