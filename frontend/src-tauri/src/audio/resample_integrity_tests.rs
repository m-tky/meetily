use super::resample;

#[test]
fn finite_resample_preserves_rational_duration_even_for_short_input() {
    for (from, to) in [
        (48000u32, 16000u32),
        (44100, 16000),
        (8000, 16000),
        (16000, 48000),
    ] {
        for n in [1usize, 17, 257, 1001, 48000] {
            let input = vec![0.1; n];
            let output = resample(&input, from, to).unwrap();
            let expected = (n as u64 * to as u64 + from as u64 / 2) / from as u64;
            assert_eq!(output.len(), expected as usize, "{from}->{to}, input={n}");
            assert!(output.iter().all(|s| s.is_finite()));
        }
    }
}

#[test]
fn finite_resample_keeps_tail_signal_in_its_original_time_interval() {
    let mut input = vec![0.0; 48000];
    // A 6ms marker at the end must not disappear into an undrained sinc buffer.
    input[48000 - 288..].fill(0.5);
    let output = resample(&input, 48000, 16000).unwrap();
    let tail_energy: f32 = output.iter().skip(15920).map(|s| s * s).sum();
    assert!(
        tail_energy > 10.0,
        "end marker lost/moved: energy={tail_energy}"
    );
}

#[test]
fn finite_resample_preserves_interior_impulse_time() {
    let mut input = vec![0.0; 48000];
    input[24000] = 1.0;
    let output = resample(&input, 48000, 16000).unwrap();
    let peak = output
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        .unwrap()
        .0;
    assert!(
        peak.abs_diff(8000) <= 1,
        "half-second impulse moved to {peak}/16000"
    );
}

#[test]
fn finite_resample_retains_anti_alias_filter() {
    let input: Vec<f32> = (0..48000)
        .map(|i| (2.0 * std::f32::consts::PI * 12000.0 * i as f32 / 48000.0).sin())
        .collect();
    let output = resample(&input, 48000, 16000).unwrap();
    let rms = (output[1000..15000].iter().map(|s| s * s).sum::<f32>() / 14000.0).sqrt();
    assert!(rms < 0.001, "12kHz alias leaked: RMS {rms}");
}

#[test]
fn finite_resample_rejects_zero_sample_rates() {
    assert!(resample(&[0.1], 0, 16000).is_err());
    assert!(resample(&[0.1], 16000, 0).is_err());
    assert!(resample(&[], 0, 0).is_err());
}

#[test]
fn finite_resample_identity_and_empty_do_not_change_samples() {
    let input = vec![-0.5, 0.25, 0.0, 0.75];
    assert_eq!(resample(&input, 16000, 16000).unwrap(), input);
    assert!(resample(&[], 48000, 16000).unwrap().is_empty());
}
