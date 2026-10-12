//! Experimental Linux stdin/stdout worker. No capture or application integration.
#[cfg(not(target_os = "linux"))]
compile_error!("This host-tuned prototype is Linux-only; Windows acceptance is pending");
use anyhow::{bail, Context, Result};
use qwen_asr::{context::QwenModel, kernels, transcribe};
use qwen_speech_worker::{
    read_line_bounded, read_wav, recognition_window, review_reasons, speech_intervals, vad_gain,
    RATE,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    env,
    io::{self, Write},
    path::Path,
    sync::Arc,
    time::Instant,
};
#[derive(Deserialize)]
#[serde(tag = "op", deny_unknown_fields)]
enum Request {
    #[serde(rename = "transcribe")]
    Transcribe {
        id: String,
        wav: String,
        language: String,
    },
    #[serde(rename = "ping")]
    Ping { id: String },
    #[serde(rename = "shutdown")]
    Shutdown { id: String },
}
impl Request {
    fn id(&self) -> &str {
        match self {
            Self::Transcribe { id, .. } | Self::Ping { id } | Self::Shutdown { id } => id,
        }
    }
}
extern "C" {
    fn openblas_get_config() -> *const std::ffi::c_char;
    fn openblas_get_num_threads() -> std::ffi::c_int;
}
fn check_blas(threads: usize) -> Result<()> {
    unsafe {
        let cfg = std::ffi::CStr::from_ptr(openblas_get_config()).to_string_lossy();
        if cfg.contains("USE64BITINT") {
            bail!("BLAS ABI mismatch: LP64 required");
        }
        if openblas_get_num_threads() != threads as i32 {
            bail!("BLAS thread count does not match worker threads");
        }
    }
    Ok(())
}
fn emit(value: Value) -> Result<()> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    serde_json::to_writer(&mut out, &value)?;
    out.write_all(b"\n")?;
    out.flush()?;
    Ok(())
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
}
fn run_request(
    id: &str,
    wav: &str,
    language: &str,
    model_dir: &str,
    model: &mut Option<Arc<QwenModel>>,
    vad_only: bool,
) -> Result<Value> {
    if !["Japanese", "English", "Thai"].contains(&language) {
        bail!("unsupported_language");
    }
    let samples = read_wav(Path::new(wav))?;
    let start = Instant::now();
    let intervals = speech_intervals(&samples)?;
    let vad_seconds = start.elapsed().as_secs_f64();
    let gain = vad_gain(&samples);
    if intervals.is_empty() {
        return Ok(
            json!({"id":id,"status":"no_speech","text":"","segments":[],"asr_calls":0,"model_loaded":model.is_some(),"vad_seconds":vad_seconds,"vad_gain":gain,"confirmed":false}),
        );
    }
    if vad_only {
        return Ok(
            json!({"id":id,"status":"vad_only","intervals":intervals,"asr_calls":0,"model_loaded":false,"vad_seconds":vad_seconds,"vad_gain":gain,"confirmed":false}),
        );
    }
    let windows = recognition_window(&intervals);
    let load_start = Instant::now();
    let newly_loaded = model.is_none();
    if newly_loaded {
        let m = QwenModel::load(model_dir).context("model_load_failed")?;
        if m.config.dec_hidden != 2048 || m.config.dec_layers != 28 {
            bail!("expected_qwen3_asr_1_7b");
        }
        *model = Some(m);
    }
    let load_seconds = if newly_loaded {
        load_start.elapsed().as_secs_f64()
    } else {
        0.0
    };
    let mut segments = Vec::new();
    let mut combined = String::new();
    let mut review = false;
    let compute = Instant::now();
    for span in &windows {
        // Retain immutable weights, but create independent KV/encoder/prompt state
        // for every utterance. No prior transcript conditioning across requests.
        let mut ctx = model.as_ref().unwrap().new_session();
        ctx.segment_sec = 0.0;
        ctx.skip_silence = false;
        ctx.past_text_conditioning = false;
        ctx.set_force_language(language)
            .map_err(|_| anyhow::anyhow!("unsupported_language"))?;
        let tick = Instant::now();
        let raw =
            transcribe::transcribe_audio(&mut ctx, &samples[span.start_sample..span.end_sample])
                .context("inference_failed")?;
        let mut reasons = review_reasons(&raw, ctx.perf_text_tokens);
        // A brief leading sound triggered VAD and hallucinated "はい".
        // Keep short real responses, but don't silently promote them.
        if span.end_sample - span.start_sample < RATE {
            reasons.push("short_speech_uncertain");
        }
        review |= !reasons.is_empty();
        if !combined.is_empty() {
            combined.push(' ');
        }
        combined.push_str(&raw);
        segments.push(json!({"start_sample":span.start_sample,"end_sample":span.end_sample,"sample_rate":RATE,"raw_text":raw,"review_reasons":reasons,"text_tokens":ctx.perf_text_tokens,"inference_seconds":tick.elapsed().as_secs_f64(),"confirmed":false,"kind":"window_final"}));
    }
    Ok(
        json!({"id":id,"status":if review{"review_required"}else{"recognized"},"text":combined,"segments":segments,"asr_calls":windows.len(),"gate_intervals":intervals,"vad_seconds":vad_seconds,"vad_gain":gain,"load_seconds":load_seconds,"inference_seconds":compute.elapsed().as_secs_f64(),"model_loaded":true,"confirmed":false}),
    )
}
fn main() -> Result<()> {
    let args: Vec<_> = env::args().collect();
    if args.len() < 3 || args.len() > 4 {
        bail!("usage: qwen-speech-worker MODEL_DIR THREADS [--vad-only]");
    }
    let threads: usize = args[2].parse()?;
    if !(1..=12).contains(&threads) {
        bail!("invalid_threads");
    }
    let vad_only = args.get(3).map(|s| s == "--vad-only").unwrap_or(false);
    if args.len() == 4 && !vad_only {
        bail!("invalid_mode");
    }
    // Fail early, before even a silent request. No library/model fallback.
    kernels::set_threads(threads);
    kernels::set_verbose(0);
    check_blas(threads)?;
    let ort_path = env::var("ORT_DYLIB_PATH")
        .context("ORT_DYLIB_PATH is required for this isolated worker")?;
    ort::init_from(ort_path).commit()?;
    emit(
        json!({"event":"ready","protocol":1,"model":"Qwen3-ASR-1.7B","precision":"decoder-INT8-encoder-prefill-F32","threads":threads,"max_audio_seconds":30,"max_request_bytes":4096,"model_loaded":false,"vad_only":vad_only}),
    )?;
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut model = None;
    let mut seen = HashSet::new();
    while let Some(line) = read_line_bounded(&mut input)? {
        let request: Request = match serde_json::from_slice(&line) {
            Ok(r) => r,
            Err(_) => {
                emit(
                    json!({"id":null,"status":"error","code":"invalid_request","confirmed":false}),
                )?;
                continue;
            }
        };
        let id = request.id().to_owned();
        if !valid_id(&id) || seen.contains(&id) {
            emit(
                json!({"id":id,"status":"error","code":"invalid_or_duplicate_id","confirmed":false}),
            )?;
            continue;
        }
        if seen.len() >= 1024 {
            bail!("request_count_limit_restart_required");
        }
        seen.insert(id.clone());
        match request {
            Request::Ping { .. } => {
                emit(json!({"id":id,"status":"pong","model_loaded":model.is_some()}))?
            }
            Request::Shutdown { .. } => {
                emit(json!({"id":id,"status":"bye"}))?;
                break;
            }
            Request::Transcribe { wav, language, .. } => {
                match run_request(&id, &wav, &language, &args[1], &mut model, vad_only) {
                    Ok(value) => emit(value)?,
                    Err(_) => emit(
                        json!({"id":id,"status":"error","code":"transcription_failed","confirmed":false}),
                    )?,
                }
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ids_bounded_and_unique_format() {
        assert!(valid_id("clip-1"));
        assert!(!valid_id("../file"));
        assert!(!valid_id(&"x".repeat(65)));
    }
    #[test]
    fn unknown_fields_rejected() {
        assert!(serde_json::from_str::<Request>(r#"{"op":"ping","id":"x","extra":1}"#).is_err());
    }
}
