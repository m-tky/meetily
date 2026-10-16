//! Initialize only the local decoder INT8 cache from official weights; no audio/inference.
use anyhow::{bail, Context, Result};
use qwen_asr::{context::QwenModel, kernels};

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 2 {
        bail!("usage: qwen-model-init MODEL_DIRECTORY");
    }
    kernels::set_threads(6);
    kernels::set_verbose(0);
    let _model = QwenModel::load(&args[1]).context("local_model_cache_initialization_failed")?;
    println!("Local Qwen model/cache initialized; no audio or inference used.");
    Ok(())
}
