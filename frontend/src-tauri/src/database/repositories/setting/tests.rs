use super::SettingsRepository;
use crate::database::manager::DatabaseManager;

#[tokio::test]
async fn local_qwen_config_roundtrips_without_api_key_and_preserves_cloud_keys() {
    let root = tempfile::tempdir().unwrap();
    let manager = DatabaseManager::new(
        root.path().join("settings.sqlite").to_str().unwrap(),
        root.path().join("unused-legacy.db").to_str().unwrap(),
    ).await.unwrap();
    let pool = manager.pool();
    SettingsRepository::save_transcript_api_key(pool, "openai", "synthetic-existing-key").await.unwrap();
    SettingsRepository::save_transcript_config(pool, "qwen", "Qwen3-ASR-1.7B-INT8").await.unwrap();
    SettingsRepository::save_transcript_api_key(pool, "qwen", "synthetic-must-not-store").await.unwrap();
    let config = SettingsRepository::get_transcript_config(pool).await.unwrap().unwrap();
    assert_eq!(config.provider, "qwen");
    assert_eq!(config.model, "Qwen3-ASR-1.7B-INT8");
    assert_eq!(SettingsRepository::get_transcript_api_key(pool, &config.provider).await.unwrap(), None);
    assert_eq!(SettingsRepository::get_transcript_api_key(pool, "openai").await.unwrap().as_deref(), Some("synthetic-existing-key"));
    assert_eq!(SettingsRepository::get_transcript_api_key(pool, "parakeet").await.unwrap(), None);
    assert!(SettingsRepository::get_transcript_api_key(pool, "unknown-provider").await.is_err());
    pool.close().await;
}
