#!/usr/bin/env python3
"""Private artificial dual-input transport A/B; never uses host hardware or ASR.
A dedicated -n Pulse server, two null sinks, and an exact-source ALSA config
are created for each run. No app profile, user audio, desktop or cloud is used.
"""
import argparse
import array
import json
import math
import os
import pathlib
import shutil
import signal
import subprocess
import tempfile
import time

REPO = pathlib.Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--probe', required=True)
parser.add_argument('--alsa-plugin-dir', required=True)
parser.add_argument('--mode', required=True, choices=['fragment', 'adjust', 'timed', 'native'])
parser.add_argument('--native-input-lifecycle', action='store_true', help='Remove/recreate only the owned private native mic and verify no fallback')
parser.add_argument('--shared-source-clock', action='store_true', help='Both capture backends observe ONE private null-sink monitor; one playback gives a known common signal timeline')
parser.add_argument('--lifecycle-smoke', help='Optional native synthetic Pulse pause/resume/removal/reconnect regression, on this PRIVATE server only')
parser.add_argument('--seconds', type=int, default=20, choices=[20, 60])
parser.add_argument('--window-ms', type=int, default=600, choices=[600, 120])
args = parser.parse_args()
assert not args.native_input_lifecycle or args.mode == 'native'
probe = pathlib.Path(args.probe).resolve(strict=True)
plugin = pathlib.Path(args.alsa_plugin_dir).resolve(strict=True) / 'libasound_module_pcm_pulse.so'
assert plugin.is_file()
root = pathlib.Path(tempfile.mkdtemp(prefix='dual-latency-', dir=REPO.parent / 'runtime'))
root.chmod(0o700)
(root / '.private-latency-test').write_text('artificial-only\n')
if args.native_input_lifecycle: (root / 'test-input-lifecycle').touch()
for d in ['home', 'run', 'config', 'cache', 'data']:
    (root / d).mkdir(mode=0o700)
env = dict(os.environ)
env.update(HOME=str(root / 'home'), XDG_RUNTIME_DIR=str(root / 'run'),
           XDG_CONFIG_HOME=str(root / 'config'), XDG_CACHE_HOME=str(root / 'cache'),
           XDG_DATA_HOME=str(root / 'data'), PULSE_SERVER='unix:' + str(root / 'pulse'),
           ALSA_CONFIG_PATH=str(root / 'alsa.conf'), MEETILY_DISABLE_ANALYTICS='1',
           MEETILY_AUDIO_TRANSPORT_DIAGNOSTICS='1', MEETILY_PRIVATE_PROBE_WINDOW_MS=str(args.window_ms),
           MEETILY_PRIVATE_CPAL_TIMING_PROBE='1' if args.mode == 'timed' else '0')
mic_monitor = 'fixture_pc.monitor' if args.shared_source_clock else 'fixture_mic.monitor'
(root / 'alsa.conf').write_text('defaults.namehint.showall off\ndefaults.namehint.basic on\ndefaults.namehint.extended off\n'
    'defaults.ctl.card 0\nctl.hw { @args [ CARD ] @args.CARD { type string default 0 } type hw card $CARD }\n'
    'pcm_type.pulse { lib ' + json.dumps(str(plugin)) + ' }\n'
    + '\n'.join('pcm.' + name + ' { type pulse server ' + json.dumps(env['PULSE_SERVER'])
        + ' device ' + json.dumps(mic_monitor) + ' hint { show on description "PRIVATE ARTIFICIAL ONLY" ioid "Input" } }'
        for name in ['!default', 'fixture_mic'])
    + '\nctl.!default { type hw card 999 }\n')
report = {'mode': args.mode, 'profile': str(root), 'duration_s': args.seconds,
          'real_microphone_used': False, 'physical_playback': False, 'cloud': False,
          'shared_source_clock': args.shared_source_clock, 'window_ms': args.window_ms, 'completed': False}
processes, handles = [], []
def spawn(cmd, log):
    h = open(root / log, 'ab'); handles.append(h)
    p = subprocess.Popen(cmd, env=env, stdout=h, stderr=h, start_new_session=True)
    processes.append(p)
    return p
def wait(predicate, timeout=15):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        if predicate(): return
        time.sleep(.02)
    raise RuntimeError('private probe stage timed out')
def pactl(kind):
    return json.loads(subprocess.check_output(['pactl', '--server=' + env['PULSE_SERVER'], '--format=json', 'list', kind], env=env))
