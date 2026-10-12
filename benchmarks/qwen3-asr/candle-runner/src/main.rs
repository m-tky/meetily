use anyhow::{bail, Result};
use candle_core::Device;
use qwen3_asr::{AsrInference, TranscribeOptions};
use serde_json::json;
use std::{env, fs, path::Path, time::Instant};
fn peak_rss() -> i64 {
    unsafe {
        let mut r: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut r);
        r.ru_maxrss
    }
}
fn main() -> Result<()> {
    let a: Vec<_> = env::args().collect();
    if a.len() != 5 {
        bail!("model threads inputs.tsv repeats");
    }
    let threads: usize = a[2].parse()?;
    let repeats: usize = a[4].parse()?;
    if !(1..=12).contains(&threads) || !(1..=5).contains(&repeats) {
        bail!("Invalid limits");
    }
    env::set_var("RAYON_NUM_THREADS", &a[2]);
    let start = Instant::now();
    let engine = AsrInference::load(Path::new(&a[1]), Device::Cpu)?;
    let load_seconds = start.elapsed().as_secs_f64();
    let mut warmed = false;
    for line in fs::read_to_string(&a[3])?.lines() {
        let f: Vec<_> = line.split('\t').collect();
        if f.len() != 3 {
            bail!("Invalid inputs");
        }
        let mut reader = hound::WavReader::open(f[2])?;
        let spec = reader.spec();
        if spec.channels != 1 || spec.sample_rate != 16000 || spec.bits_per_sample != 16 {
            bail!("Expected 16k mono PCM16");
        }
        let samples: Vec<f32> = reader
            .samples::<i16>()
            .map(|x| x.map(|v| v as f32 / 32768.))
            .collect::<std::result::Result<_, _>>()?;
        if samples.is_empty() || samples.len() > 60 * 16000 {
            bail!("Invalid audio length");
        }
        if !warmed {
            engine
                .transcribe_samples(&samples, TranscribeOptions::default().with_language(f[1]))?;
            warmed = true;
        }
        for rep in 0..repeats {
            let start = Instant::now();
            let result = engine
                .transcribe_samples(&samples, TranscribeOptions::default().with_language(f[1]))?;
            let elapsed_seconds = start.elapsed().as_secs_f64();
            let text = result
                .text
                .strip_prefix("<asr_text>")
                .unwrap_or(&result.text)
                .trim();
            println!(
                "{}",
                json!({"id": f[0],"language":f[1],"text":text,"raw_text":result.text,"status":"ok","rep":rep,
                "elapsed_seconds":elapsed_seconds,"audio_seconds":samples.len() as f64/16000.,
                "first_token_seconds":null,"load_seconds":load_seconds,"peak_rss_kib":peak_rss(),
                "threads":threads,"precision":"unquantized-F32-from-BF16","engine":"candle-rust","mode":"offline"})
            );
        }
    }
    Ok(())
}
