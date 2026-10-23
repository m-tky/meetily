use super::SettingsRepository;
use crate::database::manager::DatabaseManager;

#[tokio::test]
async fn retired_selection_migration_preserves_keys_and_never_overwrites_explicit_qwen_selection() {
    use crate::audio::transcription::qwen_model::{configured, LARGE, SMALL};
    let root = tempfile::tempdir().unwrap();
    let manager = DatabaseManager::new(
        root.path().join("migration.sqlite").to_str().unwrap(),
        root.path().join("unused.db").to_str().unwrap(),
    )
    .await
    .unwrap();
    let pool = manager.pool();
    SettingsRepository::save_transcript_api_key(pool, "openai", "synthetic-legacy-key")
        .await
        .unwrap();
    SettingsRepository::save_transcript_config(pool, "localWhisper", "large-v3")
        .await
        .unwrap();
    SettingsRepository::migrate_retired_transcript_config(pool)
        .await
        .unwrap();
    let config = SettingsRepository::get_transcript_config(pool)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(config.provider, "qwen");
    assert_eq!(config.model, LARGE);
    assert_eq!(
        SettingsRepository::get_transcript_api_key(pool, "openai")
            .await
            .unwrap()
            .as_deref(),
        Some("synthetic-legacy-key")
    );
    // Even a migration requested by an earlier reader cannot clobber a new choice.
    SettingsRepository::save_transcript_config(pool, "qwen", SMALL)
        .await
        .unwrap();
    SettingsRepository::migrate_retired_transcript_config(pool)
        .await
        .unwrap();
    let config = SettingsRepository::get_transcript_config(pool)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(config.model, SMALL);
    // An unknown Qwen ID is preserved for an actionable error, never replaced.
    SettingsRepository::save_transcript_config(pool, "qwen", "unknown")
        .await
        .unwrap();
    SettingsRepository::migrate_retired_transcript_config(pool)
        .await
        .unwrap();
    let config = SettingsRepository::get_transcript_config(pool)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(config.model, "unknown");
    assert!(configured(&config.provider, &config.model).is_err());
    pool.close().await;
}

#[tokio::test]
async fn local_qwen_config_roundtrips_without_api_key_and_preserves_cloud_keys() {
    let root = tempfile::tempdir().unwrap();
    let manager = DatabaseManager::new(
        root.path().join("settings.sqlite").to_str().unwrap(),
        root.path().join("unused-legacy.db").to_str().unwrap(),
    )
    .await
    .unwrap();
    let pool = manager.pool();
    SettingsRepository::save_transcript_api_key(pool, "openai", "synthetic-existing-key")
        .await
        .unwrap();
    SettingsRepository::save_transcript_config(pool, "qwen", "Qwen3-ASR-1.7B-INT8")
        .await
        .unwrap();
    SettingsRepository::save_transcript_api_key(pool, "qwen", "synthetic-must-not-store")
        .await
        .unwrap();
    let config = SettingsRepository::get_transcript_config(pool)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(config.provider, "qwen");
    assert_eq!(config.model, "Qwen3-ASR-1.7B-INT8");
    assert_eq!(
        SettingsRepository::get_transcript_api_key(pool, &config.provider)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        SettingsRepository::get_transcript_api_key(pool, "openai")
            .await
            .unwrap()
            .as_deref(),
        Some("synthetic-existing-key")
    );
    assert_eq!(
        SettingsRepository::get_transcript_api_key(pool, "parakeet")
            .await
            .unwrap(),
        None
    );
    assert!(
        SettingsRepository::get_transcript_api_key(pool, "unknown-provider")
            .await
            .is_err()
    );
    pool.close().await;
}
