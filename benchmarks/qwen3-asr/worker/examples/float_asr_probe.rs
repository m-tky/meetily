//! Approved diagnostic ONLY: bounded f32 input to the SAME resident1.7B INT8 ASR.
//! Gate masks are from independently verified pinned official VAD, not app policy.
//! Known public speech also runs ungated to isolate transcription from gate loss.
use anyhow::{bail, Context, Result};
use qwen_asr::{context::QwenModel, kernels, transcribe};
use qwen_speech_worker::{read_line_bounded, review_reasons, validate_audio, MAX_SAMPLES, RATE};
use serde::Deserialize;
use serde_json::json;
use std::{env, fs, io::Read, path::Path, time::Instant};

extern "C" {
    fn openblas_get_config() -> *const std::ffi::c_char;
    fn openblas_get_num_threads() -> std::ffi::c_int;
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Role {
    Speech,
    Negative,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    id: String,
    language: String,
    path: String,
    samples: usize,
    role: Role,
    gate_detected: bool,
    gate_speech_samples: usize,
}

fn read_float(path: &Path) -> Result<Vec<f32>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take((MAX_SAMPLES * 4 + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.is_empty() || bytes.len() > MAX_SAMPLES * 4 || bytes.len() % 4 != 0 {
        bail!("invalid_float_byte_count");
    }
    let samples: Vec<_> = bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    validate_audio(&samples)?;
    Ok(samples)
}

fn main() -> Result<()> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 4 {
        bail!("usage: float_asr_probe MODEL_DIR THREADS INPUTS.jsonl");
    }
    let threads: usize = args[2].parse()?;
    if threads != 6 {
        bail!("diagnostic_requires_six_threads");
    }
    kernels::set_threads(threads);
    kernels::set_verbose(0);
    unsafe {
        let config = std::ffi::CStr::from_ptr(openblas_get_config()).to_string_lossy();
        if config.contains("USE64BITINT") || openblas_get_num_threads() != 6 {
            bail!("BLAS ABI/thread mismatch");
        }
    }
    let model = QwenModel::load(&args[1]).context("model_load_failed")?;
    if model.config.dec_hidden != 2048 || model.config.dec_layers != 28 {
        bail!("expected_1_7b");
    }
    let mut reader = std::io::BufReader::new(fs::File::open(&args[3])?);
    while let Some(line) = read_line_bounded(&mut reader)? {
        let input: Input = serde_json::from_slice(&line)?;
        if input.id.is_empty()
            || input.id.len() > 256
            || !["Japanese", "English", "Thai"].contains(&input.language.as_str())
        {
            bail!("invalid_diagnostic_metadata");
        }
        let samples = read_float(Path::new(&input.path))?;
        if samples.len() != input.samples || input.gate_speech_samples > samples.len() {
            bail!("input_length_mismatch");
        }
        let known = matches!(input.role, Role::Speech);
        let run = input.gate_detected || known;
        let mut text = String::new();
        let mut reasons = Vec::new();
        let mut tokens = 0;
        let mut seconds = 0.0;
        if run {
            let mut session = model.new_session();
            session.segment_sec = 0.0;
            session.skip_silence = false;
            session.past_text_conditioning = false;
            session
                .set_force_language(&input.language)
                .map_err(|_| anyhow::anyhow!("invalid_language"))?;
            let start = Instant::now();
            text = transcribe::transcribe_audio(&mut session, &samples)
                .context("diagnostic_inference_failed")?;
            seconds = start.elapsed().as_secs_f64();
            tokens = session.perf_text_tokens;
            reasons = review_reasons(&text, tokens);
            if input.gate_speech_samples < RATE {
                reasons.push("short_speech_uncertain");
            }
        }
        println!(
            "{}",
            json!({"id":input.id,"language":input.language,"input_samples":samples.len(),
            "text":text,"asr_calls":if run{1}else{0},"gate_detected":input.gate_detected,
            "known_speech_diagnostic_bypass":known && !input.gate_detected,"confirmed":false,
            "review_reasons":reasons,"text_tokens":tokens,"inference_seconds":seconds,
            "recognition_context":"full_original_input","asr_input_gain":1.0,"threads":threads,
            "precision":"decoder-INT8-encoder-prefill-F32","application_policy_changed":false})
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn float_reader_preserves_tiny_values_and_rejects_invalid() {
        let path = env::temp_dir().join(format!("qwen-float-probe-{}.raw", std::process::id()));
        let values = [0.000001_f32, -0.000002, 0.0, 1.0];
        fs::write(
            &path,
            values
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<_>>(),
        )
        .unwrap();
        assert_eq!(read_float(&path).unwrap(), values);
        fs::write(&path, [0_u8; 3]).unwrap();
        assert!(read_float(&path).is_err());
        fs::write(&path, f32::NAN.to_le_bytes()).unwrap();
        assert!(read_float(&path).is_err());
        fs::write(&path, 1.1_f32.to_le_bytes()).unwrap();
        assert!(read_float(&path).is_err());
        fs::remove_file(&path).unwrap();
    }
    #[test]
    fn input_contract_rejects_unknown_role_and_fields() {
        assert!(serde_json::from_str::<Input>(r#"{"id":"x","language":"Japanese","path":"x","samples":1,"role":"unknown","gate_detected":false,"gate_speech_samples":0}"#).is_err());
        assert!(serde_json::from_str::<Input>(r#"{"id":"x","language":"Japanese","path":"x","samples":1,"role":"speech","gate_detected":false,"gate_speech_samples":0,"expected_text":"いいえ"}"#).is_err());
    }
}
