"""Pace PUBLIC prerecorded PCM to the C engine's live stdin (no capture).
Model-ready wait is separate. 2s arrival chunks match native Rust paced runner.
"""
import argparse
import codecs
import json
import os
import pathlib
import re
import resource
import subprocess
import threading
import time
import wave

p=argparse.ArgumentParser()
p.add_argument('root',type=pathlib.Path)
p.add_argument('--size',required=True,choices=['0.6B','1.7B'])
p.add_argument('--threads',type=int,default=6)
a=p.parse_args()
root=a.root.resolve(); artifacts=root/'artifacts'; out=root/'results';out.mkdir(exist_ok=True)
manifest=json.loads((artifacts/'manifest.json').read_text())
label=f'c-{a.size}-t{a.threads}-paced-r1'
records=[]
for language in ['Japanese','English','Thai']:
    row=next(r for r in manifest['rows'] if r['purpose']=='accuracy' and r['language']==language)
    with wave.open(str(artifacts/row['wav'])) as wav:
        if (wav.getframerate(),wav.getnchannels(),wav.getsampwidth())!=(16000,1,2):raise RuntimeError('Bad WAV')
        pcm=wav.readframes(wav.getnframes())
    ready=threading.Event(); state={'started':None,'first':None,'pieces':[],'stderr':[]}
    env=os.environ.copy(); env.update(OPENBLAS_NUM_THREADS=str(a.threads),OMP_NUM_THREADS=str(a.threads))
    command=[str(root/'c-native/qwen_asr'),'-d',str(artifacts/f'model-{a.size}'),'-t',str(a.threads),'--stdin','--stream','--language',language]
    def limit():resource.setrlimit(resource.RLIMIT_AS,(18*1024**3,18*1024**3))
    load_start=time.monotonic()
    proc=subprocess.Popen(command,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=env,preexec_fn=limit)
    def errors():
        for line in iter(proc.stderr.readline,b''):
            text=line.decode('utf8','replace');state['stderr'].append(text)
            if 'Model loaded.' in text:ready.set()
    def output():
        decoder=codecs.getincrementaldecoder('utf8')('strict')
        while True:
            chunk=os.read(proc.stdout.fileno(),4096)
            if not chunk:break
            text=decoder.decode(chunk)
            state['pieces'].append(text)
            if state['started'] is not None and state['first'] is None and text.strip():state['first']=time.monotonic()-state['started']
        state['pieces'].append(decoder.decode(b'',final=True))
    stderr_thread=threading.Thread(target=errors);stdout_thread=threading.Thread(target=output)
    stderr_thread.start();stdout_thread.start()
    try:
        if not ready.wait(60):raise RuntimeError('Model ready timeout')
        load_seconds=time.monotonic()-load_start
        started=time.monotonic();state['started']=started
        for offset in range(0,len(pcm),64000):
            end=min(offset+64000,len(pcm))
            deadline=started+end/32000
            time.sleep(max(0,deadline-time.monotonic()))
            proc.stdin.write(pcm[offset:end]);proc.stdin.flush()
        proc.stdin.close()
        code=proc.wait(timeout=120)
    except BaseException:
        proc.kill();proc.wait();raise
    finally:
        stderr_thread.join(timeout=2);stdout_thread.join(timeout=2)
    elapsed=time.monotonic()-started
    stderr=''.join(state['stderr']);(out/f'{label}-{language}.stderr').write_text(stderr)
    match=re.search(r'encoding: ([\d.]+)ms, decoding: ([\d.]+)ms',stderr)
    compute=(float(match[1])+float(match[2]))/1000 if match else None
    record={'id':row['id'],'language':language,'text':''.join(state['pieces']).strip(),'status':'ok' if code==0 else 'error',
        'rep':0,'engine':'antirez-c','precision':'unquantized-BF16-F32','mode':'paced','threads':a.threads,
        'audio_seconds':row['audio_seconds'],'elapsed_seconds':elapsed,'compute_seconds':compute,
        'first_token_seconds':state['first'],'load_seconds':load_seconds,'end_delay_seconds':elapsed-row['audio_seconds'],
        'peak_rss_kib':resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss,'arrival_chunk_seconds':2}
    records.append(record)
    with (out/f'{label}.jsonl').open('w') as f:
        for r in records:f.write(json.dumps(r,ensure_ascii=False)+'\n')
    print(json.dumps(record,ensure_ascii=False),flush=True)
    if code!=0:raise RuntimeError('Streaming inference failed')
