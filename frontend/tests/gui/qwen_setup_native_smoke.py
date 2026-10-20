#!/usr/bin/env python3
"""Packaged Linux first-run Qwen UI. Default: metadata-only, no downloads.
--prepare explicitly authorizes public model download/local initialization and selection.
Never records, plays audio, authenticates or invokes cloud inference.
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
parser.add_argument('--prepare', action='store_true', help='Consent through UI, download ASR only, initialize/select and verify native restart')
parser.add_argument('--prepare-timeout', type=int, default=3600)
parser.add_argument('--reuse-public-model', type=pathlib.Path, help='After real HTTPS download/cancel, copy only manifest files from an existing public model; initialize a fresh cache')
args = parser.parse_args()
if args.reuse_public_model and not args.prepare:
    parser.error('--reuse-public-model requires --prepare')
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
wrapper.write_text(f'#!{shutil.which("bash")}\necho $$ >"{root}/native.pid"\nexec "{pathlib.Path(args.binary).resolve()}" >"{root}/native.log" 2>&1\n')
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
    if isinstance(value, dict) and value.get('error') and 'message' in value:
        raise RuntimeError(str(value))
    return value

def execute(script):
    return request('POST', f'/session/{session}/execute/sync', {'script': script, 'args': []})

def invoke(command):
    value = request('POST', f'/session/{session}/execute/async', {
        'script': 'const done=arguments[arguments.length-1];window.__TAURI_INTERNALS__.invoke('
                  + json.dumps(command) + ').then(done).catch(e=>done({__invoke_error:String(e)}));', 'args': []})
    if isinstance(value, dict) and '__invoke_error' in value:
        raise RuntimeError(value['__invoke_error'])
    return value

def interrupted(signum, frame):
    raise SystemExit(f'Isolated setup test interrupted ({signum})')

signal.signal(signal.SIGINT, interrupted)
signal.signal(signal.SIGTERM, interrupted)

try:
    spawn(['Xvfb', args.display, '-screen', '0', '1280x900x24', '-nolisten', 'tcp'], 'xvfb.log')
    spawn(['dbus-daemon', '--session', '--nofork', f'--address=unix:path={root}/bus'], 'dbus.log')
    spawn([args.driver, f'--port={args.port}', '--host=127.0.0.1'], 'driver.log')
    for attempt in range(100):
        assert all(p.poll() is None for p in processes), 'Owned test service exited during startup'
        try:
            request('GET', '/status')
            break
        except Exception:
            time.sleep(.2)
    else:
        raise AssertionError('Owned WebDriver did not become ready')
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
    if args.prepare:
        # UI actions only: do not seed paths, model files, provider or completion flags.
        assert execute('const s=document.querySelector("#qwen-language");s.value="ja";s.dispatchEvent(new Event("change",{bubbles:true}));return s.value;') == 'ja'
        execute(r'document.querySelector("section[aria-label=\"Automatic Qwen setup\"] input[type=checkbox]").click();')
        assert execute('const b=[...document.querySelectorAll("button")].find(b=>b.textContent==="Agree and set up Qwen");if(!b||b.disabled)return false;b.click();return true;')
        if args.reuse_public_model:
            # Exercise real HTTPS streaming and UI cancellation before the cache-assisted retry.
            deadline = time.monotonic() + 180
            while time.monotonic() < deadline:
                status = invoke('qwen_setup_status')
                assert not status or status['phase'] not in ['failed', 'ready'], status and status.get('error')
                if status and status['phase'] == 'downloading' and status['downloaded'] > 1000000:
                    break
                time.sleep(.5)
            else:
                raise AssertionError('No real HTTPS progress before cancellation')
            assert execute('const b=[...document.querySelectorAll("button")].find(b=>b.textContent==="Cancel setup");if(!b)return false;b.click();return true;')
            for attempt in range(120):
                if invoke('qwen_setup_status')['phase'] == 'cancelled':
                    break
                time.sleep(.5)
            else:
                raise AssertionError('Cancellation did not finish')
            assert not invoke('get_onboarding_status')['completed']
            assert not list((root/'config').rglob('qwen-asr.json'))
            assert not list(pathlib.Path(info['model_dir']).glob('.download-*'))
            print(json.dumps({'https_progress_then_ui_cancel': True}), flush=True)
            manifest = json.loads((pathlib.Path(__file__).resolve().parents[3]/'nix/qwen-model.json').read_text())
            model = pathlib.Path(info['model_dir'])
            model.mkdir(parents=True, exist_ok=True)
            for item in manifest['files']:
                shutil.copy2(args.reuse_public_model/item['file'], model/item['file'])
            assert not (model/'qwen-asr-int8.sidecar').exists(), 'Initializer must build a fresh cache'
            time.sleep(2)
            assert execute(r'const c=document.querySelector("section[aria-label=\"Automatic Qwen setup\"] input[type=checkbox]");if(c.disabled)return false;if(!c.checked)c.click();return true;')
            assert execute('const b=[...document.querySelectorAll("button")].find(b=>b.textContent==="Agree and set up Qwen");if(!b||b.disabled)return false;b.click();return true;')
        deadline = time.monotonic() + args.prepare_timeout
        previous_phase = None
        while time.monotonic() < deadline:
            status = invoke('qwen_setup_status')
            phase = status['phase'] if status else 'pending'
            if phase != previous_phase:
                print(json.dumps({'setup_phase': phase}), flush=True)
                previous_phase = phase
            assert not invoke('is_recording'), 'Setup must not start recording'
            if phase in ['failed', 'cancelled']:
                raise AssertionError('Public-model setup failed: ' + str(status.get('error')))
            onboarding = invoke('get_onboarding_status')
            if onboarding and onboarding['completed']:
                break
            time.sleep(2)
        else:
            raise AssertionError('Qwen-only onboarding timed out')

        def verify_selected():
            saved = invoke('get_onboarding_status')
            assert saved['completed']
            assert saved['model_status']['summary'] == 'not_downloaded'
            assert saved['model_status']['parakeet'] == 'not_downloaded'
            config = invoke('api_get_transcript_config')
            assert config['provider'] == 'qwen' and config['model'] == 'Qwen3-ASR-1.7B-INT8'
            settings = invoke('qwen_get_settings')
            assert settings['model_dir'] == info['model_dir']
            assert not invoke('is_recording')
            # Check the real prepared file set without reading weights into tool output.
            model = pathlib.Path(settings['model_dir'])
            assert (model/'qwen-asr-int8.sidecar').stat().st_size == 1724760064
            assert not list(root.rglob('*qwen3.5*')) and not list(root.rglob('*Qwen3.5*'))
            allowed_large_files = {'model-00001-of-00002.safetensors', 'model-00002-of-00002.safetensors', 'qwen-asr-int8.sidecar'}
            for file in (root/'data').rglob('*'):
                if file.is_file() and file.stat().st_size > 100 * 1024 ** 2:
                    assert file.parent == model and file.name in allowed_large_files, 'Unexpected non-ASR model download'
            return saved, config

        saved, config = verify_selected()
        print(json.dumps({'asr_only_onboarding_completed': True}), flush=True)
        old_pid = int((root/'native.pid').read_text())
        request('DELETE', f'/session/{session}')
        session = None
        for attempt in range(100):
            try:
                os.kill(old_pid, 0)
            except ProcessLookupError:
                break
            time.sleep(.2)
        else:
            raise AssertionError('Old native process did not exit before restart')
        session = request('POST', '/session', {'capabilities': {'alwaysMatch': {
            'webkitgtk:browserOptions': {'binary': str(wrapper)}}}})['sessionId']
        for attempt in range(200):
            try:
                if execute('return !!window.__TAURI_INTERNALS__;') and invoke('get_onboarding_status')['completed']:
                    break
            except Exception:
                pass
            time.sleep(.3)
        else:
            raise AssertionError('Native restart did not restore onboarding')
        verify_selected()
        print(json.dumps({'native_restart_settings_restored': True}), flush=True)
        for attempt in range(200):
            if execute('return !!document.querySelector("button[aria-label=\\"Start recording\\"]") && !document.body.innerText.includes("Welcome to Meetily");'):
                break
            time.sleep(.3)
        else:
            raise AssertionError('Native restart did not reach recording screen without summary setup')
        assert invoke('qwen_setup_status') is None, 'Native restart must not start another setup'
        print(json.dumps({'passed': True, 'asr_only_prepared_selected': True,
                          'native_restart': True, 'cache_assisted_retry': bool(args.reuse_public_model),
                          'fresh_cache_initialized': True, 'summary_downloaded': False,
                          'parakeet_downloaded': False, 'recording': False,
                          'profile': str(root)}), flush=True)
    else:
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
