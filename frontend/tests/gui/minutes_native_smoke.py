#!/usr/bin/env python3
"""Synthetic-only Linux WebKitGTK/Tauri GUI check. No capture, models or auth.
Run under the repo Nix shell with prebuilt meetily/gui_fixture_bootstrap binaries.
Every profile and artifact is newly created outside Git. Only owned process groups
are stopped; a private D-Bus session prevents single-instance handoff to a real app.
"""
import argparse, base64, json, os, pathlib, shutil, signal, subprocess, tempfile, time
import urllib.request, urllib.error

parser = argparse.ArgumentParser()
parser.add_argument('--binary', required=True)
parser.add_argument('--bootstrap', required=True)
parser.add_argument('--driver', required=True)
parser.add_argument('--runtime', required=True)
parser.add_argument('--display', default=':108')
parser.add_argument('--port', type=int, default=4445)
parser.add_argument('--clean-native-env',action='store_true',help='Strip development library settings from the native app launch')
parser.add_argument('--expected-origin',help='Require the embedded frontend origin, e.g. tauri://localhost')
args = parser.parse_args()
root = pathlib.Path(tempfile.mkdtemp(prefix='minutes-gui-', dir=args.runtime)).resolve()
(root / '.synthetic-gui-test').write_text('synthetic-only\n')
for name in ['home/Documents', 'home/Downloads', 'data', 'config', 'cache', 'run']:
    (root / name).mkdir(parents=True, exist_ok=True)
(root / 'run').chmod(0o700)
(root / 'config/user-dirs.dirs').write_text(f'XDG_DOCUMENTS_DIR="{root}/home/Documents"\nXDG_DOWNLOAD_DIR="{root}/home/Downloads"\n')
(root / 'null-alsa.conf').write_text('pcm.!default { type null }\nctl.!default { type hw card 999 }\n')
keep = ['PATH', 'LD_LIBRARY_PATH', 'FONTCONFIG_FILE', 'FONTCONFIG_PATH', 'GTK_PATH', 'GI_TYPELIB_PATH', 'XDG_DATA_DIRS', 'GSETTINGS_SCHEMA_DIR', 'LOCALE_ARCHIVE', 'LANG', 'LC_ALL', 'TZ']
env = {k: os.environ[k] for k in keep if k in os.environ}
env.update(HOME=str(root/'home'), XDG_DATA_HOME=str(root/'data'), XDG_CONFIG_HOME=str(root/'config'), XDG_CACHE_HOME=str(root/'cache'), XDG_RUNTIME_DIR=str(root/'run'), DISPLAY=args.display, GDK_BACKEND='x11', DBUS_SESSION_BUS_ADDRESS=f'unix:path={root}/bus', PULSE_SERVER=f'unix:{root}/no-pulse', ALSA_CONFIG_PATH=str(root/'null-alsa.conf'), TAURI_WEBVIEW_AUTOMATION='true', MEETILY_DISABLE_ANALYTICS='1', NEXT_TELEMETRY_DISABLED='1')
subprocess.run([args.bootstrap, str(root)], env=env, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
data = root/'data/com.meetily.ai'
(data/'onboarding-status.json').write_text(json.dumps({'status': {'version':'1.0','completed':True,'current_step':5,'model_status':{'parakeet':'not_downloaded','summary':'not_downloaded'},'last_updated':'2026-01-01T00:00:00Z'}}))
folder = root/'home/Documents/meetily-recordings/GUI-Synthetic'
folder.mkdir(parents=True)
metadata = dict(version='1.0', meeting_id=None, meeting_name='GUI synthetic meeting', created_at='2026-01-01T00:00:00Z', completed_at='2026-01-01T00:00:02Z', duration_seconds=2, devices={'microphone':None,'system_audio':None}, audio_file='', transcript_file='transcripts.json', sample_rate=48000, status='completed')
segment = dict(id='seg_gui_1', text='人工データ：提案を保留し、手動で確認する。', audio_start_time=0, audio_end_time=2, duration=2, display_time='00:00', confidence=-1, sequence_id=1)
item = dict(id='gui-item-1', kind='question', text='人工データ：提案は未決定。', status='active', sources=['seg_gui_1'], owner=None, due=None)
minutes = dict(schema_version=1, revision=1, fingerprint='synthetic-gui', model='synthetic-no-inference', updated_at='2026-01-01T00:00:02Z', items=[item], user_edits=[])
for name,value in [('metadata.json',metadata),('transcripts.json',dict(version='1.0',segments=[segment],total_segments=1)),('live-minutes.json',minutes)]:
    (folder/name).write_text(json.dumps(value,ensure_ascii=False,indent=2))
original_raw = {name:(folder/name).read_bytes() for name in ['metadata.json','transcripts.json']}
wrapper = root/'launch-app.sh'
native_prefix = ('env -u LD_LIBRARY_PATH -u GTK_PATH -u GI_TYPELIB_PATH -u GSETTINGS_SCHEMA_DIR -u FONTCONFIG_FILE -u FONTCONFIG_PATH -u XDG_DATA_DIRS PATH=/run/current-system/sw/bin ' if args.clean_native_env else '')
wrapper.write_text('#!'+shutil.which('bash')+'\nexec '+native_prefix+'"'+str(pathlib.Path(args.binary).resolve())+'" >"'+str(root/'native.log')+'" 2>&1\n')
wrapper.chmod(0o700)
processes = []; handles = []; session = None; events = []; passed = False
def spawn(command, name):
    log = open(root/name, 'ab'); handles.append(log)
    process = subprocess.Popen(command, env=env, stdout=log, stderr=log, start_new_session=True)
    processes.append(process); return process

def stop(process):
    if process.poll() is None:
        os.killpg(process.pid, signal.SIGTERM)
        try: process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL); process.wait()

