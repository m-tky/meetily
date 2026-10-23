#!/usr/bin/env python3
"""Qwen-only native/UI import and retranscription wiring; synthetic file + fake ASR.
No weights, recording, playback, hardware, cloud, auth or real profile access.
"""
import argparse
import hashlib
import json
import os
import pathlib
import shutil
import signal
import socket
import sqlite3
import subprocess
import sys
import tempfile
import textwrap
import time
import urllib.error
import urllib.request

parser = argparse.ArgumentParser(description=__doc__)
for name in ['binary', 'driver', 'espeak']:
    parser.add_argument('--' + name, required=True)
parser.add_argument('--runtime', default='/tmp')
parser.add_argument('--display', default=':123')
parser.add_argument('--port', type=int, default=4490)
args = parser.parse_args()
# Never attach to an existing display/driver.
assert not pathlib.Path('/tmp/.X' + args.display.lstrip(':') + '-lock').exists()
with socket.socket() as check:
    check.bind(('127.0.0.1', args.port))
root = pathlib.Path(tempfile.mkdtemp(prefix='qwen-batch-gui-', dir=args.runtime)).resolve()
for name in ['home/Documents', 'home/Downloads', 'config', 'data', 'cache', 'run']:
    (root / name).mkdir(parents=True)
(root / 'run').chmod(0o700)
(root / '.synthetic-batch-test').write_text('synthetic-only\n')
(root / 'config/user-dirs.dirs').write_text(f'XDG_DOCUMENTS_DIR="{root}/home/Documents"\nXDG_DOWNLOAD_DIR="{root}/home/Downloads"\n')
(root / 'no-hardware-alsa.conf').write_text('pcm.!default { type hw card 999 }\nctl.!default { type hw card 999 }\n')
keep = ['PATH', 'LD_LIBRARY_PATH', 'FONTCONFIG_FILE', 'FONTCONFIG_PATH', 'GTK_PATH',
        'GI_TYPELIB_PATH', 'XDG_DATA_DIRS', 'GSETTINGS_SCHEMA_DIR', 'LOCALE_ARCHIVE', 'LANG', 'LC_ALL', 'TZ']
env = {key: os.environ[key] for key in keep if key in os.environ}
env.update(HOME=str(root / 'home'), XDG_CONFIG_HOME=str(root / 'config'), XDG_DATA_HOME=str(root / 'data'),
           XDG_CACHE_HOME=str(root / 'cache'), XDG_RUNTIME_DIR=str(root / 'run'), DISPLAY=args.display,
           GDK_BACKEND='x11', DBUS_SESSION_BUS_ADDRESS=f'unix:path={root}/bus',
           PULSE_SERVER=f'unix:{root}/unavailable-pulse', ALSA_CONFIG_PATH=str(root / 'no-hardware-alsa.conf'),
           TAURI_WEBVIEW_AUTOMATION='true', MEETILY_DISABLE_ANALYTICS='1', NEXT_TELEMETRY_DISABLED='1')
large, small = 'Qwen3-ASR-1.7B-INT8', 'Qwen3-ASR-0.6B-INT8'
text = 'SYNTHETIC BATCH FIXTURE. Not speech-recognition quality evidence.'
worker = root / 'fixture-worker'
worker.write_text('#!' + sys.executable + '\n' + textwrap.dedent('''
    import json, math, os, pathlib, struct, sys, time
    root = pathlib.Path(__file__).parent
    assert (root/'.synthetic-batch-test').read_text() == 'synthetic-only\\n'
    directory = pathlib.Path(sys.argv[1])
    hidden = json.loads((directory/'config.json').read_text())['thinker_config']['text_config']['hidden_size']
    model = 'Qwen3-ASR-0.6B' if hidden == 1024 else 'Qwen3-ASR-1.7B'
    assert len(sys.argv) == 3 and sys.argv[2] == '6'  # Legacy worker CLI remains compatible.
    print(json.dumps(dict(event='ready',protocol=1,threads=6,model=model,
        precision='decoder-INT8-encoder-prefill-F32',recognition_context='full_input',vad_only=False,
        max_audio_seconds=30,vad_frame_samples=512,audio_formats=['float32'])),flush=True)
    for raw in sys.stdin:
        req=json.loads(raw); path=pathlib.Path(req['wav']).resolve()
        assert path.is_relative_to(root/'cache')
        pcm=path.read_bytes(); offset=pcm.index(b'data')+8
        assert pcm[:4]==b'RIFF' and pcm[20:22]==b'\\x03\\x00'
        assert struct.unpack('<I',pcm[24:28])[0] == 16000
        samples=struct.unpack('<'+'f'*((len(pcm)-offset)//4),pcm[offset:])
        assert samples and all(math.isfinite(x) for x in samples) and req['language']=='English'
        with (root/'requests.jsonl').open('a') as output:
            output.write(json.dumps(dict(model=model,pid=os.getpid(),samples=len(samples),language=req['language']))+'\\n')
        while (root/'hold').exists(): time.sleep(.02)
        if (root/'fail').exists():
            print(json.dumps(dict(id=req['id'],confirmed=False,status='error',text='',asr_calls=0)),flush=True)
        else:
            print(json.dumps(dict(id=req['id'],confirmed=False,status='recognized',text=TEXT,
                asr_calls=1,recognition_context='full_input')),flush=True)
''').replace('text=TEXT', 'text=' + repr(text)))
compile(worker.read_text(), str(worker), 'exec')
worker.chmod(0o700)
ort = root / 'fixture-ort'
ort.write_text('Fixture only, not a shared library.\n')
settings = {}
for model, hidden in [(large, 2048), (small, 1024)]:
    directory = root / model
    directory.mkdir()
    (directory / 'config.json').write_text(json.dumps({'model_type': 'qwen3_asr', 'thinker_config': {'text_config': {'hidden_size': hidden, 'num_hidden_layers': 28}}}))
    names = ['model.safetensors'] if model == small else ['model-00001-of-00002.safetensors', 'model-00002-of-00002.safetensors']
    for name in names + ['qwen-asr-int8.sidecar', 'vocab.json', 'merges.txt']:
        (directory / name).write_text('Fixture only, not weights.\n')
    settings[model] = {'worker_path': str(worker), 'model_dir': str(directory), 'ort_library': str(ort)}
