//! App adapter smoke test: generated silence only, no device capture or cloud.
use app_lib::audio::transcription::{
    qwen_provider::{QwenProvider, QwenSettings}, TranscriptionProvider,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: qwen_adapter_smoke WORKER MODEL_DIR ORT_LIBRARY".into());
    }
    let cache = tempfile::tempdir()?;
    let provider = QwenProvider::new(QwenSettings {
        worker_path: args[1].clone().into(), model_dir: args[2].clone().into(),
        ort_library: args[3].clone().into(),
    }, cache.path().join("audio"))?;
    provider.connect().await?;
    for code in ["ja", "en", "th"] {
        let result = provider.transcribe(vec![0.0; 5 * 16000], Some(code.into())).await?;
        if !result.text.is_empty() { return Err("Synthetic silence unexpectedly produced text".into()); }
    }
    if std::fs::read_dir(cache.path().join("audio"))?.count() != 0 {
        return Err("Temporary audio was retained".into());
    }
    println!("App Qwen adapter: ready handshake, float32 silence (JA/EN/TH), empty transcript and temporary-file cleanup passed; no recording / cloud / accuracy claim.");
    Ok(())
}
