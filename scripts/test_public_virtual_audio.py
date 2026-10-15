#!/usr/bin/env python3
"""Operator-approved Japanese PUBLIC fixture -> PRIVATE virtual audio -> local Qwen.
Never connects to host Pulse or real ALSA devices. No physical playback/cloud.
Use --dual to add a synthetic-silence microphone on another private null sink.
This is a diagnostic: empty ASR is reported as failure, not hidden or corrected.
"""
import argparse
import array
import hashlib
import json
import math
import os
import pathlib
import re
import shutil
import signal
import socket
import sqlite3
import subprocess
import sys
import tempfile
import time
import urllib.request

from dual_latency_metrics import summarize_transport

REPO = pathlib.Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
for name in ['binary', 'bootstrap', 'driver', 'worker', 'model', 'ort', 'alsa-plugin-dir']:
    parser.add_argument('--' + name, required=True)
parser.add_argument('--dual', action='store_true')
parser.add_argument('--native-mic', action='store_true', help='Explicit native Pulse input on a private remapped zero source; requires --dual')
parser.add_argument('--pc-db', type=float, choices=[0.0, -20.0], default=0.0,
                    help='Public fixture only: optionally attenuate by 20dB in float32')
parser.add_argument('--expect-normalization', action='store_true')
parser.add_argument('--expect-fragment-100ms', action='store_true', help='Verify private PC stream negotiated 100ms delivery and sampled active buffer latency below 1.2s')
parser.add_argument('--select-via-ui', action='store_true', help='Choose devices through the visible pickers and verify reopen persistence')
parser.add_argument('--transport-diagnostics', action='store_true', help='Opt-in numeric per-source handoff/mix timing in this PRIVATE profile only')
parser.add_argument('--expect-window-ms', type=int, choices=[600, 120], help='Require the configured live mixer window and full output block sample counts')
parser.add_argument('--playback-latency-ms', type=int, choices=[50], help='Private paplay clock/latency control only; does not change native capture settings')
parser.add_argument('--display', default=':113')
parser.add_argument('--port', type=int, default=4463)
args = parser.parse_args()
assert not args.native_mic or args.dual
mic_identity = 'pulse-input:fixture_native_mic' if args.native_mic else 'fixture_mic'
mic_source = 'fixture_native_mic' if args.native_mic else 'fixture_mic.monitor'
for name in ['binary', 'bootstrap', 'driver', 'worker', 'model', 'ort', 'alsa_plugin_dir']:
    setattr(args, name, str(pathlib.Path(getattr(args, name)).resolve(strict=True)))
# A fixed previously used public fixture, NEVER a user microphone recording.
fixture = (REPO.parent / 'runtime/qwen3-asr/artifacts/fleurs-ja_jp-validation-0-1519.wav').resolve(strict=True)
assert fixture.parent == (REPO.parent / 'runtime/qwen3-asr/artifacts').resolve()
probe = subprocess.run(['ffprobe', '-v', 'error', '-show_entries', 'format=duration', '-of', 'json', str(fixture)], capture_output=True, check=True)
source_seconds = float(json.loads(probe.stdout)['format']['duration'])
assert 0 < source_seconds <= 30
root = pathlib.Path(tempfile.mkdtemp(prefix='public-virtual-', dir=REPO.parent / 'runtime'))
root.chmod(0o700)
(root / '.synthetic-gui-test').write_text('synthetic-only\n')
for name in ['home/Documents', 'home/Downloads', 'data', 'config', 'cache', 'run']:
    (root / name).mkdir(parents=True)
(root / 'run').chmod(0o700)
(root / 'config/user-dirs.dirs').write_text(f'XDG_DOCUMENTS_DIR="{root}/home/Documents"\nXDG_DOWNLOAD_DIR="{root}/home/Downloads"\n')
alsa = root / 'private-alsa.conf'
if args.dual:
    plugin = pathlib.Path(args.alsa_plugin_dir) / 'libasound_module_pcm_pulse.so'
    assert plugin.is_file()
    binding = 'type pulse server ' + json.dumps('unix:' + str(root / 'pulse')) + ' device "fixture_mic.monitor"'
    alsa.write_text('defaults.namehint.showall off\ndefaults.namehint.basic on\ndefaults.namehint.extended off\n'
        + 'defaults.ctl.card 0\nctl.hw { @args [ CARD ] @args.CARD { type string default 0 } type hw card $CARD }\n'
        + 'pcm_type.pulse { lib ' + json.dumps(str(plugin)) + ' }\n'
        + 'pcm.!default { ' + binding + ' hint { show on description "PRIVATE FIXTURE ONLY" ioid "Input" } }\n'
        + 'pcm.fixture_mic { ' + binding + ' hint { show on description "PRIVATE FIXTURE ONLY" ioid "Input" } }\n'
        + 'ctl.!default { type hw card 999 }\n')