audio = root / 'synthetic-batch.wav'
subprocess.run([args.espeak, '-v', 'en', '-s', '140', '-w', str(audio),
                'This is artificial speech for a local file import test. The proposal is pending. Please review the saved result manually.'],
               env=env, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
wrapper = root / 'launch-app.sh'
wrapper.write_text('#!' + shutil.which('bash') + '\nexec "' + str(pathlib.Path(args.binary).resolve()) + '" >>"' + str(root / 'native.log') + '" 2>&1\n')
wrapper.chmod(0o700)
processes, handles, events, meetings = [], [], [], []
session = None
passed = False
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
    try:
        with urllib.request.urlopen(req, timeout=120) as response:
            value = json.load(response).get('value')
    except urllib.error.HTTPError as error:
        detail = json.loads(error.read()).get('value', {})
        raise RuntimeError(str(detail.get('error')) + ': ' + str(detail.get('message'))) from error
    if isinstance(value, dict) and isinstance(value.get('error'), str) and 'message' in value:
        raise RuntimeError(value['error'] + ': ' + value['message'])
    return value


def execute(script, *values):
    return request('POST', f'/session/{session}/execute/sync', {'script': script, 'args': list(values)})


def invoke_result(name, values=None):
    return request('POST', f'/session/{session}/execute/async', {
        'script': 'const done=arguments[arguments.length-1];window.__TAURI_INTERNALS__.invoke(arguments[0],arguments[1]).then(v=>done({ok:true,value:v})).catch(e=>done({ok:false,invokeError:String(e)}));',
        'args': [name, values or {}]})


def invoke(name, values=None):
    result = invoke_result(name, values)
    assert result['ok'], f'{name}: {result.get("invokeError")}'
    return result.get('value')


def rejected(name, values, message):
    result = invoke_result(name, values)
    assert not result['ok'] and message in result['invokeError'], name + ' did not reject the conflicting operation'


def wait(script, seconds=90):
    until = time.monotonic() + seconds
    while time.monotonic() < until:
        value = execute(script)
        if value:
            return value
        time.sleep(.2)
    raise AssertionError('UI timeout: ' + script)


def click_text(label):
    element = wait('return [...document.querySelectorAll("button")].find(e=>e.textContent.trim()===' + json.dumps(label) + '&&!e.disabled)||null;')
    execute('arguments[0].click();', element)


def set_native_select(element_id, value):
    execute('const e=document.getElementById(arguments[0]);Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype,"value").set.call(e,arguments[1]);e.dispatchEvent(new Event("change",{bubbles:true}));', element_id, value)


def select_dialog_model(model):
    # Language first, model second; native model ID remains in the actual option value.
    button = wait('const e=document.querySelectorAll("[role=dialog][data-state=open] [role=combobox]")[1];return e&&!e.disabled?e:null;')
    execute('arguments[0].focus();arguments[0].dispatchEvent(new KeyboardEvent("keydown",{key:"ArrowDown",bubbles:true}));', button)
    label = 'Qwen3-ASR 0.6B' if model == small else 'Qwen3-ASR 1.7B'
    option = wait('return [...document.querySelectorAll("[role=option]")].find(e=>e.textContent.includes(' + json.dumps(label) + '))||null;')
    execute('arguments[0].click();', option)


def listen_events():
    execute('window.batchEvents=[];')
    for name in ['import-complete', 'import-error', 'retranscription-complete', 'retranscription-error']:
        callback = execute('return window.__TAURI_INTERNALS__.transformCallback(e=>window.batchEvents.push(e));')
        invoke('plugin:event|listen', {'event': name, 'target': {'kind': 'Any'}, 'handler': callback})


def await_event(name):
    return wait('return window.batchEvents.find(e=>e.event===' + json.dumps(name) + ')||null;', 120)['payload']


def requests():
    path = root / 'requests.jsonl'
    return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []


def database():
    # WebKit also creates hsts-storage.sqlite; it is not an application database.
    files = list((root / 'data').rglob('meeting_minutes.sqlite'))
    assert len(files) == 1, f'Expected one isolated database, got {len(files)}'
    return files[0]


def saved(meeting_id):
    with sqlite3.connect(database()) as db:
        folder = pathlib.Path(db.execute('SELECT folder_path FROM meetings WHERE id=?', (meeting_id,)).fetchone()[0])
        rows = db.execute('SELECT transcript,audio_start_time,audio_end_time,speaker FROM transcripts WHERE meeting_id=? ORDER BY audio_start_time,rowid', (meeting_id,)).fetchall()
    assert folder.resolve().is_relative_to(root)
    raw = json.loads((folder / 'transcripts.json').read_text())['segments']
    assert rows and len(rows) == len(raw)
    for row, segment in zip(rows, raw):
        assert row[0] == segment['text'] == text
        assert abs(row[1] - segment['audio_start_time']) < 1e-9
        assert abs(row[2] - segment['audio_end_time']) < 1e-9
        assert row[3] is None and segment.get('speaker') is None
    return folder, rows, hashlib.sha256((folder / 'transcripts.json').read_bytes()).hexdigest()


def native_pid():
    # Inspect only descendants of this owned driver, never process arguments/env.
    pending = [driver.pid]
    found = []
    while pending:
        pid = pending.pop()
        entry = pathlib.Path('/proc') / str(pid)
        try:
            pending.extend(int(value) for value in (entry / 'task' / str(pid) / 'children').read_text().split())
            if (entry / 'exe').resolve() == pathlib.Path(args.binary).resolve():
                found.append(pid)
        except (OSError, ValueError):
            continue
    assert len(found) == 1, f'Expected one owned native process, got {len(found)}'
    return found[0]


def new_session():
    value = request('POST', '/session', {'capabilities': {'alwaysMatch': {'webkitgtk:browserOptions': {'binary': str(wrapper)}}}})
    return value['sessionId']


try:
    print('PRIVATE_SYNTHETIC_PROFILE=' + str(root), flush=True)
    for command, name in [(['Xvfb', args.display, '-screen', '0', '1280x900x24', '-nolisten', 'tcp'], 'xvfb.log'),
                          (['dbus-daemon', '--session', '--nofork', f'--address=unix:path={root}/bus'], 'dbus.log')]:
        process = spawn(command, name)
        time.sleep(.4)
        assert process.poll() is None
    driver = spawn([args.driver, f'--port={args.port}', '--host=127.0.0.1'], 'driver.log')
    for _ in range(100):
        try:
            request('GET', '/status')
            break
        except (OSError, urllib.error.URLError):
            time.sleep(.1)
    assert driver.poll() is None
    session = new_session()
    request('POST', f'/session/{session}/timeouts', {'script': 120000})
    wait('return !!window.__TAURI_INTERNALS__&&!!document.getElementById("qwen-model");')
    assert execute('return location.origin;') == 'tauri://localhost'
    time.sleep(2)
    for model in [large, small]:
        invoke('qwen_save_settings', {'settings': settings[model]})
    execute('localStorage.setItem("primaryLanguage","en");location.reload();')
    wait('return !!document.getElementById("qwen-model");')
    click_text('Use installed model')
    wait("""return !!document.querySelector('button[aria-label="Start recording"]');""")
    assert invoke('is_recording') is False
    initial_native_pid = native_pid()
    assert [entry['name'] for entry in invoke('qwen_get_available_models')] == [large, small]
    events.append('Qwen-only onboarding uses installed fixture; both models discovered; no recording')
    listen_events()
    for selected, batch_model in [(large, small), (small, large)]:
        if selected == small:
            wait('return !document.querySelector("[role=dialog]");')
            button = wait('return [...document.querySelectorAll("button")].find(e=>e.textContent.trim()==="Settings"||e.querySelector("svg.lucide-settings"))||null;')
            execute('arguments[0].click();', button)
            tab = wait('return [...document.querySelectorAll("[role=tab]")].find(e=>e.textContent.trim()==="Transcription")||null;')
            execute('arguments[0].dispatchEvent(new MouseEvent("mousedown",{bubbles:true,button:0}));arguments[0].click();', tab)
            wait('return !!document.getElementById("qwen-model");')
            set_native_select('qwen-model', small)
            wait('return document.getElementById("qwen-model")?.value===' + json.dumps(small) + ';')
            time.sleep(.5)
            click_text('Use installed model')
            until = time.monotonic() + 30
            while invoke('api_get_transcript_config')['model'] != small:
                assert time.monotonic() < until
                time.sleep(.2)
        assert invoke('api_get_transcript_config')['model'] == selected
        execute('window.batchEvents=[];')
        before = len(requests())
        invoke('plugin:event|emit', {'event': 'tauri://drag-drop', 'payload': {'paths': [str(audio)]}})
        wait('return !!document.querySelector("[role=dialog]");')
        select_dialog_model(batch_model)
        click_text('Import')
        result = await_event('import-complete')
        meeting_id = result['meeting_id']
        folder, _, _ = saved(meeting_id)
        meetings.append(meeting_id)
        assert requests()[before:] and all(row['model'] == batch_model.removesuffix('-INT8') for row in requests()[before:])
        assert invoke('api_get_transcript_config')['model'] == selected
        assert invoke('is_recording') is False
        button = wait("""return document.querySelector('button[title="Retranscribe to enhance your recorded audio"]')||null;""")
        execute('arguments[0].click();', button)
        select_dialog_model(selected)
        execute('window.batchEvents=[];')
        before = len(requests())
        click_text('Start Retranscription')
        await_event('retranscription-complete')
        saved(meeting_id)
        assert requests()[before:] and all(row['model'] == selected.removesuffix('-INT8') for row in requests()[before:])
        events.append(f'{selected}: UI import via {batch_model}, UI retranscription via {selected}, unknown origin, JSON/DB equality')
    # Native fault/cancel paths on the last synthetic meeting, not user data.
    meeting_id = meetings[-1]
    folder, baseline_rows, baseline_hash = saved(meeting_id)
    for control in ['fail', 'hold']:
        execute('window.batchEvents=[];')
        (root / control).touch()
        before = len(requests())
        invoke('start_retranscription_command', {'meetingId': meeting_id, 'meetingFolderPath': str(folder), 'language': 'en', 'model': small, 'provider': 'qwen'})
        until = time.monotonic() + 60
        while len(requests()) == before:
            assert time.monotonic() < until
            time.sleep(.1)
        if control == 'hold':
            rejected('qwen_init', {}, 'finish')
            rejected('qwen_validate_settings', {'settings': settings[small], 'model': small}, 'Stop audio')
            rejected('qwen_save_settings', {'settings': settings[small]}, 'Stop audio')
            rejected('api_save_transcript_config', {'provider': 'qwen', 'model': large, 'apiKey': None}, 'Stop audio')
            rejected('qwen_setup_start', {'consent': True, 'model': large}, 'Stop audio')
            assert invoke('api_get_transcript_config')['model'] == small
            events.append('held inference rejects setup/model changes and recording preflight without changing selection')
            invoke('cancel_retranscription_command')
        await_event('retranscription-error')
        (root / control).unlink()
        _, rows, digest = saved(meeting_id)
        assert rows == baseline_rows and digest == baseline_hash
        assert not any((root / 'cache').rglob('.tmp*'))
        events.append(control + ': existing DB/JSON preserved, no fallback/retry, temporary audio removed')
    assert invoke('is_recording') is False
    expected = {meeting: saved(meeting)[2] for meeting in meetings}
    request('DELETE', f'/session/{session}')
    session = None
    until = time.monotonic() + 15
    while pathlib.Path('/proc', str(initial_native_pid)).exists():
        assert time.monotonic() < until, 'Old native process did not exit'
        time.sleep(.1)
    session = new_session()
    assert native_pid() != initial_native_pid
    wait("""return !!document.querySelector('button[aria-label="Start recording"]');""")
    assert invoke('api_get_transcript_config')['model'] == small
    assert invoke('is_recording') is False
    for meeting in meetings:
        assert saved(meeting)[2] == expected[meeting]
    events.append('native restart preserves small-model selection and both imported meetings without capture')
    passed = True
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
    (root / 'result.json').write_text(json.dumps({'passed': passed, 'events': events,
        'real_audio_used': False, 'model_weights_used': False, 'recording_used': False}, indent=2))
    print(json.dumps({'passed': passed, 'completed_checks': len(events), 'real_audio_used': False,
                      'model_weights_used': False, 'recording_used': False}), flush=True)
