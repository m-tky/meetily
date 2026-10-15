#!/usr/bin/env python3
"""Visible, isolated Linux preview; user controls recording, synthetic audio only.
Use prebuilt native/fixture-bootstrap binaries; dev builds need Next on 3118.
Custom-protocol builds use their embedded origin, never navigate to the dev server.
No host audio modules, microphone, credentials, or cloud inference are configured.
WebDriver sets the isolated profile up; it never clicks Start/Stop or edits minutes.
"""
import argparse
import base64
import fcntl
import json
import os
import pathlib
import shlex
import shutil
import signal
import subprocess
import tempfile
import time
import urllib.request

parser = argparse.ArgumentParser(description=__doc__)
for name in ['binary', 'bootstrap', 'driver', 'runtime', 'espeak', 'worker', 'model', 'ort']:
    parser.add_argument('--'+name, required=True)
parser.add_argument('--display', default=os.environ.get('DISPLAY', ':0'))
parser.add_argument('--port', type=int, default=4447)
parser.add_argument('--profile', help='Resume only a previously created synthetic preview profile')
args = parser.parse_args()
for name in ['binary', 'bootstrap', 'driver', 'espeak', 'worker', 'model', 'ort']:
    setattr(args, name, str(pathlib.Path(getattr(args, name)).resolve(strict=True)))
runtime = pathlib.Path(args.runtime).resolve(strict=True)
fresh = not args.profile
root = pathlib.Path(tempfile.mkdtemp(prefix='preview-gui-', dir=runtime)) if fresh else pathlib.Path(args.profile).resolve(strict=True)
assert root.is_relative_to(runtime), 'Preview must stay inside the supplied runtime'
if not fresh:
    assert (root/'.meetily-synthetic-preview').read_text() == 'synthetic-local-preview\n'
else:
    root.chmod(0o700)
    (root/'.meetily-synthetic-preview').write_text('synthetic-local-preview\n')
    (root/'.synthetic-gui-test').write_text('synthetic-only\n')
if not fresh and (root/'preview-state.json').exists():
    previous = json.loads((root/'preview-state.json').read_text())
    if previous.get('ready') and previous.get('pid'):
        try: os.kill(previous['pid'], 0)
        except ProcessLookupError: pass
        else: raise RuntimeError('This preview is still running; close it before resuming')
lease = open(root/'.preview.lock', 'a')
fcntl.flock(lease, fcntl.LOCK_EX | fcntl.LOCK_NB)
for name in ['home/Documents', 'home/Downloads', 'data', 'config', 'cache', 'run']:
    (root/name).mkdir(parents=True, exist_ok=True)
(root/'run').chmod(0o700)
(root/'config/user-dirs.dirs').write_text(f'XDG_DOCUMENTS_DIR="{root}/home/Documents"\nXDG_DOWNLOAD_DIR="{root}/home/Downloads"\n')
(root/'no-hardware-alsa.conf').write_text('pcm.!default { type hw card 999 }\nctl.!default { type hw card 999 }\n')
keep = ['PATH', 'LD_LIBRARY_PATH', 'FONTCONFIG_FILE', 'FONTCONFIG_PATH', 'GTK_PATH', 'GI_TYPELIB_PATH', 'XDG_DATA_DIRS', 'GSETTINGS_SCHEMA_DIR', 'LOCALE_ARCHIVE', 'LANG', 'LC_ALL', 'TZ', 'XAUTHORITY']
env = {key: os.environ[key] for key in keep if key in os.environ}
env.update(HOME=str(root/'home'), XDG_DATA_HOME=str(root/'data'), XDG_CONFIG_HOME=str(root/'config'), XDG_CACHE_HOME=str(root/'cache'), XDG_RUNTIME_DIR=str(root/'run'), DISPLAY=args.display, GDK_BACKEND='x11', DBUS_SESSION_BUS_ADDRESS=f'unix:path={root}/bus', PULSE_SERVER=f'unix:{root}/pulse', ALSA_CONFIG_PATH=str(root/'no-hardware-alsa.conf'), TAURI_WEBVIEW_AUTOMATION='true', MEETILY_DISABLE_ANALYTICS='1', NEXT_TELEMETRY_DISABLED='1')
data = root/'data/com.meetily.ai'
if fresh:
    subprocess.run([args.bootstrap, str(root)], env=env, check=True, capture_output=True)
    (data/'onboarding-status.json').write_text(json.dumps({'status': {'version':'1.0', 'completed':True, 'current_step':5, 'model_status':{'parakeet':'not_downloaded', 'summary':'not_downloaded'}, 'last_updated':'2026-01-01T00:00:00Z'}}))
    folder = root/'home/Documents/meetily-recordings/Preview-Synthetic-Minutes'
    folder.mkdir(parents=True)
    segment = dict(id='seg_preview_1', text='人工データ：提案を保留し、手動で確認する。', audio_start_time=0, audio_end_time=2, duration=2, display_time='00:00', confidence=-1, sequence_id=1)
    item = dict(id='preview-item-1', kind='question', text='人工データ：提案は未決定。', status='active', sources=['seg_preview_1'], owner=None, due=None)
    metadata = dict(version='1.0', meeting_id=None, meeting_name='操作確認用・人工議事録（AI推論なし）', created_at='2026-01-01T00:00:00Z', completed_at='2026-01-01T00:00:02Z', duration_seconds=2, devices={'microphone':None, 'system_audio':None}, audio_file='', transcript_file='transcripts.json', sample_rate=48000, status='completed')
    minutes = dict(schema_version=1, revision=1, fingerprint='synthetic-preview', model='synthetic-no-inference', updated_at='2026-01-01T00:00:02Z', items=[item], user_edits=[])
    for name, value in [('metadata.json', metadata), ('transcripts.json', dict(version='1.0', segments=[segment], total_segments=1)), ('live-minutes.json', minutes)]:
        (folder/name).write_text(json.dumps(value, ensure_ascii=False, indent=2))