try:
    modules = ['--load=module-native-protocol-unix socket=' + str(root / 'pulse') + ' auth-anonymous=1',
        '--load=module-null-sink sink_name=fixture_pc rate=48000 channels=1',
        '--load=module-null-sink sink_name=fixture_mic rate=48000 channels=1']
    if args.mode == 'native':
        modules.append('--load=module-remap-source source_name=fixture_native_mic channels=1 master=' + mic_monitor)
    spawn(['pulseaudio', '--daemonize=no', '--exit-idle-time=-1', '--use-pid-file=no', '--disallow-exit', '-n', *modules], 'pulse.log')
    wait(lambda: (root / 'pulse').is_socket())
    expected_sources = {'fixture_mic.monitor', 'fixture_pc.monitor'}
    if args.mode == 'native': expected_sources.add('fixture_native_mic')
    assert {s['name'] for s in pactl('sources')} == expected_sources
    assert all(m['name'] in ({'module-native-protocol-unix', 'module-null-sink', 'module-remap-source'} if args.mode == 'native' else {'module-native-protocol-unix', 'module-null-sink'}) for m in pactl('modules'))
    report['private_sources_verified'] = True
    # Identical periodic 100ms tone bursts, separated by digital zeroes.
    tone = array.array('f', (0.1 * math.sin(2 * math.pi * 700 * (i % 96000) / 48000)
        if 48000 <= i % 96000 < 52800 else 0.0 for i in range(48000 * args.seconds)))
    if __import__('sys').byteorder != 'little': tone.byteswap()
    (root / 'pulses.f32').write_bytes(tone.tobytes())
    capture = spawn([str(probe), str(root), args.mode], 'probe.log')
    def ready():
        assert capture.poll() is None, 'probe exited before ready'
        return (root / 'ready').exists()
    wait(ready)
    source_ids = {s['name']: s['index'] for s in pactl('sources')}
    captured = pactl('source-outputs')
    from collections import Counter
    routes = Counter([source_ids[mic_monitor], source_ids['fixture_pc.monitor']])
    if args.mode == 'native': routes[source_ids['fixture_native_mic']] += 1
    assert Counter(s['source'] for s in captured) == routes
    report['exact_private_capture_routes_verified'] = True
    began = time.monotonic()
    playbacks = []
    launches = []
    for sink in (['fixture_pc'] if args.shared_source_clock else ['fixture_mic', 'fixture_pc']):
        launches.append(time.monotonic() - began)
        playbacks.append(spawn(['paplay', '--raw', '--format=float32le', '--rate=48000', '--channels=1',
            '--device=' + sink, str(root / 'pulses.f32')], sink + '-playback.log'))
    report['playback_launch_skew_ms'] = (launches[-1] - launches[0]) * 1000
    latencies = []
    deadline = began + args.seconds + 15
    while any(p.poll() is None for p in playbacks):
        assert capture.poll() is None
        assert time.monotonic() < deadline
        pc_id = next(s['index'] for s in pactl('sources') if s['name'] == 'fixture_pc.monitor')
        streams = [s for s in pactl('source-outputs') if s['source'] == pc_id]
        assert len(streams) == (2 if args.shared_source_clock else 1)
        latencies.extend(s.get('buffer_latency_usec') for s in streams)
        time.sleep(.5)
    assert all(p.returncode == 0 for p in playbacks)
    time.sleep(1)
    (root / 'stop').touch()
    assert capture.wait(timeout=15) == 0
    transport = json.loads((root / 'probe-result.json').read_text())
    report['transport'] = transport
    report['pc_buffer_latency_usec_series'] = latencies
    expected = args.seconds // 2
    roles = transport['roles_mic_pc']
    report['expected_pulses_per_input'] = expected
    report['all_pulses_preserved'] = all(len(r['input_onsets_s']) == expected and len(r['output_onsets_s']) == expected for r in roles)
    report['output_peer_skew_ms'] = [(b-a)*1000 for a,b in zip(roles[0]['output_onsets_s'],roles[1]['output_onsets_s'])]
    report['handoff_peer_skew_ms'] = [(b-a)*1000 for a,b in zip(roles[0]['onset_handoff_s'],roles[1]['onset_handoff_s'])]
    integrity_keys = ['late_samples', 'overflow_samples', 'stall_episodes', 'capture_errors']
    if args.mode in ['timed', 'native']: integrity_keys += ['invalid_timestamps', 'gap_samples', 'overlap_samples']
    report['native_input_lifecycle_verified'] = not args.native_input_lifecycle or transport['native_input_lifecycle_passed']
    report['sync_healthy'] = all(transport[k] == 0 for k in integrity_keys) and report['native_input_lifecycle_verified']
    # Fixed probe targets, not a claim of hardware sync or recognition quality.
    report['handoff_cadence_bounded'] = all(r['handoff_gap_p95_ms'] <= 150 and r['handoff_gap_max_ms'] <= 300 for r in roles)
    skew = report['output_peer_skew_ms']
    report['output_skew_within_probe_target'] = (report['all_pulses_preserved'] and bool(skew)
        and max(abs(v) for v in skew) <= 100.000001 and max(skew) - min(skew) <= 20.000001)
    report['shared_capture_clock_verified'] = (not args.shared_source_clock or args.mode not in ['native', 'timed'] or
        (bool(skew) and max(abs(v) for v in skew) <= 20.000001 and max(skew)-min(skew) <= 5.000001))
    report['passed_latency_probe'] = (report['all_pulses_preserved'] and report['sync_healthy']
        and report['handoff_cadence_bounded'] and report['output_skew_within_probe_target']
        and report['shared_capture_clock_verified'])
    if args.lifecycle_smoke:
        lifecycle = pathlib.Path(args.lifecycle_smoke).resolve(strict=True)
        with open(root / 'lifecycle.log', 'wb') as out:
            result = subprocess.run([str(lifecycle)], env=env, stdout=out, stderr=out, timeout=60)
        assert result.returncode == 0
        assert {s['name'] for s in pactl('sources')} == expected_sources
        report['private_lifecycle_smoke_passed'] = True
    report['completed'] = True
    report['passed_transport'] = report['all_pulses_preserved'] and report['sync_healthy']
except Exception as e:
    report['exception_type'] = type(e).__name__
    report['passed_transport'] = False
finally:
    for p in reversed(processes):
        if p.poll() is None:
            try:
                os.killpg(p.pid, signal.SIGTERM); p.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(p.pid, signal.SIGKILL); p.wait()
            except ProcessLookupError: pass
    for h in handles: h.close()
    report['owned_helpers_exited'] = all(p.poll() is not None for p in processes)
    report['passed_transport'] = report.get('passed_transport', False) and report['owned_helpers_exited']
    report['passed'] = (report.get('completed', False) and report['passed_transport']
        and report.get('passed_latency_probe', False))
    (root / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))
    raise SystemExit(0 if report['passed'] else 1)
