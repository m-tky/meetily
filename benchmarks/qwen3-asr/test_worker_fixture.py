"""Offline/public-only VAD + independent worker tests. No capture/playback.
Outputs outside Git; failures remain visible. ASR runs sequentially after build.
"""
import argparse,hashlib,json,math,pathlib,random,struct,time,wave
from worker_client import Worker
p=argparse.ArgumentParser();p.add_argument('root',type=pathlib.Path);p.add_argument('--vad-only',action='store_true');a=p.parse_args()
root=a.root.resolve();out=root/'worker-fixtures';out.mkdir(exist_ok=True)
def read(path):
 with wave.open(str(path),'rb') as w:
  assert w.getframerate()==16000 and w.getsampwidth()==2 and w.getnchannels()==1
  return list(struct.unpack('<'+'h'*w.getnframes(),w.readframes(w.getnframes())))
def write(name,samples):
 path=out/(name+'.wav')
 with wave.open(str(path),'wb') as w:
  w.setnchannels(1);w.setsampwidth(2);w.setframerate(16000);w.writeframes(struct.pack('<'+'h'*len(samples),*samples))
 return path
manifest=json.loads((root/'artifacts/manifest.json').read_text())
row=next(r for r in manifest['rows'] if r['language']=='Japanese' and r['purpose']=='accuracy')
source=root/'artifacts'/row['wav'];assert hashlib.sha256(source.read_bytes()).hexdigest()==row['sha256']
speech=read(source);cases=[]
cases.append(('silence-before','negative',write('silence-before',[0]*80000)))
for amp in [.002,.01,.05,.1]:
 rng=random.Random(321);cases.append((f'white-{amp}','negative',write(f'white-{amp}',[int(rng.uniform(-1,1)*amp*32767) for _ in range(80000)])))
for hz in [50,440]:cases.append((f'tone-{hz}','negative',write(f'tone-{hz}',[int(.05*32767*math.sin(2*math.pi*hz*i/16000)) for i in range(80000)])))
cases.append(('speech','speech',source))
cases.append(('padded','speech',write('padded',[0]*16000+speech+[0]*16000)))
cases.append(('long-tail-silence','speech',write('long-tail-silence',speech+[0]*(480000-len(speech)))))
for factor in [.1,.01]:cases.append((f'quiet-{factor}','speech',write(f'quiet-{factor}',[int(s*factor) for s in speech])))
# Edge proxies: public sentence prefix, NOT proof of genuine Japanese acknowledgement quality.
cases.append(('short-prefix','short-proxy',write('short-prefix',speech[32000:40000])))
cases.append(('partial-frame-tail','speech',write('partial-frame-tail',speech[:128001])))
cases.append(('silence-after','negative',write('silence-after',[0]*80000)))
output=out/('fixed-vad-only.jsonl' if a.vad_only else 'fixed-asr-gated.jsonl')
with Worker(root/'worker-target/release/qwen-speech-worker',root/'artifacts/model-1.7B',vad_only=a.vad_only,timeout=120) as worker,output.open('w') as f:
 for index,(name,kind,path) in enumerate(cases):
  tick=time.monotonic();r=worker.request({'op':'transcribe','id':f'case-{index}','wav':str(path),'language':'Japanese'})
  spans=r.get('intervals',r.get('segments',[]))
  item={'case':name,'fixture_kind':kind,'sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'request_seconds':time.monotonic()-tick,**r}
  item['expected_negative_pass']=kind!='negative' or (r['status']=='no_speech' and r['asr_calls']==0 and r['text']=='')
  item['detected_speech']=len(spans)>0
  item['duration_samples']=len(read(path))
  if spans:item['tail_gap_samples']=item['duration_samples']-spans[-1]['end_sample']
  f.write(json.dumps(item,ensure_ascii=False)+'\n');f.flush()
  print(json.dumps(item,ensure_ascii=False),flush=True)
  assert item['expected_negative_pass'],f'Non-speech gate failed: {name}'
  if kind in ['speech','short-proxy']:assert item['detected_speech'],f'Speech gate missed: {name}'
  if not a.vad_only and name in ['speech','padded','long-tail-silence','quiet-0.1','quiet-0.01']:
   normalized=''.join(c for c in r['text'] if c.isalnum())
   expected=''.join(c for c in row['reference'] if c.isalnum())
   assert normalized==expected,f'Public sentence changed: {name}'
  if not a.vad_only and kind in ['speech','short-proxy']:
   assert r['status'] in ['recognized','review_required'] and r['text'].strip(),f'Speech recognition failed: {name}'
   assert spans[0]['start_sample']==0 and spans[-1]['end_sample']==item['duration_samples'],f'Original context/tail lost: {name}'
 # Invalid input and duplicate ID don't execute inference; following silence still suppressed.
 bad=worker.request({'op':'transcribe','id':'bad','wav':str(source),'language':'unsupported'})
 assert bad['status']=='error'
 duplicate=worker.request({'op':'ping','id':'bad'});assert duplicate['code']=='invalid_or_duplicate_id'
 result=worker.request({'op':'transcribe','id':'final-silence','wav':str(cases[-1][2]),'language':'Japanese'})
 assert result['status']=='no_speech' and result['asr_calls']==0
print('RESULTS',output)
