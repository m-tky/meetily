#!/usr/bin/env python3
"""Synthetic-only capture/Stop-tail/SQLite/restart smoke; no cloud/LLM/auth.
Private Pulse server loads ONLY null sink + private Unix protocol, no hardware.
ASR defaults to a fixture; explicit --real-* paths opt into local Qwen inference.
"""
import argparse, base64, hashlib, json, math, os, pathlib, shutil, signal, sqlite3
import subprocess, sys, tempfile, textwrap, time, urllib.request, urllib.error, urllib.parse

parser = argparse.ArgumentParser()
for name in ['binary', 'bootstrap', 'driver', 'runtime', 'espeak']:
    parser.add_argument('--'+name, required=True)
parser.add_argument('--display', default=':109')
parser.add_argument('--port', type=int, default=4446)
parser.add_argument('--fault', choices=['none', 'worker-crash', 'save-failure'], default='none')
for name in ['real-worker', 'real-model', 'real-ort']:
    parser.add_argument('--'+name)
parser.add_argument('--soak-seconds',type=int,default=0,help='Real-ASR-only bounded synthetic repeat run (60..3600 seconds)')
parser.add_argument('--clean-native-env',action='store_true',help='Strip development library settings from the native app launch')
parser.add_argument('--expected-origin',help='Require the embedded frontend origin, e.g. tauri://localhost')
parser.add_argument('--model-size', choices=['1.7B', '0.6B'], default='1.7B')
parser.add_argument('--no-screenshots', action='store_true')
args = parser.parse_args()
model_name = 'Qwen3-ASR-' + args.model_size
model_id = model_name + '-INT8'
real_asr = any([args.real_worker,args.real_model,args.real_ort])
if real_asr and (not all([args.real_worker,args.real_model,args.real_ort]) or args.fault!='none'):
    parser.error('Real ASR requires all three --real-* paths and --fault none')
if args.soak_seconds and (not real_asr or not 60<=args.soak_seconds<=3600):
    parser.error('--soak-seconds requires real ASR and a duration of 60..3600')
root = pathlib.Path(tempfile.mkdtemp(prefix='recording-gui-', dir=args.runtime)).resolve()
(root/'.synthetic-gui-test').write_text('synthetic-only\n')
for name in ['home/Documents', 'home/Downloads', 'data', 'config', 'cache', 'run', 'fixture-model']:
    (root/name).mkdir(parents=True)
