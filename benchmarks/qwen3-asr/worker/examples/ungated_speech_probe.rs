//! Diagnostic ASR-only probe of KNOWN public speech: deliberately bypass VAD.
//! Never an application fallback or a silence decision. Fresh state per request.
use anyhow::{bail, Context, Result};
use qwen_asr::{context::QwenModel, kernels, transcribe};
use qwen_speech_worker::read_wav;
use serde_json::json;
use std::{env, fs, path::Path, time::Instant};

extern "C" {
    fn openblas_get_config() -> *const std::ffi::c_char;
    fn openblas_get_num_threads() -> std::ffi::c_int;
}

fn main() -> Result<()> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 4 {
        bail!("usage: ungated_speech_probe MODEL_DIR THREADS KNOWN_SPEECH.tsv");
    }
    let threads: usize = args[2].parse()?;
    if !(1..=12).contains(&threads) {
        bail!("invalid_threads");
    }
    kernels::set_threads(threads);
    kernels::set_verbose(0);
    unsafe {
        let config = std::ffi::CStr::from_ptr(openblas_get_config()).to_string_lossy();
        if config.contains("USE64BITINT") || openblas_get_num_threads() != threads as i32 {
            bail!("BLAS ABI/thread mismatch");
        }
    }
    let model = QwenModel::load(&args[1]).context("model_load_failed")?;
    if model.config.dec_hidden != 2048 || model.config.dec_layers != 28 {
        bail!("expected_qwen3_asr_1_7b");
    }
    for line in fs::read_to_string(&args[3])?.lines() {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 3 || !["Japanese", "English", "Thai"].contains(&fields[1]) {
            bail!("invalid_known_speech_row");
        }
        let samples = read_wav(Path::new(fields[2]))?;
        for repeat in 0..2 {
            let mut session = model.new_session();
            session.segment_sec = 0.0;
            session.skip_silence = false;
            session.past_text_conditioning = false;
            session
                .set_force_language(fields[1])
                .map_err(|_| anyhow::anyhow!("invalid_language"))?;
            let start = Instant::now();
            let text = transcribe::transcribe_audio(&mut session, &samples)
                .context("diagnostic_inference_failed")?;
            println!(
                "{}",
                json!({"id":fields[0],"language":fields[1],"repeat":repeat,
                "text":text,"gate_bypassed_for_diagnosis":true,"confirmed":false,
                "input_samples":samples.len(),"inference_seconds":start.elapsed().as_secs_f64(),
                "text_tokens":session.perf_text_tokens,"recognition_context":"full_input",
                "threads":threads,"precision":"decoder-INT8-encoder-prefill-F32"})
            );
        }
    }
    Ok(())
}