base = f'http://127.0.0.1:{args.port}'
def request(method,path,body=None):
    encoded = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(base+path, encoded, method=method, headers={'Content-Type':'application/json'})
    try:
        with urllib.request.urlopen(req,timeout=90) as response: result = json.load(response)
    except urllib.error.HTTPError as e: raise RuntimeError(e.read().decode()) from e
    value = result.get('value')
    if isinstance(value,dict) and value.get('error'): raise RuntimeError(json.dumps(value))
    return value

def execute(script,*values): return request('POST',f'/session/{session}/execute/sync',{'script':script,'args':list(values)})
def wait(script, timeout=60):
    until = time.monotonic()+timeout
    while time.monotonic()<until:
        result = execute(script)
        if result: return result
        time.sleep(.3)
    raise RuntimeError('UI condition timed out: '+script)
def click(text):
    element = wait('return [...document.querySelectorAll("button")].find(e=>e.textContent.trim()=='+json.dumps(text)+'&&!e.disabled) || null;')
    execute('arguments[0].scrollIntoView({block:"center",inline:"center"});', element)
    time.sleep(.3)  # WebKit hit testing must observe layout after details/scroll changes.
    # Exercise the rendered DOM handler, not React internals or mocked IPC.
    # WebKitWebDriver pointer clicks miss controls after split-pane navigation.
    execute('arguments[0].click();', element)
def screenshot(name):
    (root/name).write_bytes(base64.b64decode(request('GET',f'/session/{session}/screenshot')))
def start_session():
    return request('POST','/session',{'capabilities':{'alwaysMatch':{'webkitgtk:browserOptions':{'binary':str(wrapper)}}}})['sessionId']
def view_minutes():
    wait('return [...document.querySelectorAll("summary")].some(e=>e.textContent.includes("Saved minutes"));')
    execute('const s=[...document.querySelectorAll("summary")].find(e=>e.textContent.includes("Saved minutes"));s.parentElement.open=true;')
    wait('return document.body.innerText.includes("Sources: seg_gui_1");')

