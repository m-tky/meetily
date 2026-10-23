//! The complete supported ASR catalog. IDs are stable persisted values.
use serde_json::Value;

pub const LARGE: &str = "Qwen3-ASR-1.7B-INT8";
pub const SMALL: &str = "Qwen3-ASR-0.6B-INT8";
pub const MODELS: [&str; 2] = [LARGE, SMALL];

pub fn validate(model: &str) -> Result<&str, String> {
    if MODELS.contains(&model) {
        Ok(model)
    } else {
        Err(format!(
            "Unsupported ASR model: {model}. Select Qwen3-ASR 1.7B or 0.6B."
        ))
    }
}
pub fn directory(model: &str) -> Result<&str, String> {
    validate(model)?;
    Ok(model.strip_suffix("-INT8").unwrap())
}
pub fn from_config(config: &Value) -> Result<&'static str, String> {
    let text = &config["thinker_config"]["text_config"];
    if config["model_type"] == "qwen3_asr" && text["num_hidden_layers"] == 28 {
        match text["hidden_size"].as_u64() {
            Some(2048) => return Ok(LARGE),
            Some(1024) => return Ok(SMALL),
            _ => {}
        }
    }
    Err("Select an official Qwen3-ASR 1.7B or 0.6B model directory".into())
}
/// Old providers are retired; preserve their files but use the Qwen default.
/// An unknown Qwen ID is an error, never a silent model substitution.
pub fn configured(provider: &str, model: &str) -> Result<String, String> {
    if provider == "qwen" {
        Ok(validate(model)?.into())
    } else {
        Ok(LARGE.into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_is_exact_and_legacy_migration_is_non_destructive() {
        assert_eq!(MODELS.len(), 2);
        assert!(validate("Qwen3-ASR-0.4B-INT8").is_err());
        assert_eq!(configured("parakeet", "old").unwrap(), LARGE);
        assert_eq!(configured("qwen", SMALL).unwrap(), SMALL);
        assert!(configured("qwen", "unknown").is_err());
        for (hidden, id) in [(2048, LARGE), (1024, SMALL)] {
            assert_eq!(from_config(&serde_json::json!({"model_type":"qwen3_asr", "thinker_config":{"text_config":{"hidden_size":hidden,"num_hidden_layers":28}}})).unwrap(), id);
        }
        assert!(from_config(&serde_json::json!({})).is_err());
    }
}
