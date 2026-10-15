//! Linux GUI fixture bootstrap: migrations only, no capture/models/network/auth.
use app_lib::database::manager::DatabaseManager;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let root = std::path::PathBuf::from(std::env::args().nth(1).ok_or_else(|| anyhow::anyhow!("Private synthetic root required"))?).canonicalize()?;
    anyhow::ensure!(std::fs::read_to_string(root.join(".synthetic-gui-test"))? == "synthetic-only\n", "Not a synthetic GUI profile");
    let data = root.join("data/com.meetily.ai");
    let db = data.join("meeting_minutes.sqlite");
    anyhow::ensure!(!db.exists(), "Refusing to modify an existing database");
    std::fs::create_dir_all(&data)?;
    let manager = DatabaseManager::new(db.to_str().unwrap(), data.join("unused-legacy.db").to_str().unwrap()).await?;
    manager.pool().close().await;
    println!("Synthetic GUI database migrated; no models or devices used");
    Ok(())
}