try:
    print('PRIVATE_PROFILE='+str(root),flush=True)
    spawn(['Xvfb',args.display,'-screen','0','1280x900x24','-nolisten','tcp'],'xvfb.log')
    spawn(['dbus-daemon','--session','--nofork',f'--address=unix:path={root}/bus'],'dbus.log')
    driver = spawn([args.driver,f'--port={args.port}','--host=127.0.0.1'],'driver.log')
    for _ in range(50):
        try: request('GET','/status'); break
        except Exception: time.sleep(.2)
    session = start_session()
    events.append('native WebDriver session attached (DOM-driven buttons; real IPC/filesystem)')
    wait('return [...document.querySelectorAll("button")].some(e=>Object.keys(e).some(k=>k.startsWith("__reactProps")));')
    if args.expected_origin:
        origin=execute('return location.origin;')
        assert origin==args.expected_origin, f'Wrong frontend origin: {origin}'
        (root/'frontend-origin.txt').write_text(origin+'\n')
    default_folder = request('POST',f'/session/{session}/execute/async', {'script': 'const done=arguments[arguments.length-1];window.__TAURI_INTERNALS__.invoke("get_default_recordings_folder_path").then(done).catch(e=>done({error:String(e)}));', 'args': []})
    assert default_folder == str(folder.parent), f'Unexpected recording root: {default_folder}'
    probe = request('POST',f'/session/{session}/execute/async', {'script': 'const done=arguments[arguments.length-1];window.__TAURI_INTERNALS__.invoke("api_scan_recording_recovery",{cursor:null}).then(done).catch(e=>done({error:String(e)}));', 'args': []})
    (root/'scan-probe.json').write_text(json.dumps(probe,ensure_ascii=False,indent=2))
    click('Check saved recordings')
    wait('return document.body.innerText.includes("GUI synthetic meeting");',30)
    screenshot('01-recovery.png')
    click('Add saved data to meeting list')
    view_minutes(); screenshot('02-original.png')
    click('Edit')
    textarea = wait('return document.querySelector("textarea[aria-label=\\"Minutes item text\\"]");')
    element = textarea['element-6066-11e4-a52e-4f735466cecf']
    request('POST',f'/session/{session}/element/{element}/clear',{})
    edited = '人工GUI編集：提案は保留。根拠と履歴を保持する。'
    request('POST',f'/session/{session}/element/{element}/value',{'text':edited})
    click('Save')
    wait('return document.body.innerText.includes("revision 2") && document.body.innerText.includes("Edited · protected");')
    saved = json.loads((folder/'live-minutes.json').read_text())
    assert saved['items'][0]['text']==edited and saved['revision']==2 and len(saved['user_edits'])==1
    assert saved['items'][0]['id']=='gui-item-1' and saved['items'][0]['sources']==['seg_gui_1']
    assert saved['user_edits'][0]['before']['text']==item['text']
    assert saved['user_edits'][0]['after']['text']==edited
    screenshot('03-edited.png'); events.append('GUI edit persisted as revision 2 with original IDs/evidence/history')
    url = request('GET',f'/session/{session}/url')
    request('DELETE',f'/session/{session}'); session=None; stop(driver)
    driver = spawn([args.driver,f'--port={args.port}','--host=127.0.0.1'],'driver-restart.log')
    time.sleep(1); session=start_session()
    request('POST',f'/session/{session}/url',{'url':url})
    view_minutes()
    wait('return document.body.innerText.includes("人工GUI編集：提案は保留。根拠と履歴を保持する。");')
    screenshot('04-reopened.png'); events.append('native app restart reopened the protected edit')
    for label,extension in [('Export JSON','.json'),('Export Markdown','.md')]:
        click(label); time.sleep(2)
        outputs = list((root/'home/Downloads').glob('*'+extension))
        assert outputs, 'Native export did not create a '+extension+' file in isolated Downloads'
        payload=outputs[-1].read_text()
        assert edited in payload
        if extension=='.json':
            assert json.loads(payload)==json.loads((folder/'live-minutes.json').read_text())
        events.append('native '+extension+' download exists and contains saved revision')
    for name,original in original_raw.items(): assert (folder/name).read_bytes()==original
    screenshot('05-exported.png'); events.append('raw metadata/transcripts unchanged; no capture/models/auth invoked')
    passed = True
    print(json.dumps({'passed':events,'profile':str(root)},ensure_ascii=False),flush=True)
except Exception:
    if session:
        try:
            screenshot('failure.png')
            (root/'failure-dom.txt').write_text(execute('return location.href+"\\n"+document.body.innerText;'))
        except Exception: pass
    raise
finally:
    (root/'result.json').write_text(json.dumps({'passed':passed,'completed':events},ensure_ascii=False,indent=2))
    if session:
        try: request('DELETE',f'/session/{session}')
        except Exception: pass
    for process in reversed(processes): stop(process)
    for handle in handles: handle.close()
