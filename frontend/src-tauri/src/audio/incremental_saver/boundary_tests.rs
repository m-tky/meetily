//! Synthetic-only regression: split transport must equal one continuous AAC encode.
use super::*;
use crate::audio::encode::encode_single_audio;

fn decode(path: &std::path::Path, rate: u32) -> Vec<u8> {
    let output = std::process::Command::new(find_ffmpeg_path().unwrap())
        .args(["-v", "warning", "-i"])
        .arg(path)
        .args(["-f", "f32le", "-ac", "1", "-ar", &rate.to_string(), "-"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "Decode warning: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[tokio::test]
async fn wav_boundaries_equal_single_encode_including_fractional_tail() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().to_path_buf();
    std::fs::create_dir(folder.join(".checkpoints")).unwrap();
    let mut full = Vec::<f32>::new();
    for (index, count) in [1_440_000, 1_440_240, 11_777].into_iter().enumerate() {
        let mut state = 100u32 + index as u32;
        let samples: Vec<f32> = (0..count)
            .map(|i| {
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                0.15 * (std::f32::consts::TAU * (300.0 + index as f32 * 211.0) * i as f32 / 48000.0)
                    .sin()
                    + 0.04 * (state as f32 / 2147483648.0 - 1.0)
            })
            .collect();
        let wav = folder.join(format!(".checkpoints/audio_chunk_{index:03}.wav"));
        write_float_checkpoint(&wav, &samples, 48000).unwrap();
        let bytes = std::fs::read(&wav).unwrap();
        assert_eq!(&bytes[58..], bytemuck::cast_slice::<f32, u8>(&samples));
        full.extend(samples);
    }
    let mut saver = IncrementalAudioSaver::new(folder.clone(), 48000).unwrap();
    saver.checkpoint_count = 3;
    let final_path = saver.finalize().await.unwrap();
    let reference = folder.join("single-encode.mp4");
    encode_single_audio(bytemuck::cast_slice(&full), 48000, 1, &reference).unwrap();
    for rate in [48000, 16000] {
        assert_eq!(
            decode(&final_path, rate),
            decode(&reference, rate),
            "Split checkpoints changed decoded samples or added priming at {rate}Hz"
        );
    }
    assert!(folder.join(".checkpoints/audio_chunk_002.wav").is_file());
}

#[tokio::test]
async fn duplicate_and_mixed_formats_fail_without_deleting_originals() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().to_path_buf();
    std::fs::create_dir(folder.join(".checkpoints")).unwrap();
    let wav = folder.join(".checkpoints/audio_chunk_000.wav");
    write_float_checkpoint(&wav, &[0.1; 480], 48000).unwrap();
    let legacy = folder.join(".checkpoints/audio_chunk_000.mp4");
    std::fs::write(&legacy, b"legacy preserved").unwrap();
    let mut saver = IncrementalAudioSaver::new(folder.clone(), 48000).unwrap();
    saver.checkpoint_count = 1;
    assert!(saver.finalize().await.is_err());
    assert!(
        recover_audio_from_checkpoints(folder.to_string_lossy().into(), 48000)
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&legacy).unwrap(), b"legacy preserved");
    assert!(wav.is_file());
    assert!(merge_codec_args(&[wav, folder.join("audio_chunk_001.mp4")], 48000).is_err());
    assert!(!folder.join("audio.mp4").exists());
}

#[tokio::test]
async fn new_recovery_keeps_wave_and_existing_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().to_path_buf();
    std::fs::create_dir(folder.join(".checkpoints")).unwrap();
    let wav = folder.join(".checkpoints/audio_chunk_000.wav");
    write_float_checkpoint(&wav, &[0.1; 11777], 48000).unwrap();
    assert!(has_audio_checkpoints(folder.to_string_lossy().into())
        .await
        .unwrap());
    let before = std::fs::read(&wav).unwrap();
    let result = recover_audio_from_checkpoints(folder.to_string_lossy().into(), 48000)
        .await
        .unwrap();
    assert_eq!(result.status, "success");
    let audio = std::fs::read(folder.join("audio.mp4")).unwrap();
    let repeated = recover_audio_from_checkpoints(folder.to_string_lossy().into(), 48000)
        .await
        .unwrap();
    assert_eq!(repeated.status, "existing");
    assert_eq!(std::fs::read(&wav).unwrap(), before);
    assert_eq!(std::fs::read(folder.join("audio.mp4")).unwrap(), audio);
}

#[tokio::test]
async fn valid_legacy_recovery_is_explicitly_partial_and_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().to_path_buf();
    std::fs::create_dir(folder.join(".checkpoints")).unwrap();
    let legacy = folder.join(".checkpoints/audio_chunk_000.mp4");
    encode_single_audio(bytemuck::cast_slice(&[0.1f32; 4800]), 48000, 1, &legacy).unwrap();
    let before = std::fs::read(&legacy).unwrap();
    let result = recover_audio_from_checkpoints(folder.to_string_lossy().into(), 48000)
        .await
        .unwrap();
    assert_eq!(result.status, "partial");
    assert!(result.message.contains("padding/timing"));
    assert_eq!(std::fs::read(&legacy).unwrap(), before);
}
