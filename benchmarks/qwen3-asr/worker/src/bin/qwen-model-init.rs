//! Initialize only the local decoder INT8 cache from official weights; no audio/inference.
use anyhow::{bail, Context, Result};
use qwen_asr::{context::QwenModel, kernels};

fn main() -> Result<()> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    if !std::is_x86_feature_detected!("avx2") || !std::is_x86_feature_detected!("fma") {
        bail!("Qwen requires AVX2 and FMA CPU support");
    }
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 2 {
        bail!("usage: qwen-model-init MODEL_DIRECTORY");
    }
    kernels::set_threads(6);
    kernels::set_verbose(0);
    let model = QwenModel::load(&args[1]).context("local_model_cache_initialization_failed")?;
    let layout = qwen_asr::int8_sidecar::SidecarLayout::compute(&model.config);
    if qwen_asr::int8_sidecar::SidecarMmap::open_valid(
        &qwen_asr::int8_sidecar::sidecar_path(&args[1]),
        &args[1],
        &model.config,
        &layout,
    )
    .is_none()
    {
        bail!("local_INT8_cache_was_not_persisted_or_is_invalid");
    }
    println!("Local Qwen model/cache initialized; no audio or inference used.");
    Ok(())
}
