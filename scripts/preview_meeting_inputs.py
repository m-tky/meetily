#!/usr/bin/env python3
"""User-operated dual-input check, explicitly approved mic + PC playback.
ALSA microphone is bound to EchoCancel_Mic; native monitor is EchoCancel_Speaker.monitor.
Private profile/DBus, local Qwen JA. No automatic Start/Stop or playback/cloud/login.
This does not prove accuracy, AEC effectiveness or absence of duplicated speech.
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
for name in ['binary', 'bootstrap', 'driver', 'runtime', 'worker', 'model', 'ort', 'alsa-plugin-dir', 'pulse-server', 'mic-source']:
    parser.add_argument('--'+name, required=True)
parser.add_argument('--display', default=os.environ.get('DISPLAY', ':0'))
parser.add_argument('--port', type=int, default=4453)
parser.add_argument('--window-title', default='Meetily — 会議入力確認 / マイク＋PC音声 / 日本語')
parser.add_argument('--no-ready-screenshot', action='store_true', help='Do not capture the UI when resuming a profile that may contain real text')
parser.add_argument('--preserve-settings', action='store_true', help='Resume without rewriting provider, language or selected input preferences')
parser.add_argument('--language',required=True,choices=['ja','en','th'])
parser.add_argument('--system-monitor',required=True,choices=['EchoCancel_Speaker.monitor'])
parser.add_argument('--profile', help='Resume only a previously created dual-input preview profile')
parser.add_argument('--show-missing-devices', action='store_true', help='Show the new input UI with saved missing choices and Start disabled; never fallback/capture')
args = parser.parse_args()
for name in ['binary', 'bootstrap', 'driver', 'worker', 'model', 'ort', 'alsa_plugin_dir']:
    setattr(args, name, str(pathlib.Path(getattr(args, name)).resolve(strict=True)))
runtime = pathlib.Path(args.runtime).resolve(strict=True)
fresh = not args.profile
assert not (fresh and args.preserve_settings), '--preserve-settings requires --profile'
root = pathlib.Path(tempfile.mkdtemp(prefix='dual-preview-', dir=runtime)) if fresh else pathlib.Path(args.profile).resolve(strict=True)
assert root.is_relative_to(runtime), 'Preview must stay inside the supplied runtime'
if not fresh:
    assert (root/'.meetily-dual-preview').read_text() == 'real-dual-local-preview\n'
else:
    root.chmod(0o700)
    (root/'.meetily-dual-preview').write_text('real-dual-local-preview\n')

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
import re
assert re.fullmatch(r'[A-Za-z0-9_.-]+',args.mic_source), 'Invalid source name'
assert args.mic_source=='EchoCancel_Mic', 'This bounded check permits only the user-selected EchoCancel_Mic'
assert args.pulse_server.startswith('unix:') and pathlib.Path(args.pulse_server[5:]).is_socket()
plugin=pathlib.Path(args.alsa_plugin_dir)/'libasound_module_pcm_pulse.so'
assert plugin.is_file(), 'ALSA Pulse plugin missing'
binding='type pulse server '+json.dumps(args.pulse_server)+' device '+json.dumps(args.mic_source)
(root/'microphone-alsa.conf').write_text(
    'defaults.namehint.showall off\ndefaults.namehint.basic on\ndefaults.namehint.extended off\n'
    'defaults.ctl.card 0\nctl.hw { @args [ CARD ] @args.CARD { type string default 0 } type hw card $CARD }\n'
    'pcm_type.pulse { lib '+json.dumps(str(plugin))+' }\n'
    'pcm.!default { '+binding+' hint { show on description "EchoCancel_Mic only" ioid "Input" } }\n'
    'pcm.meetily_mic { '+binding+' hint { show on description "EchoCancel_Mic only" ioid "Input" } }\n'
    'ctl.!default { type hw card 999 }\n')
keep = ['PATH', 'LD_LIBRARY_PATH', 'FONTCONFIG_FILE', 'FONTCONFIG_PATH', 'GTK_PATH', 'GI_TYPELIB_PATH', 'XDG_DATA_DIRS', 'GSETTINGS_SCHEMA_DIR', 'LOCALE_ARCHIVE', 'LANG', 'LC_ALL', 'TZ', 'XAUTHORITY']
env = {key: os.environ[key] for key in keep if key in os.environ}
env.update(HOME=str(root/'home'), XDG_DATA_HOME=str(root/'data'), XDG_CONFIG_HOME=str(root/'config'), XDG_CACHE_HOME=str(root/'cache'), XDG_RUNTIME_DIR=str(root/'run'), DISPLAY=args.display, GDK_BACKEND='x11', DBUS_SESSION_BUS_ADDRESS=f'unix:path={root}/bus', PULSE_SERVER=args.pulse_server, ALSA_CONFIG_PATH=str(root/'microphone-alsa.conf'), TAURI_WEBVIEW_AUTOMATION='true', MEETILY_DISABLE_ANALYTICS='1', NEXT_TELEMETRY_DISABLED='1')
data = root/'data/com.meetily.ai'
if fresh:
    (root/'.synthetic-gui-test').write_text('synthetic-only\n')  # Schema bootstrap only; no microphone use.
    try:
        subprocess.run([args.bootstrap, str(root)], env=env, check=True, capture_output=True)
    finally:
        (root/'.synthetic-gui-test').unlink()
    (data/'onboarding-status.json').write_text(json.dumps({'status': {'version':'1.0', 'completed':True, 'current_step':5, 'model_status':{'parakeet':'not_downloaded', 'summary':'not_downloaded'}, 'last_updated':'2026-01-01T00:00:00Z'}}))
wrapper = root/'launch-app.sh'
wrapper.write_text('#!'+shutil.which('bash')+'\nexec '+shlex.quote(args.binary)+' >>'+shlex.quote(str(root/'native.log'))+' 2>&1\n')
wrapper.chmod(0o700)
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
    # Do not attach to or stop a previously running WebDriver on this port.
    import socket
    with socket.socket() as probe:
        # Allow TIME_WAIT from our closed session, still reject a live listener.
        probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        probe.bind(('127.0.0.1', args.port))
    driver = spawn([args.driver, f'--port={args.port}', '--host=127.0.0.1'], 'driver.log')
    def driver_ready():
        if driver.poll() is not None: raise RuntimeError('Owned WebDriver exited')
        try: return request('GET', '/status') is not None
        except OSError: return False
    wait(driver_ready)
    session = request('POST', '/session', {'capabilities':{'alwaysMatch':{'webkitgtk:browserOptions':{'binary':str(wrapper)}}}})['sessionId']
    wait(lambda: execute('return [...document.querySelectorAll("button")].some(e=>Object.keys(e).some(k=>k.startsWith("__reactProps")));'))
    if not args.preserve_settings:
        invoke('qwen_save_settings', {'settings':{'worker_path':args.worker, 'model_dir':args.model, 'ort_library':args.ort}})
        invoke('api_save_transcript_config', {'provider':'qwen', 'model':'Qwen3-ASR-1.7B-INT8', 'apiKey':None})
        preferences = invoke('get_recording_preferences')
        preferences.update(auto_save=True, preferred_mic_device='meetily_mic (input)', preferred_system_device='pulse-monitor:'+args.system_monitor+' (output)')
        invoke('set_recording_preferences', {'preferences':preferences})
        invoke('set_language_preference', {'language':args.language})
        execute('localStorage.setItem("primaryLanguage",'+json.dumps(args.language)+');location.reload();')
    wait(lambda: execute('return document.readyState=="complete" && [...document.querySelectorAll("button")].some(e=>Object.keys(e).some(k=>k.startsWith("__reactProps")));'))
    invoke('plugin:window|set_title',{'label':'main','value':args.window_title})
    devices=invoke('get_audio_devices')
    mic_available = any(d['name']=='meetily_mic' for d in devices)
    system_available = any(d.get('device_type')=='Output' and d['name']=='pulse-monitor:'+args.system_monitor for d in devices)
    if not args.show_missing_devices:
        assert devices and mic_available, 'Explicit microphone not enumerated'
        assert system_available, 'Selected PC monitor not enumerated'
    elif not (mic_available and system_available):
        # Only open an idle UI, preserving saved identities. Never bypass its
        # missing-device Start guard to make the preview appear ready.
        wait(lambda: execute('return [...document.querySelectorAll("select option")].some(e=>e.textContent.includes("見つかりません")) && [...document.querySelectorAll("button")].some(e=>e.getAttribute("aria-label")==="Start recording" && e.disabled);'))
    assert invoke('is_recording') is False
    assert execute('return location.origin === "tauri://localhost"'), 'Expected embedded candidate UI'
    (root/'input-routing.json').write_text(json.dumps({'devices':devices,'microphone_source':args.mic_source,'selected_system_monitor':args.system_monitor,
        'host_server':args.pulse_server},indent=2))
    (root/'preview-state.json').write_text(json.dumps({'ready':True, 'profile':str(root), 'display':args.display, 'port':args.port, 'microphone':args.mic_source, 'language':args.language, 'system_audio':args.system_monitor, 'automatic_recording':False, 'pid':os.getpid(), 'binary':args.binary, 'embedded_origin_verified':True, 'webdriver_session_id':session, 'selected_mic_enumerated':mic_available, 'selected_system_monitor_enumerated':system_available, 'missing_device_preview':args.show_missing_devices and not (mic_available and system_available)}, indent=2))
    if not args.no_ready_screenshot:
        (root/'ready.png').write_bytes(base64.b64decode(request('GET', f'/session/{session}/screenshot')))
    ready = True
    if args.show_missing_devices and not (mic_available and system_available):
        print('READY: input UI / missing saved devices / Start disabled. No fallback or recording started.', flush=True)
    else:
        print('READY: mic + PC monitor / Japanese; user controls Start/Stop. No recording started.', flush=True)
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
    (root/'preview-state.json').write_text(json.dumps({'ready':False, 'was_ready':ready, 'profile':str(root), 'microphone':args.mic_source,'system_audio':args.system_monitor}, indent=2))
