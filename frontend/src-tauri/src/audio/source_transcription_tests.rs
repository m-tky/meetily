//! Offline synthetic fixtures; no device capture, playback, ASR or cloud.
use super::tests::synthetic_pipeline;
use super::*;

#[cfg(target_os = "linux")]
fn speech(text: &str, pitch: &str) -> Vec<f32> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let wav = Command::new("espeak-ng")
        .args(["--stdout", "-v", "en", "-s", "160", "-p", pitch, text])
        .output().unwrap();
    assert!(wav.status.success());
    let mut decoder = Command::new("ffmpeg")
        .args(["-v", "error", "-i", "pipe:0", "-ac", "1", "-ar", "48000", "-f", "f32le", "pipe:1"])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut input = decoder.stdin.take().unwrap();
    let writer = std::thread::spawn(move || input.write_all(&wav.stdout));
    let output = decoder.wait_with_output().unwrap();
    writer.join().unwrap().unwrap();
    assert!(output.status.success());
    output.stdout.chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect()
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn overlapping_inputs_match_independent_vad_and_original_recording_mix() {
    let mut mic = vec![0.0; 96000];
    mic.extend(speech("Microphone input keeps the entire final sentence separately.", "35"));
    let mut pc = vec![0.0; 24000];
    pc.extend(speech("Computer audio is another independent input. Do not mix its transcript.", "75"));
    let length = mic.len().max(pc.len()) + 1001;
    mic.resize(length, 0.0); pc.resize(length, 0.0);
    assert_ne!(length % 5760, 0, "exercise exact short Stop tail");
    let (mut pipeline, mut recording, mut transcripts) = synthetic_pipeline(true, true);
    pipeline.ring_buffer = AudioMixerRingBuffer::with_window(48000, true, true, 120);
    let mut mic_vad = ContinuousVadProcessor::new(48000, VAD_REDEMPTION_TIME_MS).unwrap();
    let mut pc_vad = ContinuousVadProcessor::new(48000, VAD_REDEMPTION_TIME_MS).unwrap();
    let mut reference_mixer = ProfessionalAudioMixer::new(48000);
    let (mut mic_expected, mut pc_expected, mut mix_expected) = (Vec::new(), Vec::new(), Vec::new());
    for (m, s) in mic.chunks(5760).zip(pc.chunks(5760)) {
        mic_expected.extend(mic_vad.process_audio(m).unwrap());
        pc_expected.extend(pc_vad.process_audio(s).unwrap());
        mix_expected.extend(reference_mixer.mix_window(m, s));
        pipeline.ring_buffer.add_samples(DeviceType::Microphone, m.to_vec());
        pipeline.ring_buffer.add_samples(DeviceType::System, s.to_vec());
        pipeline.drain_mixed_audio(false).unwrap();
    }
    mic_expected.extend(mic_vad.flush().unwrap()); pc_expected.extend(pc_vad.flush().unwrap());
    pipeline.flush_remaining_audio().unwrap();
    let mut actual = Vec::new();
    while let Ok(c) = transcripts.try_recv() { actual.push(c); }
    for (source, expected) in [(DeviceType::Microphone, mic_expected), (DeviceType::System, pc_expected)] {
        let expected: Vec<_> = expected.into_iter().filter(|s| s.samples.len() >= 800).collect();
        let got: Vec<_> = actual.iter().filter(|c| c.device_type == source).collect();
        assert!(!expected.is_empty(), "must exercise real source VAD");
        assert_eq!(got.len(), expected.len());
        for (g, e) in got.iter().zip(expected) {
            assert_eq!(g.data, e.samples, "ASR must receive unmixed source PCM");
            assert_eq!(g.timestamp, e.start_timestamp_ms / 1000.0);
            assert_eq!(g.sample_rate, 16000);
        }
    }
    let ids: std::collections::HashSet<_> = actual.iter().map(|c| c.chunk_id).collect();
    assert_eq!(ids.len(), actual.len());
    let mut saved = Vec::new(); while let Ok(c) = recording.try_recv() { saved.extend(c.data); }
    assert_eq!(saved, mix_expected, "recording mixer and exact length are unchanged");
    pipeline.flush_remaining_audio().unwrap();
    assert!(recording.try_recv().is_err()); assert!(transcripts.try_recv().is_err());
}

#[tokio::test]
async fn selected_source_only_has_its_own_vad_and_disabled_lane_is_not_inferred() {
    for mic in [true, false] {
        let (mut p, _, _) = synthetic_pipeline(mic, !mic);
        assert_eq!(p.mic_vad_processor.is_some(), mic);
        assert_eq!(p.system_vad_processor.is_some(), !mic);
        p.dispatch_source_segments(vec![super::super::vad::SpeechSegment {
            samples: vec![0.1; 1600], start_timestamp_ms: 2000.0,
            end_timestamp_ms: 2100.0, confidence: 0.0,
        }], if mic { DeviceType::Microphone } else { DeviceType::System });
        assert_eq!(p.state.get_error_count(), 1, "closed ASR channel is reported");
    }
}

#[tokio::test]
async fn broken_asr_receiver_does_not_stop_recording_tail() {
    let (mut p, mut recording, transcripts) = synthetic_pipeline(false, true);
    drop(transcripts);
    p.dispatch_source_segments(vec![super::super::vad::SpeechSegment {
        samples: vec![0.1; 1600], start_timestamp_ms: 0.0,
        end_timestamp_ms: 100.0, confidence: 0.0,
    }], DeviceType::System);
    p.ring_buffer.add_samples(DeviceType::System, vec![0.0; 24001]);
    p.flush_remaining_audio().unwrap();
    assert_eq!(recording.try_recv().unwrap().data.len(), 24001);
    assert!(p.state.get_error_count() > 0);
}
