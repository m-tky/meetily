#!/usr/bin/env python3
"""Synthetic source-level regressions for the dual-input ring and unchanged DSP.
Compiles mixing.rs directly and extracts the mixer/resampler verbatim. No app
startup, capture, playback, ASR, private profile or native recording log access.
Run: nix-shell -p python3 rustc --run 'python3 scripts/verify_dual_audio_code.py'
Full native pipeline/real VAD tests are separate cargo tests (see DUAL-INPUT-CHECK).
The two remaining DSP diagnostic failures are intentional, not quality acceptance.
"""
import hashlib
import json
import pathlib
import re
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
AUDIO = ROOT / 'frontend/src-tauri/src/audio'


def region(source, start, end):
    assert source.count(start) == 1 and source.count(end) == 1
    begin = source.index(start)
    return source[begin:source.index(end, begin)].rstrip()


pipeline = (AUDIO / 'pipeline.rs').read_text()
vad = (AUDIO / 'vad.rs').read_text()
mixer = region(pipeline, 'struct ProfessionalAudioMixer;', '\n/// Simplified audio capture without broadcast channels')
resample = region(vad, '    fn resample_to_16k(', '\n    /// Flush any remaining audio and return final speech segments')
ring_path = AUDIO / 'mixing.rs'
ring = ring_path.read_text()
support = r'''
#![allow(dead_code, unused_variables)]
macro_rules! debug { ($($t:tt)*) => { { let _ = format_args!($($t)*); } }; }
type Result<T> = std::result::Result<T, &'static str>;
mod recording_state { #[derive(Clone)] pub enum DeviceType { Microphone, System } }
struct ResampleHarness { sample_rate: u32 }
'''
tests = r'''
fn tone_gain(freq: f64) -> f64 {
    let x: Vec<f32> = (0..28800).map(|i|
        (0.5*(2.0*std::f64::consts::PI*freq*i as f64/48000.0).sin()) as f32).collect();
    let y = ResampleHarness {sample_rate:48000}.resample_to_16k(&x).unwrap();
    let rms = (y[100..y.len()-100].iter().map(|v|(*v as f64).powi(2)).sum::<f64>()
        / (y.len()-200) as f64).sqrt();
    20.0*(rms / (0.5 / 2.0_f64.sqrt())).log10()
}
#[test] fn characterize_unchanged_mixer_system_unity_and_hard_saturation() {
    let mut m = ProfessionalAudioMixer::new(48000);
    assert_eq!(m.mix_window(&[0.0],&[0.7]),vec![0.7]);
    assert_eq!(m.mix_window(&[0.6,-0.6],&[0.6,-0.6]),vec![1.0,-1.0]);
}
#[test] fn characterize_resample_dc_and_length() {
    let y = ResampleHarness {sample_rate:48000}.resample_to_16k(&vec![0.25;28800]).unwrap();
    assert_eq!(y.len(),9600); assert!(y.iter().all(|x|(*x-0.25).abs()<1e-6));
}
#[test] fn characterize_actual_tone_response() {
    let g6 = tone_gain(6000.0); let g12 = tone_gain(12000.0);
    println!("synthetic tone gains: 6k={g6:.3}dB; 12k->4k alias={g12:.3}dB");
    assert!((g6 + 6.324).abs()<0.01); assert!((g12 + 13.979).abs()<0.01);
}
#[test] fn characterize_actual_alias_waveform() {
    let x: Vec<f32> = (0..28800).map(|i|
        (0.5*(2.0*std::f64::consts::PI*12000.0*i as f64/48000.0).sin()) as f32).collect();
    let y = ResampleHarness {sample_rate:48000}.resample_to_16k(&x).unwrap();
    for i in 100..200 { let expected = 0.1*(2.0*std::f64::consts::PI*4000.0*i as f64/16000.0).sin();
        assert!((y[i] as f64-expected).abs()<1e-5); }
}
// Remaining diagnostic targets, NOT agreed ASR acceptance criteria.
#[test] fn invariant_mixer_is_not_pointwise_hard_clamp() {
    let y = ProfessionalAudioMixer::new(48000).mix_window(&[0.6],&[0.6]);
    assert!(y[0]<1.0,"unchanged mixer still hard-saturates");
}
#[test] fn invariant_alias_rejection_at_least_40db_diagnostic_target() {
    assert!(tone_gain(12000.0)<=-40.0,"unchanged resampler still aliases at about -14dB");
}
'''
harness = support + '\n#[path = ' + json.dumps(str(ring_path)) + '] mod mixing;\n' + mixer + '\nimpl ResampleHarness {\n' + resample + '\n}\n' + tests
hashes = {name: hashlib.sha256(text.encode()).hexdigest() for name, text in
          [('mixing.rs', ring), ('mixer', mixer), ('resample_to_16k', resample)]}
with tempfile.TemporaryDirectory(prefix='meetily-synthetic-code-') as tmp:
    source = pathlib.Path(tmp) / 'test.rs'
    binary = pathlib.Path(tmp) / 'test'
    source.write_text(harness)
    build = subprocess.run(['rustc', '--edition=2021', '--test', str(source), '-o', str(binary)], capture_output=True, text=True)
    if build.returncode:
        print(build.stderr)
        raise SystemExit(build.returncode)
    runs = {}
    for name, arguments, expected, counts in [
        ('regressions', ['--skip', 'invariant_'], 0, (20, 0)),
        ('remaining_dsp_diagnostics', ['invariant_'], 101, (0, 2)),
    ]:
        run = subprocess.run([str(binary), *arguments, '--test-threads=1', '--nocapture'], capture_output=True, text=True)
        print(run.stdout)
        print(run.stderr)
        match = re.search(r'test result: .*? (\d+) passed; (\d+) failed;', run.stdout)
        observed = tuple(map(int, match.groups())) if match else None
        runs[name] = {'exit_code': run.returncode, 'expected_exit_code': expected,
                      'expected_pass_fail_counts': counts, 'observed_pass_fail_counts': observed,
                      'counts_match': observed == counts, 'stdout': run.stdout, 'stderr': run.stderr}
    report = {'synthetic_only': True, 'production_source_compiled_verbatim': True,
              'full_app_integration': False, 'capture_playback_asr': False,
              'proves_user_recognition_cause': False, 'snippet_sha256': hashes, 'runs': runs}
    report['expected_results_reproduced'] = all(r['exit_code'] == r['expected_exit_code'] and r['counts_match'] for r in runs.values())
    log = ROOT.parent / 'logs/meetily-dual-postfix-source-tests.json'
    log.write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps({k:v for k,v in report.items() if k != 'runs'}, indent=2))
    raise SystemExit(0 if report['expected_results_reproduced'] else 1)