else:
    alsa.write_text('pcm.!default { type hw card 999 }\nctl.!default { type hw card 999 }\n')
keep = ['PATH', 'LD_LIBRARY_PATH', 'FONTCONFIG_FILE', 'FONTCONFIG_PATH', 'GTK_PATH', 'GI_TYPELIB_PATH', 'XDG_DATA_DIRS', 'GSETTINGS_SCHEMA_DIR', 'LOCALE_ARCHIVE', 'LANG', 'LC_ALL', 'TZ']
env = {k: os.environ[k] for k in keep if k in os.environ}
env.update(HOME=str(root / 'home'), XDG_DATA_HOME=str(root / 'data'), XDG_CONFIG_HOME=str(root / 'config'), XDG_CACHE_HOME=str(root / 'cache'), XDG_RUNTIME_DIR=str(root / 'run'), DISPLAY=args.display, GDK_BACKEND='x11', DBUS_SESSION_BUS_ADDRESS=f'unix:path={root}/bus', PULSE_SERVER='unix:' + str(root / 'pulse'), ALSA_CONFIG_PATH=str(alsa), TAURI_WEBVIEW_AUTOMATION='true', MEETILY_DISABLE_ANALYTICS='1', NEXT_TELEMETRY_DISABLED='1')
if args.transport_diagnostics:
    env['MEETILY_AUDIO_TRANSPORT_DIAGNOSTICS'] = '1'
report = {'mode': 'virtual-dual' if args.dual else 'virtual-PC-only', 'profile': str(root), 'public_fixture': fixture.name, 'source_seconds': source_seconds, 'source_sha256': hashlib.sha256(fixture.read_bytes()).hexdigest(), 'language': 'ja', 'public_fixture_input_db': args.pc_db, 'real_microphone_used': False, 'physical_playback': False, 'cloud': False, 'content_exported': False, 'native_pulse_mic_selected': args.native_mic, 'completed': False}
processes, handles = [], []
session = None
stage = 'bootstrap'
base = f'http://127.0.0.1:{args.port}'


def spawn(command, name):
    handle = open(root / name, 'ab')
    handles.append(handle)
    process = subprocess.Popen(command, env=env, stdout=handle, stderr=handle, start_new_session=True)
    processes.append(process)
    return process


def request(method, path, body=None):
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(base + path, data, method=method, headers={'Content-Type': 'application/json'})
    with urllib.request.urlopen(req, timeout=120) as response:
        value = json.load(response).get('value')
    if isinstance(value, dict) and value.get('error'):
        raise RuntimeError('WebDriver operation failed (details withheld)')
    return value


def execute(script, *values):
    return request('POST', f'/session/{session}/execute/sync', {'script': script, 'args': list(values)})


def invoke(name, values=None):
    value = request('POST', f'/session/{session}/execute/async', {'script': 'const done=arguments[arguments.length-1];window.__TAURI_INTERNALS__.invoke(arguments[0],arguments[1]).then(done).catch(()=>done({error:true}));', 'args': [name, values or {}]})
    if isinstance(value, dict) and value.get('error'):
        raise RuntimeError('Native command failed (details withheld)')
    return value


def wait(predicate, seconds=90):
    until = time.monotonic() + seconds
    while time.monotonic() < until:
        if predicate():
            return
        time.sleep(.25)
    raise RuntimeError('Diagnostic stage timed out (details withheld)')


def click(label):
    wait(lambda: execute('return [...document.querySelectorAll("button")].some(e=>e.getAttribute("aria-label")===arguments[0]&&!e.disabled);', label))
    execute('[...document.querySelectorAll("button")].find(e=>e.getAttribute("aria-label")===arguments[0]&&!e.disabled).click();', label)