speech = root/'synthetic.wav'
subprocess.run([args.espeak, '-v', 'en', '-s', '140', '-w', str(speech), 'This is synthetic audio for the recording test. The proposal is pending. Please verify the saved recording manually.'], env=env, check=True, capture_output=True)
wrapper = root/'launch-app.sh'
wrapper.write_text('#!'+shutil.which('bash')+'\nexec '+shlex.quote(args.binary)+' >>'+shlex.quote(str(root/'native.log'))+' 2>&1\n')
wrapper.chmod(0o700)
# Also permits replay without exporting any host audio configuration.
play = root/'play-synthetic.sh'
play.write_text('#!'+shutil.which('bash')+'\nexec env PULSE_SERVER='+shlex.quote(env['PULSE_SERVER'])+' '+shlex.quote(shutil.which('paplay'))+' --device=meetily_fixture '+shlex.quote(str(speech))+'\n')
play.chmod(0o700)
processes, handles = [], []
session = None
ready = False
exit_requested = False
base = f'http://127.0.0.1:{args.port}'

def request(method, path, body=None):
    payload = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(base+path, payload, method=method, headers={'Content-Type':'application/json'})
    with urllib.request.urlopen(req, timeout=90) as response:
        value = json.load(response).get('value')
    if isinstance(value, dict) and value.get('error'):
        raise RuntimeError(json.dumps(value))
    return value

def execute(script):
    return request('POST', f'/session/{session}/execute/sync', {'script':script, 'args':[]})

def invoke(name, values=None):
    value = request('POST', f'/session/{session}/execute/async', {'script':'const done=arguments[arguments.length-1];window.__TAURI_INTERNALS__.invoke(arguments[0],arguments[1]).then(done).catch(e=>done({error:String(e)}));', 'args':[name, values or {}]})
    if isinstance(value, dict) and 'error' in value:
        raise RuntimeError(json.dumps(value))
    return value

def wait(predicate, timeout=90):
    until = time.monotonic()+timeout
    while time.monotonic()<until:
        if predicate(): return
        time.sleep(.25)
    raise RuntimeError('Preview readiness timeout')

def spawn(command, name):
    log = open(root/name, 'ab')
    handles.append(log)
    process = subprocess.Popen(command, env=env, stdout=log, stderr=log, start_new_session=True)
    processes.append(process)
    return process

def shutdown(signum, frame):
    global exit_requested
    exit_requested = True

for signum in [signal.SIGTERM, signal.SIGINT]:
    signal.signal(signum, shutdown)