(root/'run').chmod(0o700)
(root/'config/user-dirs.dirs').write_text(f'XDG_DOCUMENTS_DIR="{root}/home/Documents"\nXDG_DOWNLOAD_DIR="{root}/home/Downloads"\n')
# No ALSA physical devices, even if a default mic is requested/fallback is attempted.
(root/'no-hardware-alsa.conf').write_text('pcm.!default { type hw card 999 }\nctl.!default { type hw card 999 }\n')
keep = ['PATH','LD_LIBRARY_PATH','FONTCONFIG_FILE','FONTCONFIG_PATH','GTK_PATH','GI_TYPELIB_PATH','XDG_DATA_DIRS','GSETTINGS_SCHEMA_DIR','LOCALE_ARCHIVE','LANG','LC_ALL','TZ']
env = {k:os.environ[k] for k in keep if k in os.environ}
env.update(HOME=str(root/'home'),XDG_DATA_HOME=str(root/'data'),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache'),XDG_RUNTIME_DIR=str(root/'run'),DISPLAY=args.display,GDK_BACKEND='x11',DBUS_SESSION_BUS_ADDRESS=f'unix:path={root}/bus',PULSE_SERVER=f'unix:{root}/pulse',ALSA_CONFIG_PATH=str(root/'no-hardware-alsa.conf'),TAURI_WEBVIEW_AUTOMATION='true',MEETILY_DISABLE_ANALYTICS='1',NEXT_TELEMETRY_DISABLED='1')
subprocess.run([args.bootstrap,str(root)],env=env,check=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
data = root/'data/com.meetily.ai'
(data/'onboarding-status.json').write_text(json.dumps({'status':{'version':'1.0','completed':True,'current_step':5,'model_status':{'parakeet':'not_downloaded','summary':'not_downloaded'},'last_updated':'2026-01-01T00:00:00Z'}}))
if real_asr:
    worker = pathlib.Path(args.real_worker).resolve(strict=True)
    model = pathlib.Path(args.real_model).resolve(strict=True)
    ort = pathlib.Path(args.real_ort).resolve(strict=True)
    assert worker.is_file() and model.is_dir() and ort.is_file()
    fixture_text = None  # Never substitute or correct actual recognition.
    (root/'real-asr-manifest.json').write_text(json.dumps({
        'worker':str(worker),'worker_sha256':hashlib.sha256(worker.read_bytes()).hexdigest(),
        'model':str(model),'ort':str(ort),'language':'en','synthetic_source':'espeak-ng',
        'model_files':{p.name:{'bytes':p.stat().st_size,'mtime_ns':p.stat().st_mtime_ns}
            for p in model.iterdir() if p.is_file()},
    },indent=2))
else:
    # Explicit fake artifacts satisfy path/shape validation, never contain model weights.
    model = root/'fixture-model'
    (model/'config.json').write_text(json.dumps({'model_type':'qwen3_asr','thinker_config':{'text_config':{'hidden_size':1024 if args.model_size=='0.6B' else 2048,'num_hidden_layers':28}}}))
    weights = ['model.safetensors'] if args.model_size=='0.6B' else ['model-00001-of-00002.safetensors','model-00002-of-00002.safetensors']
    for name in weights + ['qwen-asr-int8.sidecar','vocab.json','merges.txt']:
        (model/name).write_text('SYNTHETIC TEST PLACEHOLDER — NOT MODEL WEIGHTS\n')
    (root/'fixture-ort').write_text('SYNTHETIC TEST PLACEHOLDER — NOT A LIBRARY\n')
    fixture_text = 'SYNTHETIC WORKER FIXTURE: proposal is pending, verify manually.'
    worker = root/'fixture-worker'
    worker.write_text('#!'+sys.executable+'\n'+textwrap.dedent('''
    import json, os, pathlib, struct, sys, time
    root = pathlib.Path(__file__).parent
    assert (root/'.synthetic-gui-test').read_text() == 'synthetic-only\\n'
    print(json.dumps(dict(event='ready',protocol=1,threads=6,model=MODEL_NAME,precision='decoder-INT8-encoder-prefill-F32',recognition_context='full_input',vad_only=False,max_audio_seconds=30,vad_frame_samples=512,audio_formats=['float32'])),flush=True)
    for raw in sys.stdin:
        req=json.loads(raw); wav=pathlib.Path(req['wav']).resolve()
        assert wav.is_relative_to(root/'cache')
        pcm=wav.read_bytes(); pos=pcm.index(b'data')+8
        assert pcm[:4]==b'RIFF' and pcm[20:22]==b'\\x03\\x00'
        samples=struct.unpack('<'+'f'*((len(pcm)-pos)//4),pcm[pos:])
        assert samples and max(abs(x) for x in samples)>0.001
        with (root/'worker-requests.jsonl').open('a') as f:
            f.write(json.dumps(dict(id=req['id'],pid=os.getpid(),samples=len(samples),peak=max(abs(x) for x in samples)))+'\\n')
        if (root/'crash-once').exists():
            (root/'crash-once').unlink(); sys.exit(7)
        deadline=time.monotonic()+90
        while not (root/'release-response').exists():
            if time.monotonic()>deadline: sys.exit(8)
            time.sleep(.05)
        print(json.dumps(dict(id=req['id'],confirmed=False,text=TEXT,asr_calls=1,status='recognized',recognition_context='full_input')),flush=True)
    ''').replace('text=TEXT','text='+repr(fixture_text)).replace('MODEL_NAME',repr(model_name)))
    compile(worker.read_text(),str(worker),'exec')  # Catch harness generation errors before native start.
    worker.chmod(0o700)
    if args.fault=='worker-crash': (root/'crash-once').touch()
    ort = root/'fixture-ort'
speech = root/'synthetic.wav'
subprocess.run([args.espeak,'-v','en','-s','140','-w',str(speech),'This is synthetic audio for the recording test. The proposal is pending. Please verify the saved recording manually.'],env=env,check=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
wrapper=root/'launch-app.sh'
native_prefix = ('env -u LD_LIBRARY_PATH -u GTK_PATH -u GI_TYPELIB_PATH -u GSETTINGS_SCHEMA_DIR -u FONTCONFIG_FILE -u FONTCONFIG_PATH -u XDG_DATA_DIRS PATH=/run/current-system/sw/bin ' if args.clean_native_env else '')
wrapper.write_text('#!'+shutil.which('bash')+'\nexec '+native_prefix+'"'+str(pathlib.Path(args.binary).resolve())+'" >>"'+str(root/'native.log')+'" 2>&1\n')
wrapper.chmod(0o700)
processes=[]; handles=[]; session=None; events=[]; passed=False

def spawn(command,name):
    log=open(root/name,'ab'); handles.append(log)
    p=subprocess.Popen(command,env=env,stdout=log,stderr=log,start_new_session=True)
    processes.append(p); return p

def stop(p):
    if p.poll() is None:
        os.killpg(p.pid,signal.SIGTERM)
        try: p.wait(timeout=10)
        except subprocess.TimeoutExpired:
            os.killpg(p.pid,signal.SIGKILL);p.wait()

base=f'http://127.0.0.1:{args.port}'
def request(method,path,body=None):
    payload=None if body is None else json.dumps(body).encode()
    req=urllib.request.Request(base+path,payload,method=method,headers={'Content-Type':'application/json'})
    try:
        with urllib.request.urlopen(req,timeout=120) as response: result=json.load(response)
    except urllib.error.HTTPError as e: raise RuntimeError(e.read().decode()) from e
    value=result.get('value')
    if isinstance(value,dict) and value.get('error'): raise RuntimeError(json.dumps(value))
    return value

def execute(script,*values):
    return request('POST',f'/session/{session}/execute/sync',{'script':script,'args':list(values)})
def invoke(name,values=None):
    return request('POST',f'/session/{session}/execute/async',{'script':'const done=arguments[arguments.length-1];window.__TAURI_INTERNALS__.invoke(arguments[0],arguments[1]).then(done).catch(e=>done({error:String(e)}));','args':[name,values or {}]})
def wait(script,timeout=90):
    until=time.monotonic()+timeout
    while time.monotonic()<until:
        result=execute(script)
        if result: return result
        time.sleep(.25)
    raise RuntimeError('UI timeout: '+script)
def wait_file(path,timeout=60):
    until=time.monotonic()+timeout
    while time.monotonic()<until:
        if path.exists() and (path.is_socket() or path.stat().st_size): return
        time.sleep(.2)
    raise RuntimeError('No expected fixture artifact: '+str(path))
def click_label(label):
    element=wait('return [...document.querySelectorAll("button")].find(e=>e.getAttribute("aria-label")=='+json.dumps(label)+'&&!e.disabled)||null;')
    execute('arguments[0].click();',element)
def screenshot(name):
    if args.no_screenshots: return
    (root/name).write_bytes(base64.b64decode(request('GET',f'/session/{session}/screenshot')))
def start_driver():
    driver=spawn([args.driver,f'--port={args.port}','--host=127.0.0.1'],'driver.log')
    for _ in range(50):
        try: request('GET','/status');return driver
        except Exception:time.sleep(.2)
    raise RuntimeError('WebDriver did not start')
def start_session():
    return request('POST','/session',{'capabilities':{'alwaysMatch':{'webkitgtk:browserOptions':{'binary':str(wrapper)}}}})['sessionId']
def read_requests():
    path=root/'worker-requests.jsonl'
    return [json.loads(x) for x in path.read_text().splitlines()] if path.exists() else []

try:
    print('PRIVATE_PROFILE='+str(root),flush=True)
    spawn(['Xvfb',args.display,'-screen','0','1280x900x24','-nolisten','tcp'],'xvfb.log')
    spawn(['dbus-daemon','--session','--nofork',f'--address=unix:path={root}/bus'],'dbus.log')
    pulse=spawn(['pulseaudio','--daemonize=no','--exit-idle-time=-1','--use-pid-file=no','--disallow-exit','-n','--load='+f'module-native-protocol-unix socket={root}/pulse auth-anonymous=1','--load=module-null-sink sink_name=meetily_fixture rate=48000 channels=1'],'pulse.log')
    wait_file(root/'pulse')
    sources=json.loads(subprocess.check_output(['pactl','--format=json','list','sources'],env=env))
    assert len(sources)==1 and sources[0]['name']=='meetily_fixture.monitor'
    events.append('private Pulse server has only the synthetic monitor; ALSA hardware disabled')
    driver=start_driver(); session=start_session()
    wait('return [...document.querySelectorAll("button")].some(e=>Object.keys(e).some(k=>k.startsWith("__reactProps")));')
    if args.expected_origin:
        origin=execute('return location.origin;')
        assert origin==args.expected_origin, f'Wrong frontend origin: {origin}'
        (root/'frontend-origin.txt').write_text(origin+'\n')
    invoke('qwen_save_settings',{'settings':{'worker_path':str(worker),'model_dir':str(model),'ort_library':str(ort)}})
    invoke('api_save_transcript_config',{'provider':'qwen','model':model_id,'apiKey':None})
    preferences=invoke('get_recording_preferences')
    preferences.update(auto_save=True,preferred_mic_device=None,preferred_system_device='pulse-monitor:meetily_fixture.monitor (output)')
    invoke('set_recording_preferences',{'preferences':preferences})
    execute('localStorage.setItem("primaryLanguage","en");location.reload();')
    wait('return document.readyState=="complete" && [...document.querySelectorAll("button")].some(e=>Object.keys(e).some(k=>k.startsWith("__reactProps")));')
    time.sleep(2)
    invoke('set_language_preference',{'language':'en'})
    # Instrument only public native events. No injected transcript or provider result.
    execute('window.fixtureEvents=[];')
    for name in ['recording-stopped','recording-save-warning','transcription-error','transcript-error','transcript-update','recording-error','transcription-warning']:
        callback=execute('return window.__TAURI_INTERNALS__.transformCallback(e=>window.fixtureEvents.push(e));')
        invoke('plugin:event|listen',{'event':name,'target':{'kind':'Any'},'handler':callback})
    click_label('Start recording')
    wait('return !!document.querySelector("button[aria-label=\\"Stop recording\\"]");')
    assert invoke('is_recording') is True
    folder=pathlib.Path(invoke('get_meeting_folder_path')).resolve()
    assert folder.is_relative_to(root/'home/Documents/meetily-recordings')
    screenshot('01-recording.png')
    playback=spawn(['paplay','--device=meetily_fixture',str(speech)],'playback.log')
    if args.soak_seconds:
        from soak_monitor import SoakMonitor
        monitor=SoakMonitor(root,driver.pid,pathlib.Path(args.binary).resolve(),worker)
        soak_start=time.monotonic()
        bursts=1
        def sample_soak():
            assert invoke('is_recording') is True
            assert not execute('return window.fixtureEvents.some(e=>["transcription-error","transcript-error","recording-error","recording-save-warning","transcription-warning"].includes(e.event));')
            monitor.sample(folder)
        while True:
            while playback.poll() is None:
                sample_soak();time.sleep(1)
            assert playback.returncode==0
            sample_soak()
            if time.monotonic()-soak_start>=args.soak_seconds: break
            for _ in range(5): sample_soak();time.sleep(1)
            bursts+=1
            playback=spawn(['paplay','--device=meetily_fixture',str(speech)],f'playback-{bursts}.log')
        soak_seconds=time.monotonic()-soak_start
        monitor.finish(soak_seconds,bursts)
        assert len(monitor.worker_pids)==1, 'Unexpected real worker restart during soak'
        events.append(f'{bursts} complete synthetic bursts over {soak_seconds:.1f}s; one persistent real worker, no native error events')
    if real_asr:
        # Complete playback before Stop: unplayed source must not be mistaken for lost speech.
        assert playback.wait(timeout=40)==0
        # Observe the app-owned float WAV and process, not a proxy or injected result.
        until=time.monotonic()+60
        while True:
            if time.monotonic()>until: raise RuntimeError('App did not send synthetic speech to real Qwen')
            inputs=list((root/'cache/com.meetily.ai/qwen-audio').glob('*'))
            if inputs and inputs[0].stat().st_size>58:
                try: payload=inputs[0].read_bytes()
                except FileNotFoundError: continue  # A very fast result may unlink before observation.
                if len(payload)<58 or int.from_bytes(payload[4:8],'little')+8!=len(payload):
                    time.sleep(.02); continue  # Don't record a partially written temporary WAV.
                assert payload[:4]==b'RIFF' and payload[20:22]==b'\x03\x00'
                assert int.from_bytes(payload[22:24],'little')==1
                assert int.from_bytes(payload[24:28],'little')==16000
                pids=[]
                for p in pathlib.Path('/proc').iterdir():
                    if not p.name.isdigit(): continue
                    try:
                        if (p/'exe').resolve()==worker and os.getpgid(int(p.name))==driver.pid:
                            pids.append(int(p.name))
                    except (OSError,ProcessLookupError): pass
                assert pids, 'No real worker in the owned native process group'
                (root/'worker-requests.jsonl').write_text(json.dumps({'pid':pids[0],
                    'wav_bytes':len(payload),'wav_sha256':hashlib.sha256(payload).hexdigest(),
                    'observed_at':time.monotonic()})+'\n')
                break
            time.sleep(.02)
    else:
        wait_file(root/'worker-requests.jsonl')
    if args.fault=='worker-crash':
        # Only a subsequent new burst may start a fresh worker; failed input isn't replayed.
        playback.wait(timeout=40);time.sleep(2)
        spawn(['paplay','--device=meetily_fixture',str(speech)],'playback-retry.log')
        until=time.monotonic()+60
        while len(read_requests())<2 and time.monotonic()<until: time.sleep(.2)
        assert len({r['pid'] for r in read_requests()})>=2
        wait('return document.body.innerText.includes("Transcription failed");',20)
        assert invoke('is_recording') is True
        events.append('fixture worker crash is visible; capture/Stop remain usable; subsequent audio starts a fresh worker')
    if args.fault=='save-failure':
        path=folder/'metadata.json'
        if path.exists(): path.rename(root/'metadata-before-fault.json')
        path.mkdir()
    # Stop with a pending ASR request. Only fixture mode artificially holds a reply.
    stop_started=time.monotonic()
    click_label('Stop recording')
    until=time.monotonic()+20
    while invoke('get_meeting_folder_path') is not None:
        if time.monotonic()>until: raise RuntimeError('Stop did not remove the recording manager')
        time.sleep(.1)
    before_tail=execute('return window.fixtureEvents.filter(e=>e.event=="transcript-update").map(e=>e.payload.sequence_id);')
    if not real_asr: assert not before_tail
    if real_asr:
        events.append('native Stop removed the manager while real Qwen inference was pending')
    else:
        events.append('native Stop removed the manager before the delayed fixture transcript was released')
        (root/'release-response').touch()
    wait('return window.fixtureEvents.some(e=>e.event=="recording-stopped");',240 if real_asr else 120)
    stop_seconds=time.monotonic()-stop_started
    stopped=execute('return window.fixtureEvents.find(e=>e.event=="recording-stopped").payload;')
    (root/'stop-event.json').write_text(json.dumps(stopped,indent=2))
    assert stopped['transcription_drained'] is (args.fault!='worker-crash')
    (root/'native-events.json').write_text(json.dumps(execute('return window.fixtureEvents;'),indent=2))
    assert stopped['save_succeeded'] is (args.fault!='save-failure')
    if args.fault=='save-failure':
        assert stopped['save_error']
        wait('return document.body.innerText.includes("Recording files were not fully saved");',20)
        assert list((folder/'.checkpoints').glob('audio_chunk_*.wav'))
        (folder/'metadata.json').rmdir()
        shutil.copyfile(root/'metadata-before-fault.json',folder/'metadata.json')
        events.append('completion metadata save failure is explicit; original metadata preserved for restart')
    transcripts=json.loads((folder/'transcripts.json').read_text())
    if real_asr:
        assert transcripts['segments'], 'Real ASR returned no saved speech'
        native_events=execute('return window.fixtureEvents;')
        updates={e['payload']['sequence_id']:e['payload']['text'] for e in native_events if e['event']=='transcript-update'}
        saved={s['sequence_id']:s['text'] for s in transcripts['segments']}
        assert saved==updates and all(text.strip() for text in saved.values())
        assert set(saved)-set(before_tail), 'No real ASR tail arrived after native manager removal'
        assert all(s['confidence']==-1 for s in transcripts['segments'])
        assert not any(e['event'] in ['transcription-error','transcript-error'] for e in native_events)
        fixture_text=transcripts['segments'][0]['text']  # Actual raw output for restart equality only.
        real_result={'stop_wait_seconds':stop_seconds,'segments':transcripts['segments'],
            'asr':'real local Qwen','quality_acceptance':False,'full_synthetic_playback':True,'soak_requested_seconds':args.soak_seconds}
    else:
        assert any(s['text']==fixture_text for s in transcripts['segments'])
    assert (folder/'audio.mp4').is_file() and (folder/'audio.mp4').stat().st_size>1000
    # Decode stored audio and verify nonzero samples, not just a container file.
    decoded=subprocess.run(['ffmpeg','-v','warning','-i',str(folder/'audio.mp4'),'-f','f32le','-ac','1','-ar','16000','-'],env=env,capture_output=True,check=True)
    assert not decoded.stderr, 'Stored audio decode warning/error: '+decoded.stderr.decode(errors='replace')
    pcm=decoded.stdout
    from array import array
    samples=array('f');samples.frombytes(pcm)
    if sys.byteorder!='little': samples.byteswap()
    assert len(samples)>16000 and all(math.isfinite(x) for x in samples) and max(abs(x) for x in samples)>.001
    if real_asr:
        import wave
        with wave.open(str(speech),'rb') as source:
            source_seconds=source.getnframes()/source.getframerate()
        assert len(samples)/16000>=source_seconds-.1, 'Captured audio shorter than complete synthetic playback'
        if args.soak_seconds:
            assert len(samples)/16000>=soak_seconds-1, 'Soak saved audio is shorter than elapsed recording'
            assert len(transcripts['segments'])>=bursts, 'Fewer persisted segments than synthetic bursts'
            real_result.update(soak_seconds=soak_seconds,synthetic_bursts=bursts)
        real_result.update(source_seconds=source_seconds,captured_seconds=len(samples)/16000)
        (root/'real-asr-result.json').write_text(json.dumps(real_result,ensure_ascii=False,indent=2))
    assert not list((root/'cache/com.meetily.ai/qwen-audio').glob('*'))
    events.append(('Real Qwen raw output' if real_asr else 'Stop-tail fixture transcript')+' and nonzero captured audio persisted; temporary input removed')
    wait('return location.pathname.includes("meeting-details");',120)
    url=request('GET',f'/session/{session}/url')
    wait('return document.body.innerText.includes('+json.dumps(fixture_text)+');')
    screenshot('02-saved.png')
    meeting_id=urllib.parse.parse_qs(urllib.parse.urlparse(url).query)['id'][0]
    def sqlite_snapshot():
        with sqlite3.connect(f'file:{data}/meeting_minutes.sqlite?mode=ro',uri=True) as db:
            meeting=db.execute('SELECT folder_path FROM meetings WHERE id=?',(meeting_id,)).fetchone()
            rows=db.execute('SELECT id,transcript,audio_start_time,audio_end_time,duration FROM transcripts WHERE meeting_id=? ORDER BY audio_start_time,id',(meeting_id,)).fetchall()
        return meeting,rows
    sql_before=sqlite_snapshot()
    assert sql_before[0]==(str(folder),)
    assert sorted(row[1] for row in sql_before[1])==sorted(s['text'] for s in transcripts['segments'])
    for pid in {r['pid'] for r in read_requests()}:
        try: os.kill(pid,0)
        except ProcessLookupError: pass
        else: raise AssertionError('ASR worker survived Stop: '+str(pid))
    before={str(p.relative_to(folder)):hashlib.sha256(p.read_bytes()).hexdigest() for p in folder.rglob('*') if p.is_file()}
    request('DELETE',f'/session/{session}');session=None;stop(driver)
    driver=start_driver();session=start_session()
    request('POST',f'/session/{session}/url',{'url':url})
    wait('return document.body.innerText.includes('+json.dumps(fixture_text)+');')
    screenshot('03-reopened.png')
    assert before=={str(p.relative_to(folder)):hashlib.sha256(p.read_bytes()).hexdigest() for p in folder.rglob('*') if p.is_file()}
    assert invoke('is_recording') is False
    assert sqlite_snapshot()==sql_before
    if real_asr:
        manifest=json.loads((root/'real-asr-manifest.json').read_text())
        assert manifest['model_files']=={p.name:{'bytes':p.stat().st_size,'mtime_ns':p.stat().st_mtime_ns}
            for p in model.iterdir() if p.is_file()}
    events.append('native restart reopens the saved SQLite meeting; recording files unchanged')
    passed=True
    print(json.dumps({'passed':events,'fault':args.fault,'asr':'real-qwen' if real_asr else 'fixture','profile':str(root)},ensure_ascii=False),flush=True)
except Exception:
    if session:
        try:
            screenshot('failure.png')
            (root/'failure-dom.txt').write_text(execute('return location.href+"\\n"+document.body.innerText;'))
            (root/'native-events.json').write_text(json.dumps(execute('return window.fixtureEvents||[];'),indent=2))
        except Exception:pass
    raise
finally:
    (root/'result.json').write_text(json.dumps({'passed':passed,'fault':args.fault,'asr':'real-qwen' if real_asr else 'fixture','completed':events},indent=2))
    if session:
        try:request('DELETE',f'/session/{session}')
        except Exception:pass
    for p in reversed(processes):stop(p)
    for h in handles:h.close()