try:
    subprocess.run([args.bootstrap, str(root)], env=env, check=True, capture_output=True)
    data = root / 'data/com.meetily.ai'
    (data / 'onboarding-status.json').write_text(json.dumps({'status': {'version': '1.0', 'completed': True, 'current_step': 5, 'model_status': {'parakeet': 'not_downloaded', 'summary': 'not_downloaded'}, 'last_updated': '2026-01-01T00:00:00Z'}}))
    wrapper = root / 'launch-app.sh'
    wrapper.write_text('#!' + shutil.which('bash') + '\nexec ' + json.dumps(args.binary) + ' >>' + json.dumps(str(root / 'native.log')) + ' 2>&1\n')
    wrapper.chmod(0o700)
    stage = 'private-audio-isolation'
    assert re.fullmatch(r':\d+', args.display)
    display_number = args.display[1:]
    assert not pathlib.Path('/tmp/.X11-unix/X' + display_number).exists()
    assert not pathlib.Path('/tmp/.X' + display_number + '-lock').exists()
    # Reject live owners, allow our previous connection's TIME_WAIT. Never
    # attach to another operator's WebDriver if our new driver fails to bind.
    with socket.socket() as port_probe:
        port_probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        port_probe.bind(('127.0.0.1', args.port))
    spawn(['Xvfb', args.display, '-screen', '0', '1280x900x24', '-nolisten', 'tcp'], 'xvfb.log')
    spawn(['dbus-daemon', '--session', '--nofork', f'--address=unix:path={root}/bus'], 'dbus.log')
    modules = ['--load=' + f'module-native-protocol-unix socket={root}/pulse auth-anonymous=1', '--load=module-null-sink sink_name=fixture_pc rate=48000 channels=1']
    if args.dual:
        modules.append('--load=module-null-sink sink_name=fixture_mic rate=48000 channels=1')
    if args.native_mic:
        modules.append('--load=module-remap-source master=fixture_mic.monitor source_name=fixture_native_mic channels=1')
    spawn(['pulseaudio', '--daemonize=no', '--exit-idle-time=-1', '--use-pid-file=no', '--disallow-exit', '-n', *modules], 'pulse.log')
    wait(lambda: (root / 'pulse').is_socket())
    sources = json.loads(subprocess.check_output(['pactl', '--format=json', 'list', 'sources'], env=env))
    expected = {'fixture_pc.monitor', 'fixture_mic.monitor'} if args.dual else {'fixture_pc.monitor'}
    if args.native_mic: expected.add('fixture_native_mic')
    assert {s['name'] for s in sources} == expected
    loaded = json.loads(subprocess.check_output(['pactl', '--format=json', 'list', 'modules'], env=env))
    assert all(m['name'] in ({'module-native-protocol-unix', 'module-null-sink', 'module-remap-source'} if args.native_mic else {'module-native-protocol-unix', 'module-null-sink'}) for m in loaded)
    report['private_sources_verified'] = True
    report['no_hardware_modules_verified'] = True
    stage = 'native-ready'
    driver = spawn([args.driver, f'--port={args.port}', '--host=127.0.0.1'], 'driver.log')
    def driver_ready():
        if driver.poll() is not None:
            raise RuntimeError('Owned driver exited')
        try:
            return request('GET', '/status') is not None
        except OSError:
            return False
    wait(driver_ready)
    session = request('POST', '/session', {'capabilities': {'alwaysMatch': {'webkitgtk:browserOptions': {'binary': str(wrapper)}}}})['sessionId']
    wait(lambda: execute('return [...document.querySelectorAll("button")].some(e=>Object.keys(e).some(k=>k.startsWith("__reactProps")));'))
    stage = 'embedded-origin'
    assert execute('return location.origin === "tauri://localhost";')
    stage = 'qwen-config'
    invoke('qwen_save_settings', {'settings': {'worker_path': args.worker, 'model_dir': args.model, 'ort_library': args.ort}})
    invoke('api_save_transcript_config', {'provider': 'qwen', 'model': 'Qwen3-ASR-1.7B-INT8', 'apiKey': None})
    preferences = invoke('get_recording_preferences')
    preferences.update(auto_save=True,
        preferred_mic_device=None if args.select_via_ui else (mic_identity + ' (input)' if args.dual else None),
        preferred_system_device=None if args.select_via_ui else 'pulse-monitor:fixture_pc.monitor (output)')
    invoke('set_recording_preferences', {'preferences': preferences})
    execute('localStorage.setItem("primaryLanguage","ja");location.reload();')
    wait(lambda: execute('return document.readyState==="complete" && [...document.querySelectorAll("button")].some(e=>Object.keys(e).some(k=>k.startsWith("__reactProps")));'))
    invoke('set_language_preference', {'language': 'ja'})
    stage = 'device-enumeration'
    if args.dual:
        devices = invoke('get_audio_devices')
        assert any(d['name'] == 'pulse-monitor:fixture_pc.monitor' for d in devices)
        assert any(d['name'] == mic_identity for d in devices)
    # System-only: the deliberately invalid ALSA config can make CPAL
    # enumeration fail. Resolve the explicit native Pulse monitor at Start,
    # like recording_native_smoke.py; never enable hardware to bypass this.
    if args.select_via_ui:
        stage = 'ui-device-selection'
        labels = ['マイク', 'PC音声（取り込むスピーカー）']
        choices = [mic_identity + ' (input)' if args.dual else '__default__', 'pulse-monitor:fixture_pc.monitor (output)']
        def picker_ready(label):
            return execute('const e=[...document.querySelectorAll("select")].find(e=>e.getAttribute("aria-label")===arguments[0]);return !!e&&!e.disabled;', label)
        for label, value in zip(labels, choices):
            wait(lambda label=label: picker_ready(label))
            execute('const e=[...document.querySelectorAll("select")].find(e=>e.getAttribute("aria-label")===arguments[0]);e.value=arguments[1];e.dispatchEvent(new Event("change",{bubbles:true}));', label, value)
            key = 'preferred_mic_device' if label == labels[0] else 'preferred_system_device'
            wait(lambda key=key, value=value: invoke('get_recording_preferences')[key] == (None if value == '__default__' else value))
        selected_preferences = invoke('get_recording_preferences')
        report['ui_preserves_other_preferences'] = all(
            selected_preferences[k] == v for k, v in preferences.items()
            if k not in ['preferred_mic_device', 'preferred_system_device'])
        execute('location.reload();')
        for label, value in zip(labels, choices):
            wait(lambda label=label, value=value: picker_ready(label) and execute('return [...document.querySelectorAll("select")].find(e=>e.getAttribute("aria-label")===arguments[0]).value===arguments[1];', label, value))
        assert invoke('is_recording') is False
        report['ui_reload_preserves_selection'] = True
    stage = 'event-listeners'
    execute('window.testEvents=[];')
    for name in ['recording-stopped', 'transcript-update', 'recording-error', 'transcription-error', 'transcript-error', 'recording-save-warning', 'transcription-warning']:
        callback = execute('return window.__TAURI_INTERNALS__.transformCallback(e=>window.testEvents.push(e));')
        invoke('plugin:event|listen', {'event': name, 'target': {'kind': 'Any'}, 'handler': callback})
    stage = 'virtual-capture'
    if args.dual:
        # PCM zeroes to PRIVATE null sink only, to make virtual mic callbacks flow.
        silence = root / 'silence.f32'
        silence.write_bytes(bytes(48000 * 4 * 45))
        spawn(['paplay', *([] if args.playback_latency_ms is None else [f'--latency-msec={args.playback_latency_ms}']), '--raw', '--format=float32le', '--rate=48000', '--channels=1', '--device=fixture_mic', str(silence)], 'virtual-mic-playback.log')
    assert invoke('is_recording') is False
    click('Start recording')
    wait(lambda: invoke('is_recording') is True)
    folder = pathlib.Path(invoke('get_meeting_folder_path')).resolve()
    assert folder.is_relative_to(root / 'home/Documents/meetily-recordings')
    if args.select_via_ui:
        report['ui_locked_while_recording'] = execute('return ["マイク","PC音声（取り込むスピーカー）"].every(label=>[...document.querySelectorAll("select")].find(e=>e.getAttribute("aria-label")===label)?.disabled===true);')
        assert report['ui_locked_while_recording']
    stage = 'private-stream-metadata'
    source_ids = {s['name']: s['index'] for s in json.loads(subprocess.check_output(['pactl', '--server=' + env['PULSE_SERVER'], '--format=json', 'list', 'sources'], env=env, text=True))}
    outputs = json.loads(subprocess.check_output(['pactl', '--server=' + env['PULSE_SERVER'], '--format=json', 'list', 'source-outputs'], env=env, text=True))
    for role, name in [('mic', mic_source), ('pc', 'fixture_pc.monitor')]:
        streams = [s for s in outputs if s['source'] == source_ids.get(name)]
        report['private_' + role + '_capture_streams'] = len(streams)
        report['private_' + role + '_buffer_latency_usec'] = [s.get('buffer_latency_usec') for s in streams]
        report['private_' + role + '_source_latency_usec'] = [s.get('source_latency_usec') for s in streams]
        report['private_' + role + '_streams_uncorked'] = all(s.get('corked') is False for s in streams)
    assert report['private_pc_capture_streams'] == 1
    assert report['private_mic_capture_streams'] == (1 if args.dual else 0)
    stage = 'public-fixture-private-playback'
    report['private_playback_latency_msec'] = args.playback_latency_ms
    latency_args = [] if args.playback_latency_ms is None else [f'--latency-msec={args.playback_latency_ms}']
    if args.pc_db == -20.0:
        quiet = root / 'public-fixture-minus20db.f32'
        subprocess.run(['ffmpeg', '-v', 'error', '-i', str(fixture),
                        '-af', 'volume=0.1:precision=double', '-ac', '1', '-ar', '48000',
                        '-f', 'f32le', '-n', str(quiet)], env=env, capture_output=True, check=True)
        playback = spawn(['paplay', *latency_args, '--raw', '--format=float32le', '--rate=48000',
                          '--channels=1', '--device=fixture_pc', str(quiet)], 'playback.log')
    else:
        playback = spawn(['paplay', *latency_args, '--device=fixture_pc', str(fixture)], 'playback.log')
    # Read-only private server diagnostics while playback is active. No audio or
    # client names/properties leave this process; export numeric latency only.
    time.sleep(2)
    active_outputs = json.loads(subprocess.check_output(['pactl', '--server=' + env['PULSE_SERVER'], '--format=json', 'list', 'source-outputs'], env=env, text=True))
    for role, name in [('mic', mic_source), ('pc', 'fixture_pc.monitor')]:
        streams = [s for s in active_outputs if s['source'] == source_ids.get(name)]
        report['active_' + role + '_buffer_latency_usec'] = [s.get('buffer_latency_usec') for s in streams]
        report['active_' + role + '_source_latency_usec'] = [s.get('source_latency_usec') for s in streams]
    assert playback.wait(timeout=40) == 0
    time.sleep(.5)
    stage = 'stop-save-drain'
    click('Stop recording')
    wait(lambda: execute('return window.testEvents.some(e=>e.event==="recording-stopped");'), 240)
    stopped = execute('return window.testEvents.find(e=>e.event==="recording-stopped").payload;')
    assert invoke('is_recording') is False
    report['save_succeeded'] = stopped['save_succeeded']
    report['transcription_drained'] = stopped['transcription_drained']
    stage = 'non-content-audit'
    metadata = json.loads((folder / 'metadata.json').read_text())
    segments = json.loads((folder / 'transcripts.json').read_text())['segments']
    native_events = execute('return window.testEvents;')
    saved = {s['sequence_id']: s['text'] for s in segments}
    updates = {e['payload']['sequence_id']: e['payload']['text'] for e in native_events if e['event'] == 'transcript-update'}
    report['saved_segments'] = len(segments)
    report['recognition_nonempty'] = bool(segments) and all(s['text'].strip() for s in segments)
    report['events_match_saved_text'] = updates == saved
    report['event_error_counts'] = {name: sum(e['event'] == name for e in native_events) for name in ['recording-error', 'transcription-error', 'transcript-error', 'recording-save-warning', 'transcription-warning']}
    report['completion_committed'] = metadata.get('status') == 'completed' and bool(metadata.get('completed_at'))
    report['selected_virtual_mic_persisted'] = metadata.get('devices', {}).get('microphone') in {mic_identity, mic_identity + ' (input)'} if args.dual else metadata.get('devices', {}).get('microphone') is None
    report['selected_virtual_pc_persisted'] = metadata.get('devices', {}).get('system_audio') in {'pulse-monitor:fixture_pc.monitor', 'pulse-monitor:fixture_pc.monitor (output)'}
    # recording-stopped precedes frontend meeting registration. Do not close
    # the app or read SQLite before the normal save/navigation handler finishes.
    stage = 'frontend-meeting-registration'
    wait(lambda: execute('return location.pathname.includes("meeting-details");'), 120)
    with sqlite3.connect(f'file:{data}/meeting_minutes.sqlite?mode=ro', uri=True) as db:
        meetings = db.execute('SELECT id FROM meetings WHERE folder_path=?', (str(folder),)).fetchall()
        assert len(meetings) == 1
        rows = db.execute('SELECT transcript FROM transcripts WHERE meeting_id=?', (meetings[0][0],)).fetchall()
    report['sqlite_text_matches'] = sorted(r[0] for r in rows) == sorted(s['text'] for s in segments)
    stage = 'decoded-audio-audit'
    decoded = subprocess.run(['ffmpeg', '-v', 'warning', '-i', str(folder / 'audio.mp4'), '-f', 'f32le', '-ac', '1', '-ar', '16000', '-'], capture_output=True, env=env)
    assert decoded.returncode == 0
    samples = array.array('f'); samples.frombytes(decoded.stdout)
    if sys.byteorder != 'little': samples.byteswap()
    report['saved_audio_seconds'] = len(samples) / 16000
    report['finite_nonzero_audio'] = bool(samples) and all(math.isfinite(x) for x in samples) and max(abs(x) for x in samples) > .001
    report['decode_warnings_absent'] = not bool(decoded.stderr)
    report['rms_amplitude'] = math.sqrt(sum(x*x for x in samples)/len(samples)) if samples else 0
    report['temporary_inputs_removed'] = not list((root / 'cache/com.meetily.ai/qwen-audio').glob('*'))
    report['checkpoint_cleanup_complete'] = not (folder / '.checkpoints').exists()
    log = (root / 'native.log').read_text(errors='replace')
    marker = 'Setting IS_RECORDING to true and resetting SPEECH_DETECTED_EMITTED'
    # Capture initialization can precede the recording-state marker. This is
    # a fresh one-session profile, so verify initialization against the full log.
    windows = [int(n) for n in re.findall(r'Audio mixing window: (\d+)ms', log)]
    report['mixing_window_ms'] = windows
    full_blocks = [int(n) for n in re.findall(r'Audio transport mix: start_sample=\d+, samples=(\d+), at_s=[0-9.]+, finishing=false', log)]
    report['regular_mix_block_samples'] = sorted(set(full_blocks))
    report['window_policy_verified'] = (args.expect_window_ms is None or
        (bool(windows) and all(n == args.expect_window_ms for n in windows) and
         (not args.transport_diagnostics or (bool(full_blocks) and all(n == 48 * args.expect_window_ms for n in full_blocks)))))
    report['capture_time_sync_enabled'] = 'Linux capture-time synchronization enabled;' in log
    report['capture_time_policy_verified'] = not args.native_mic or report['capture_time_sync_enabled']
    report['pc_normalization_enabled'] = 'PC loudness normalization enabled:' in log
    report['normalization_policy_verified'] = (not args.expect_normalization or
        (report['pc_normalization_enabled'] and 'PC loudness normalization unavailable:' not in log))
    if args.transport_diagnostics:
        report.update(summarize_transport(log))
        expected_roles = ['Microphone', 'System'] if args.dual else ['System']
        report['transport_telemetry_verified'] = (report['first_mix_output_s'] is not None
            and all(report['transport_timing'][r]['callbacks'] > 0 for r in expected_roles))
    else:
        report['transport_telemetry_verified'] = True
    attrs = re.findall(r'PC Pulse capture buffer: requested_fragment_bytes=(\d+), negotiated_fragment_bytes=(\d+), negotiated_maxlength_bytes=(\d+)', log)
    report['pc_capture_buffer_attributes'] = [
        {'requested_fragment_bytes': int(requested), 'negotiated_fragment_bytes': int(fragment),
         'negotiated_maxlength_bytes': int(maxlength)} for requested, fragment, maxlength in attrs]
    latencies = report.get('active_pc_buffer_latency_usec', [])
    report['fragment_policy_verified'] = (not args.expect_fragment_100ms or (
        bool(attrs) and all(int(requested) == 19200 and int(fragment) == 19200
                            for requested, fragment, _ in attrs)
        and bool(latencies) and all(isinstance(v, (int, float)) and 0 <= v < 1200000 for v in latencies)))
    log = log.rsplit(marker, 1)[-1]
    report['stall_warnings'] = log.count('Audio input stalled for 1200ms')
    report['late_discarded_samples'] = sum(int(m[1]) for m in re.finditer(r'Discarded (\d+) late audio samples', log))
    report['queued_asr_chunks'] = [int(m[1]) for m in re.finditer(r'Input finished with (\d+) total chunks queued', log)]
    report['no_error_events'] = not any(report['event_error_counts'].values())
    report['sync_healthy'] = report['stall_warnings'] == 0 and report['late_discarded_samples'] == 0
    if args.select_via_ui:
        stage = 'native-ui-reopen'
        shutil.copyfile(root / 'native.log', root / 'native-recording.log')
        before_files = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in folder.iterdir() if p.is_file()}
        request('DELETE', f'/session/{session}'); session = None
        if driver.poll() is None:
            os.killpg(driver.pid, signal.SIGTERM)
            driver.wait(timeout=10)
        driver = spawn([args.driver, f'--port={args.port}', '--host=127.0.0.1'], 'driver-reopen.log')
        wait(driver_ready)
        session = request('POST', '/session', {'capabilities': {'alwaysMatch': {'webkitgtk:browserOptions': {'binary': str(wrapper)}}}})['sessionId']
        for label, value in zip(labels, choices):
            wait(lambda label=label, value=value: picker_ready(label) and execute('return [...document.querySelectorAll("select")].find(e=>e.getAttribute("aria-label")===arguments[0]).value===arguments[1];', label, value))
        report['ui_native_reopen_preserves_selection'] = invoke('get_recording_preferences') == selected_preferences
        report['ui_reopen_recording_false'] = invoke('is_recording') is False
        report['ui_reopen_saved_files_unchanged'] = before_files == {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in folder.iterdir() if p.is_file()}
        report['ui_verification_passed'] = all(report.get(k) for k in ['ui_preserves_other_preferences', 'ui_reload_preserves_selection', 'ui_locked_while_recording', 'ui_native_reopen_preserves_selection', 'ui_reopen_recording_false', 'ui_reopen_saved_files_unchanged'])
    else:
        report['ui_verification_passed'] = True
    report['completed'] = True
    report['passed'] = all(report[k] for k in ['ui_verification_passed', 'normalization_policy_verified', 'fragment_policy_verified', 'window_policy_verified', 'capture_time_policy_verified', 'transport_telemetry_verified', 'no_error_events', 'sync_healthy', 'save_succeeded', 'transcription_drained', 'recognition_nonempty', 'events_match_saved_text', 'completion_committed', 'selected_virtual_mic_persisted', 'selected_virtual_pc_persisted', 'sqlite_text_matches', 'finite_nonzero_audio', 'decode_warnings_absent', 'temporary_inputs_removed', 'checkpoint_cleanup_complete'])
except Exception as error:
    report['diagnostic_exception_type'] = type(error).__name__
    # Do not export exception strings, DOM, transcript text or native log lines.
    report['failed_stage'] = stage
    report['passed'] = False
finally:
    if session:
        try: request('DELETE', f'/session/{session}')
        except Exception: pass
    for process in reversed(processes):
        if process.poll() is None:
            try:
                os.killpg(process.pid, signal.SIGTERM)
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
            except ProcessLookupError: pass
    for handle in handles: handle.close()
    report['owned_helpers_exited'] = all(p.poll() is not None for p in processes)
    report['passed'] = report.get('passed', False) and report['owned_helpers_exited']
    (root / 'public-virtual-result.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps(report, indent=2))
    sys.exit(0 if report.get('passed') else 1)
