use anyhow::{bail, Context, Result};
use qwen_asr::{context::QwenCtx, kernels, transcribe};
use serde_json::json;
use std::{
    env, fs,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
fn cpu_seconds() -> f64 {
    unsafe {
        let mut r: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut r);
        r.ru_utime.tv_sec as f64
            + r.ru_utime.tv_usec as f64 / 1e6
            + r.ru_stime.tv_sec as f64
            + r.ru_stime.tv_usec as f64 / 1e6
    }
}
fn rss() -> i64 {
    unsafe {
        let mut r: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut r);
        r.ru_maxrss
    }
}
#[cfg(target_os = "linux")]
extern "C" {
    fn openblas_get_config() -> *const std::ffi::c_char;
    fn openblas_get_corename() -> *const std::ffi::c_char;
    fn openblas_get_num_threads() -> std::ffi::c_int;
}
fn check_cpu_blas() -> Result<()> {
    #[cfg(target_arch = "x86_64")]
    if !std::is_x86_feature_detected!("avx2") || !std::is_x86_feature_detected!("fma") {
        bail!("AVX2/FMA required for this host-tuned experimental binary");
    }
    #[cfg(target_os = "linux")]
    unsafe {
        let config = std::ffi::CStr::from_ptr(openblas_get_config()).to_string_lossy();
        if config.contains("USE64BITINT") {
            bail!("BLAS ABI mismatch: qwen-asr requires LP64/i32, loaded OpenBLAS is ILP64");
        }
        eprintln!(
            "cpu_blas_json={}",
            json!({"openblas_config":config,
            "openblas_core":std::ffi::CStr::from_ptr(openblas_get_corename()).to_string_lossy(),
            "openblas_threads":openblas_get_num_threads(),"rust_kernel_threads":kernels::get_num_threads(),
            "precision":"decoder-INT8-encoder-F32-prefill-F32","blas_integer_bits":32})
        );
    }
    Ok(())
}
fn main() -> Result<()> {
    let a: Vec<_> = env::args().collect();
    if a.len() != 6 {
        bail!("model threads offline|stream|paced inputs.tsv repeats");
    }
    let threads: usize = a[2].parse()?;
    let repeats: usize = a[5].parse()?;
    if !(1..=12).contains(&threads)
        || !(1..=5).contains(&repeats)
        || !["offline", "stream", "paced", "paced-reconcile"].contains(&a[3].as_str())
    {
        bail!("Invalid limits");
    }
    kernels::set_threads(threads);
    kernels::set_verbose(0);
    check_cpu_blas()?;
    let profile = env::var_os("QWEN_BENCH_PROFILE").is_some();
    let t = Instant::now();
    let mut ctx = QwenCtx::load(&a[1]).context("Model load failed")?;
    let load_seconds = t.elapsed().as_secs_f64();
    ctx.stream_chunk_sec = 2.;
    ctx.stream_unfixed_chunks = 2;
    let clock = Arc::new(Mutex::new((Instant::now(), None::<f64>)));
    let cb_clock = clock.clone();
    ctx.token_cb = Some(Box::new(move |piece| {
        if !piece.trim().is_empty() {
            let mut c = cb_clock.lock().unwrap();
            if c.1.is_none() {
                c.1 = Some(c.0.elapsed().as_secs_f64());
            }
        }
    }));
    let mut warmed = false;
    for line in fs::read_to_string(&a[4])?.lines() {
        let f: Vec<_> = line.split('\t').collect();
        if f.len() != 3 {
            bail!("Bad TSV");
        }
        let mut reader = hound::WavReader::open(f[2])?;
        let spec = reader.spec();
        if spec.channels != 1 || spec.sample_rate != 16000 || spec.bits_per_sample != 16 {
            bail!("Bad WAV");
        }
        let samples: Vec<f32> = reader
            .samples::<i16>()
            .map(|v| v.map(|s| s as f32 / 32768.))
            .collect::<std::result::Result<_, _>>()?;
        if samples.is_empty() || samples.len() > 60 * 16000 {
            bail!("Invalid duration");
        }
        ctx.set_force_language(f[1])
            .map_err(|_| anyhow::anyhow!("Unsupported language"))?;
        if !warmed {
            transcribe::transcribe_audio(&mut ctx, &samples).context("Warm-up failed")?;
            warmed = true;
        }
        if profile {
            kernels::profile_reset();
            kernels::set_profile(true);
        }
        for rep in 0..repeats {
            let cpu_start = cpu_seconds();
            let start = Instant::now();
            *clock.lock().unwrap() = (start, None);
            let mut compute = 0.;
            let mut first_provisional = None::<f64>;
            let paced = a[3].starts_with("paced");
            let reconcile = a[3] == "paced-reconcile";
            let text = if paced {
                let mut state = transcribe::StreamState::new();
                let mut reconciled = None::<String>;
                for (i, _chunk) in samples.chunks(32000).enumerate() {
                    // Simulate arrival at the END of each 2s chunk, not preloaded playback/capture.
                    let deadline = Duration::from_secs_f64(
                        ((i + 1) * 32000).min(samples.len()) as f64 / 16000.,
                    );
                    if let Some(wait) = deadline.checked_sub(start.elapsed()) {
                        std::thread::sleep(wait);
                    }
                    let tick = Instant::now();
                    let end = ((i + 1) * 32000).min(samples.len());
                    // 32 intermediate tokens are NOT enough to flush Thai tails.
                    // Final call needs room for EOS; leave intermediate settings unchanged.
                    ctx.stream_max_new_tokens = if end == samples.len() { 512 } else { 32 };
                    if end == samples.len() && reconcile {
                        // Independent utterance-final pass can correct prefix-locked drafts.
                        let final_text = transcribe::transcribe_audio(&mut ctx, &samples)
                            .context("Final reconciliation failed")?;
                        compute += tick.elapsed().as_secs_f64();
                        state.reset();
                        reconciled = Some(final_text);
                        break;
                    }
                    transcribe::stream_push_audio(
                        &mut ctx,
                        &samples[..end],
                        &mut state,
                        end == samples.len(),
                    )
                    .context("Stream step failed")?;
                    compute += tick.elapsed().as_secs_f64();
                    if first_provisional.is_none() && !state.provisional_text().trim().is_empty() {
                        first_provisional = Some(start.elapsed().as_secs_f64());
                    }
                }
                reconciled.unwrap_or_else(|| state.text())
            } else if a[3] == "stream" {
                transcribe::transcribe_stream(&mut ctx, &samples).context("Stream failed")?
            } else {
                transcribe::transcribe_audio(&mut ctx, &samples).context("Inference failed")?
            };
            let elapsed = start.elapsed().as_secs_f64();
            println!(
                "{}",
                json!({"id":f[0],"language":f[1],"text":text,"status":"ok","rep":rep,
                "elapsed_seconds":elapsed,"cpu_seconds":cpu_seconds()-cpu_start,"audio_seconds":samples.len() as f64/16000.,
                "first_token_seconds":clock.lock().unwrap().1,"first_provisional_seconds":first_provisional,
                "end_delay_seconds":if paced {Some(elapsed-samples.len() as f64/16000.)}else{None},
                "stream_final_max_new_tokens":if a[3]=="paced" {Some(512)}else{None},
                "load_seconds":load_seconds,"peak_rss_kib":rss(),
                "compute_seconds":if paced {compute}else{elapsed},"threads":threads,
                "precision":"decoder-INT8-encoder-F32-prefill-F32","engine":"native-rust","mode":a[3]})
            );
        }
    }
    if env::var_os("QWEN_BENCH_PROFILE").is_some() {
        kernels::profile_report();
    }
    Ok(())
}
