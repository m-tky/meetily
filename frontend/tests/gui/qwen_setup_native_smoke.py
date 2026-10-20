#!/usr/bin/env python3
"""Packaged Linux first-run Qwen UI, metadata-only. Never consent/download/record.
Run in the pinned nix-shell with --binary PACKAGE/bin/meetily --driver WebKitWebDriver.
Fresh HOME/XDG, private D-Bus/Xvfb, no accessible Pulse server or ALSA hardware.
"""
import argparse
import ctypes
import json
import os
import pathlib
import shutil
import signal
import subprocess
import tempfile
import time
import urllib.request

parser = argparse.ArgumentParser()
parser.add_argument('--binary', required=True)
parser.add_argument('--driver', required=True)
parser.add_argument('--runtime', required=True)
parser.add_argument('--display', default=':118')
parser.add_argument('--port', type=int, default=4468)
args = parser.parse_args()
root = pathlib.Path(tempfile.mkdtemp(prefix='qwen-onboarding-', dir=args.runtime)).resolve()
for name in ['home', 'data', 'config', 'cache', 'run']:
    (root / name).mkdir()
(root / 'run').chmod(0o700)
(root / 'alsa.conf').write_text('pcm.!default { type null }\nctl.!default { type hw card 999 }\n')
keep = ['PATH', 'LD_LIBRARY_PATH', 'FONTCONFIG_FILE', 'FONTCONFIG_PATH', 'GTK_PATH',
        'GI_TYPELIB_PATH', 'XDG_DATA_DIRS', 'GSETTINGS_SCHEMA_DIR', 'LOCALE_ARCHIVE',
        'LANG', 'LC_ALL', 'TZ']
env = {key: os.environ[key] for key in keep if key in os.environ}
env.update(HOME=str(root/'home'), XDG_DATA_HOME=str(root/'data'),
           XDG_CONFIG_HOME=str(root/'config'), XDG_CACHE_HOME=str(root/'cache'),
           XDG_RUNTIME_DIR=str(root/'run'), DISPLAY=args.display, GDK_BACKEND='x11',
           DBUS_SESSION_BUS_ADDRESS=f'unix:path={root}/bus',
           PULSE_SERVER=f'unix:{root}/no-pulse', ALSA_CONFIG_PATH=str(root/'alsa.conf'),
           TAURI_WEBVIEW_AUTOMATION='true', MEETILY_DISABLE_ANALYTICS='1')
wrapper = root / 'launch.sh'
wrapper.write_text(f'#!{shutil.which("bash")}\nexec "{pathlib.Path(args.binary).resolve()}" >"{root}/native.log" 2>&1\n')
wrapper.chmod(0o700)
processes, handles = [], []
session = None
base = f'http://127.0.0.1:{args.port}'

def spawn(command, log):
    handle = open(root/log, 'ab')
    handles.append(handle)
    process = subprocess.Popen(command, env=env, stdout=handle, stderr=handle, start_new_session=True)
    processes.append(process)

def request(method, path, body=None):
    encoded = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(base+path, encoded, method=method, headers={'Content-Type': 'application/json'})
    with urllib.request.urlopen(req, timeout=60) as response:
        value = json.load(response).get('value')
    if isinstance(value, dict) and value.get('error'):
        raise RuntimeError(str(value))
    return value

def execute(script):
    return request('POST', f'/session/{session}/execute/sync', {'script': script, 'args': []})

def invoke(command):
    return request('POST', f'/session/{session}/execute/async', {
        'script': 'const done=arguments[arguments.length-1];window.__TAURI_INTERNALS__.invoke('
                  + json.dumps(command) + ').then(done).catch(e=>done({error:String(e)}));', 'args': []})

try:
    spawn(['Xvfb', args.display, '-screen', '0', '1280x900x24', '-nolisten', 'tcp'], 'xvfb.log')
    spawn(['dbus-daemon', '--session', '--nofork', f'--address=unix:path={root}/bus'], 'dbus.log')
    spawn([args.driver, f'--port={args.port}', '--host=127.0.0.1'], 'driver.log')
    for attempt in range(100):
        try:
            request('GET', '/status')
            break
        except Exception:
            time.sleep(.2)
    session = request('POST', '/session', {'capabilities': {'alwaysMatch': {
        'webkitgtk:browserOptions': {'binary': str(wrapper)}}}})['sessionId']
    for reload in range(2):
        if reload:
            request('POST', f'/session/{session}/refresh', {})
        for attempt in range(200):
            ready = execute(r'return !!document.querySelector("section[aria-label=\"Automatic Qwen setup\"]");')
            if ready:
                break
            time.sleep(.3)
        assert ready, 'Fresh Welcome must offer Qwen setup'
        assert execute('return location.origin;') == 'tauri://localhost'
        info = invoke('qwen_setup_info')
        assert info['available'] and info['model_dir'] and info['download_bytes'] > 0
        bundled = invoke('qwen_get_settings')
        assert bundled['model_dir'] == info['model_dir']
        ctypes.CDLL(bundled['ort_library'])  # Actual dlopen, no model/audio/inference.
        assert invoke('qwen_setup_status') is None, 'Mount/reload must not start a job'
        assert not invoke('is_recording')
        assert execute(r'const s=document.querySelector("section[aria-label=\"Automatic Qwen setup\"]");return !s.querySelector("input[type=checkbox]").checked && [...s.querySelectorAll("button")].some(b=>b.textContent==="Agree and set up Qwen" && b.disabled);')
        assert execute('return document.body.innerText.includes("No local summary model is required.");')
        assert not pathlib.Path(info['model_dir']).exists(), 'No model download/initialization on mount'
    assert not list((root/'config').rglob('qwen-asr.json')), 'Opening setup must not save/select Qwen'
    print(json.dumps({'passed': True, 'fresh_welcome': True, 'reload': True,
                      'automatic_available': True, 'ort_dlopen': True, 'consent_unchecked': True,
                      'setup_job_started': False, 'model_directory_created': False,
                      'recording': False, 'profile': str(root)}))
finally:
    if session:
        try:
            request('DELETE', f'/session/{session}')
        except Exception:
            pass
    for process in reversed(processes):
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
    for handle in handles:
        handle.close()
