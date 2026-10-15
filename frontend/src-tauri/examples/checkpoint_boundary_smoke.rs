//! Synthetic PCM only: exercise the actual saver without devices/ASR/network.
use app_lib::audio::{
    encode::encode_single_audio,
    incremental_saver::IncrementalAudioSaver,
    recording_state::{AudioChunk, DeviceType},
};
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let root = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or_else(|| anyhow::anyhow!("New output directory required"))?,
    );
    anyhow::ensure!(!root.exists(), "Refusing an existing directory");
    std::fs::create_dir_all(root.join(".checkpoints"))?;
    let mut saver = IncrementalAudioSaver::new(root.clone(), 48000)?;
    let mut full = Vec::new();
    for (index, count) in [1_440_000, 1_440_240, 11_777].into_iter().enumerate() {
        let mut state = 100u32 + index as u32;
        let data: Vec<f32> = (0..count)
            .map(|i| {
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                0.15 * (std::f32::consts::TAU * (300.0 + index as f32 * 211.0) * i as f32 / 48000.0)
                    .sin()
                    + 0.04 * (state as f32 / 2147483648.0 - 1.0)
            })
            .collect();
        full.extend_from_slice(&data);
        saver.add_chunk(AudioChunk { capture_timestamp: None,
            data,
            sample_rate: 48000,
            timestamp: index as f64 * 30.0,
            chunk_id: index as u64,
            device_type: DeviceType::System,
        })?;
    }
    saver.finalize().await?;
    encode_single_audio(
        bytemuck::cast_slice(&full),
        48000,
        1,
        &root.join("single-encode.mp4"),
    )?;
    std::fs::write(root.join("source.f32"), bytemuck::cast_slice(&full))?;
    println!(
        "{} samples; synthetic artifacts at {}",
        full.len(),
        root.display()
    );
    Ok(())
}