try:
    print('PRIVATE_PREVIEW='+str(root), flush=True)
    spawn(['dbus-daemon', '--session', '--nofork', f'--address=unix:path={root}/bus'], 'dbus.log')
    spawn(['pulseaudio', '--daemonize=no', '--exit-idle-time=-1', '--use-pid-file=no', '--disallow-exit', '-n', '--load='+f'module-native-protocol-unix socket={root}/pulse auth-anonymous=1', '--load=module-null-sink sink_name=meetily_fixture rate=48000 channels=1'], 'pulse.log')
    wait(lambda: (root/'pulse').is_socket())
    sources = json.loads(subprocess.check_output(['pactl', '--format=json', 'list', 'sources'], env=env))
    assert len(sources)==1 and sources[0]['name']=='meetily_fixture.monitor'
    # Do not attach to or stop a previously running WebDriver on this port.
    import socket
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', args.port))
    driver = spawn([args.driver, f'--port={args.port}', '--host=127.0.0.1'], 'driver.log')
    def driver_ready():
        if driver.poll() is not None: raise RuntimeError('Owned WebDriver exited')
        try: return request('GET', '/status') is not None
        except OSError: return False
    wait(driver_ready)
    session = request('POST', '/session', {'capabilities':{'alwaysMatch':{'webkitgtk:browserOptions':{'binary':str(wrapper)}}}})['sessionId']
    wait(lambda: execute('return [...document.querySelectorAll("button")].some(e=>Object.keys(e).some(k=>k.startsWith("__reactProps")));'))
    invoke('qwen_save_settings', {'settings':{'worker_path':args.worker, 'model_dir':args.model, 'ort_library':args.ort}})
    invoke('api_save_transcript_config', {'provider':'qwen', 'model':'Qwen3-ASR-1.7B-INT8', 'apiKey':None})
    preferences = invoke('get_recording_preferences')
    preferences.update(auto_save=True, preferred_mic_device=None, preferred_system_device='pulse-monitor:meetily_fixture.monitor (output)')
    invoke('set_recording_preferences', {'preferences':preferences})
    invoke('set_language_preference', {'language':'en'})
    execute('localStorage.setItem("primaryLanguage","en");location.reload();')
    wait(lambda: execute('return document.readyState=="complete" && [...document.querySelectorAll("button")].some(e=>Object.keys(e).some(k=>k.startsWith("__reactProps")));'))
    # Import only our newly generated artificial saved minutes, never overwrite edits on resume.
    if fresh:
        scan = invoke('api_scan_recording_recovery', {'cursor':None})
        candidate = next(c for c in scan['candidates'] if c['folder_name']=='Preview-Synthetic-Minutes')
        restored = invoke('api_restore_recording', {'candidateId':candidate['id'], 'fingerprint':candidate['fingerprint']})
        origin = execute('return location.origin;')
        request('POST', f'/session/{session}/url', {'url':origin+'/meeting-details?id='+restored['meeting_id']})
        wait(lambda: execute('return document.body.innerText.includes("人工データ：提案を保留し、手動で確認する。");'))
        execute('const s=[...document.querySelectorAll("summary")].find(e=>e.textContent.includes("保存済み継続議事録"));if(s)s.parentElement.open=true;')
        wait(lambda: execute('return document.body.innerText.includes("人工データ：提案は未決定。");'))
    (root/'preview-state.json').write_text(json.dumps({'ready':True, 'profile':str(root), 'display':args.display, 'port':args.port, 'synthetic_only':True, 'automatic_playback':'once after each user Start, never starts recording', 'pid':os.getpid()}, indent=2))
    (root/'ready.png').write_bytes(base64.b64decode(request('GET', f'/session/{session}/screenshot')))
    ready = True
    print('READY: user controls Start/Stop; synthetic sample plays once per Start.', flush=True)
    was_recording, playback, scheduled = False, None, None
    while not exit_requested:
        try: recording = invoke('is_recording')
        except Exception:
            # User navigation/reload briefly removes the IPC bridge; don't close their app.
            try: windows = request('GET', f'/session/{session}/window/handles')
            except Exception: windows = []
            if windows:
                time.sleep(.5)
                continue
            print('Native preview closed or disconnected; helper shutting down.', flush=True)
            break
        if recording and not was_recording:
            scheduled = time.monotonic()+2
        if not recording:
            scheduled = None
            if playback is not None and playback.poll() is None:
                playback.terminate()
                playback.wait(timeout=5)
        if recording and scheduled is not None and time.monotonic()>=scheduled:
            playback = spawn(['paplay', '--device=meetily_fixture', str(speech)], 'playback.log')
            scheduled = None
            print('Synthetic playback started after user Start.', flush=True)
        was_recording = recording
        time.sleep(.5)
finally:
    if session:
        try: request('DELETE', f'/session/{session}')
        except Exception: pass
    for process in reversed(processes):
        if process.poll() is None:
            try: os.killpg(process.pid, signal.SIGTERM)
            except ProcessLookupError: continue
            try: process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
    for handle in handles: handle.close()
    (root/'preview-state.json').write_text(json.dumps({'ready':False, 'was_ready':ready, 'profile':str(root), 'synthetic_only':True}, indent=2))
