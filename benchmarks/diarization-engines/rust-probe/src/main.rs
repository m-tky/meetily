use parakeet_rs::sortformer::{DiarizationConfig, Sortformer, SpeakerSegment, StreamingProfile};
use parakeet_rs::{ExecutionConfig, ExecutionProvider};
use serde_json::json;
use std::{
    env,
    io::{self, Write},
    time::{Duration, Instant},
};

fn emit(value: serde_json::Value) {
    println!("{}", value);
    io::stdout().flush().unwrap();
}
fn segs(segs: &[SpeakerSegment]) -> Vec<serde_json::Value> {
    segs.iter().map(|s| json!({"speaker":s.speaker_id,"start":s.start as f64/16000.,"end":s.end as f64/16000.})).collect()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 5 {
        return Err("usage: probe model.onnx approved.wav low|very-low|ultra paced|fast".into());
    }
    let mut reader = hound::WavReader::open(&args[2])?;
    let spec = reader.spec();
    if spec.sample_rate != 16000 || spec.channels != 1 {
        return Err("mono 16k required".into());
    }
    let audio: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int if spec.bits_per_sample == 16 => reader
            .samples::<i16>()
            .map(|v| v.map(|x| x as f32 / 32768.))
            .collect::<Result<_, _>>()?,
        _ => return Err("PCM16 or float32 required".into()),
    };
    if audio.len() > 600 * 16000 || !audio.iter().all(|x| x.is_finite()) {
        return Err("invalid/beyond bounded input".into());
    }
    let profile = match args[3].as_str() {
        "low" => StreamingProfile::low_latency(),
        "very-low" => StreamingProfile::very_low_latency(),
        "ultra" => StreamingProfile::ultra_low_latency(),
        _ => return Err("unknown profile".into()),
    };
    let config = ExecutionConfig::new()
        .with_environment(parakeet_rs::ort::init().build()?)
        .with_execution_provider(ExecutionProvider::Cpu)
        .with_intra_threads(4)
        .with_inter_threads(1);
    let load_start = Instant::now();
    let mut diar = Sortformer::with_config(&args[1], Some(config), DiarizationConfig::default())?;
    diar.set_profile(profile)?;
    emit(
        json!({"phase":"ready","load_seconds":load_start.elapsed().as_secs_f64(),"cpu_threads":4,"buffer_latency":diar.latency()}),
    );
    let start = Instant::now();
    let mut supplied = 0usize;
    let feed_samples = (profile.chunk_len + profile.right_context) * 1280;
    let stride_samples = profile.chunk_len * 1280;
    for chunk in audio.chunks(1920) {
        supplied += chunk.len();
        let input_seconds = supplied as f64 / 16000.;
        if args[4] == "paced" {
            let deadline = Duration::from_secs_f64(input_seconds);
            if deadline > start.elapsed() {
                std::thread::sleep(deadline - start.elapsed());
            }
        }
        let t = Instant::now();
        let segments = diar.feed(chunk)?;
        let push_seconds = t.elapsed().as_secs_f64();
        let processed = if supplied < feed_samples {
            0
        } else {
            ((supplied - feed_samples) / stride_samples + 1) * stride_samples
        };
        emit(
            json!({"phase":"push","input_seconds":input_seconds,"push_seconds":push_seconds,"wall_seconds":start.elapsed().as_secs_f64(),"processed_seconds":processed as f64/16000.,"segments":segs(&segments)}),
        );
    }
    let t = Instant::now();
    let segments = diar.flush()?;
    emit(
        json!({"phase":"finish","flush_seconds":t.elapsed().as_secs_f64(),"wall_seconds":start.elapsed().as_secs_f64(),"processed_seconds":supplied as f64/16000.,"segments":segs(&segments)}),
    );
    Ok(())
}
