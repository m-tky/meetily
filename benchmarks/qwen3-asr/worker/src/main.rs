//! CPU stdin/stdout worker. Capture stays in the opt-in Tauri provider.
// Windows compilation is enabled for isolated validation, not app acceptance.
#[cfg(not(any(
    target_os = "linux",
    all(target_os = "windows", target_arch = "x86_64", target_env = "msvc")
)))]
compile_error!("Worker validation supports Linux and Windows x64/MSVC only");
use anyhow::{bail, Context, Result};
use qwen_asr::{context::QwenModel, kernels, transcribe};
use qwen_speech_worker::alignment::align_text;
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
        #[serde(default)]
        align: bool,
    },
    #[serde(rename = "align")]
    Align {
        id: String,
        wav: String,
        text: String,
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
            Self::Transcribe { id, .. }
            | Self::Align { id, .. }
            | Self::Ping { id }
            | Self::Shutdown { id } => id,
        }
    }
}
extern "C" {
    fn openblas_get_config() -> *const std::ffi::c_char;
    fn openblas_get_num_threads() -> std::ffi::c_int;
    fn openblas_set_num_threads(threads: std::ffi::c_int);
}
fn check_blas(threads: usize) -> Result<()> {
    unsafe {
        let cfg = std::ffi::CStr::from_ptr(openblas_get_config()).to_string_lossy();
        if cfg.contains("USE64BITINT") {
            bail!("BLAS ABI mismatch: LP64 required");
        }
        // OpenBLAS can cap its environment-derived startup count to the CPU
        // count (e.g. four-core CI), even when six threads were requested.
        // Apply the existing explicit worker budget, then verify it strictly.
        openblas_set_num_threads(threads as std::ffi::c_int);
        let actual = openblas_get_num_threads();
        if actual != threads as i32 {
            bail!("BLAS thread count mismatch: requested {threads}, actual {actual}");
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
    crop_context: bool,
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
    let windows = recognition_window(&intervals, samples.len(), crop_context);
    let load_start = Instant::now();
    let newly_loaded = model.is_none();
    if newly_loaded {
        let m = QwenModel::load(model_dir).context("model_load_failed")?;
        let config: Value =
            serde_json::from_slice(&std::fs::read(Path::new(model_dir).join("config.json"))?)?;
        let declared = qwen_speech_worker::model_identity(&config)?;
        let hidden = if declared == "Qwen3-ASR-0.6B" {
            1024
        } else {
            2048
        };
        if m.config.dec_hidden != hidden || m.config.dec_layers != 28 {
            bail!("model_tensor_identity_does_not_match_config");
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
        // Brief reference-unmatched sound is ambiguous, not proven hallucination.
        // Preserve short responses; count gate evidence, not added context silence.
        if intervals
            .iter()
            .map(|i| i.end_sample - i.start_sample)
            .sum::<usize>()
            < RATE
        {
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
        json!({"id":id,"status":if review{"review_required"}else{"recognized"},"text":combined,"segments":segments,"asr_calls":windows.len(),"gate_intervals":intervals,"vad_frame_samples":512,"recognition_context":if crop_context{"vad_envelope"}else{"full_input"},"vad_seconds":vad_seconds,"vad_gain":gain,"load_seconds":load_seconds,"inference_seconds":compute.elapsed().as_secs_f64(),"model_loaded":true,"confirmed":false}),
    )
}
fn main() -> Result<()> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    if !std::is_x86_feature_detected!("avx2") || !std::is_x86_feature_detected!("fma") {
        bail!("Qwen requires AVX2 and FMA CPU support");
    }
    let args: Vec<_> = env::args().collect();
    if args.len() < 3 || args.len() > 9 {
        bail!("usage: qwen-speech-worker MODEL_DIR THREADS [--model Qwen3-ASR-1.7B|Qwen3-ASR-0.6B] [--vad-only] [--crop-context] [--aligner-dir DIR]");
    }
    let threads: usize = args[2].parse()?;
    if !(1..=12).contains(&threads) {
        bail!("invalid_threads");
    }
    let mut flags = args[3..].to_vec();
    let mut requested_model = None;
    if let Some(pos) = flags.iter().position(|s| s == "--model") {
        let name = flags
            .get(pos + 1)
            .context("missing_model_identity")?
            .clone();
        if !["Qwen3-ASR-1.7B", "Qwen3-ASR-0.6B"].contains(&name.as_str()) {
            bail!("unsupported_model_identity");
        }
        requested_model = Some(name);
        flags.drain(pos..pos + 2);
    }
    let mut aligner_dir = None;
    if let Some(pos) = flags.iter().position(|s| s == "--aligner-dir") {
        let dir = flags
            .get(pos + 1)
            .context("missing_aligner_directory")?
            .clone();
        if dir.is_empty() || dir.starts_with("--") {
            bail!("invalid_aligner_directory");
        }
        aligner_dir = Some(dir);
        flags.drain(pos..pos + 2);
    }
    if flags
        .iter()
        .any(|s| !["--vad-only", "--crop-context"].contains(&s.as_str()))
        || flags.iter().collect::<HashSet<_>>().len() != flags.len()
    {
        bail!("invalid_mode");
    }
    let vad_only = flags.iter().any(|s| s == "--vad-only");
    let crop_context = flags.iter().any(|s| s == "--crop-context");
    // Fail early, before even a silent request. No library/model fallback.
    kernels::set_threads(threads);
    kernels::set_verbose(0);
    check_blas(threads)?;
    let ort_path = env::var("ORT_DYLIB_PATH")
        .context("ORT_DYLIB_PATH is required for this isolated worker")?;
    ort::init_from(ort_path).commit()?;
    let config_path = Path::new(&args[1]).join("config.json");
    let model_name = if config_path.exists() {
        let config: Value = serde_json::from_slice(&std::fs::read(config_path)?)?;
        let detected = qwen_speech_worker::model_identity(&config)?;
        if requested_model
            .as_deref()
            .is_some_and(|name| name != detected)
        {
            bail!("requested_model_does_not_match_config");
        }
        detected.to_string()
    } else {
        // An explicit identity permits a no-weights runtime startup probe. This
        // never claims weights are loaded; actual inference still requires config
        // and matching tensors. Never infer a size from an absent directory.
        requested_model.context("model_config_missing_select_explicit_model_for_runtime_probe")?
    };
    emit(
        json!({"event":"ready","protocol":1,"model":model_name,"precision":"decoder-INT8-encoder-prefill-F32","threads":threads,"max_audio_seconds":30,"max_request_bytes":4096,"audio_formats":["pcm16","float32"],"model_loaded":false,"vad_only":vad_only,"vad_model":"silero-v4","vad_frame_samples":512,"recognition_context":if crop_context{"vad_envelope"}else{"full_input"},"alignment_enabled":aligner_dir.is_some(),"alignment_unit":"unicode_scalar","max_alignment_chars":512}),
    )?;
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut model = None;
    let mut aligner_model = None;
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
            Request::Align {
                wav,
                text,
                language,
                ..
            } => {
                let result = (|| -> Result<Value> {
                    let dir = aligner_dir.as_deref().context("alignment_not_enabled")?;
                    let samples = read_wav(Path::new(&wav))?;
                    align_text(dir, &mut aligner_model, &samples, &text, &language)
                })();
                match result {
                    Ok(mut value) => {
                        value["id"] = json!(id);
                        value["text_origin"] = json!("provided_not_recognition_verified");
                        emit(value)?;
                    }
                    Err(_) => emit(
                        json!({"id":id,"status":"error","code":"alignment_failed","confirmed":false}),
                    )?,
                }
            }
            Request::Transcribe {
                wav,
                language,
                align,
                ..
            } => {
                match run_request(
                    &id,
                    &wav,
                    &language,
                    &args[1],
                    &mut model,
                    vad_only,
                    crop_context,
                ) {
                    Ok(mut value) => {
                        if align {
                            value["alignment"] = if value["asr_calls"] == 0 {
                                json!({"status":"skipped_no_speech","confirmed":false})
                            } else {
                                let result = (|| -> Result<Value> {
                                    if crop_context {
                                        bail!("alignment_requires_full_input");
                                    }
                                    let dir =
                                        aligner_dir.as_deref().context("alignment_not_enabled")?;
                                    let samples = read_wav(Path::new(&wav))?;
                                    let text =
                                        value["text"].as_str().context("missing_asr_text")?;
                                    align_text(dir, &mut aligner_model, &samples, text, &language)
                                })();
                                match result {
                                    Ok(mut alignment) => {
                                        alignment["text_origin"] = json!("raw_asr");
                                        alignment
                                    }
                                    Err(_) => {
                                        json!({"status":"failed","code":"alignment_failed","confirmed":false})
                                    }
                                }
                            };
                        }
                        emit(value)?;
                    }
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
    fn explicit_blas_budget_overrides_startup_count() {
        let saved = unsafe { openblas_get_num_threads() };
        unsafe { openblas_set_num_threads(1) };
        let initial = unsafe { openblas_get_num_threads() };
        let result = check_blas(6);
        let actual = unsafe { openblas_get_num_threads() };
        unsafe { openblas_set_num_threads(saved) };
        assert_eq!(initial, 1);
        result.unwrap();
        assert_eq!(actual, 6);
    }
    #[test]
    fn ids_bounded_and_unique_format() {
        assert!(valid_id("clip-1"));
        assert!(!valid_id("../file"));
        assert!(!valid_id(&"x".repeat(65)));
    }
    #[test]
    fn alignment_is_explicit_and_legacy_requests_default_off() {
        let r: Request = serde_json::from_str(
            r#"{"op":"transcribe","id":"x","wav":"/public.wav","language":"Japanese"}"#,
        )
        .unwrap();
        assert!(matches!(r, Request::Transcribe { align: false, .. }));
        let r: Request = serde_json::from_str(r#"{"op":"transcribe","id":"x","wav":"/public.wav","language":"Japanese","align":true}"#).unwrap();
        assert!(matches!(r, Request::Transcribe { align: true, .. }));
        let r: Request = serde_json::from_str(
            r#"{"op":"align","id":"a","wav":"/public.wav","language":"Japanese","text":"はい"}"#,
        )
        .unwrap();
        assert_eq!(r.id(), "a");
    }
    #[test]
    fn unknown_fields_rejected() {
        assert!(serde_json::from_str::<Request>(r#"{"op":"ping","id":"x","extra":1}"#).is_err());
    }
}
