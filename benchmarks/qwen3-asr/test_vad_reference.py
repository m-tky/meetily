"""Sample-based Rust endpoint parity against Silero v4 trigger/temp-end rules.
Reference loop adapted from official v4.0/utils_vad.py, MIT/Silero Team.
See worker/licenses/Silero-MIT.txt. Worker settings: min90ms/silence500ms,
asymmetric pre300/post200ms; merge overlapping pads rather than split them.
This is a synthetic endpoint test, NOT annotated speech-quality validation.
"""
import argparse,json,random,subprocess

def reference(probs,length):
 triggered=False;temp_end=0;current_speech={};speeches=[]
 for i,speech_prob in enumerate(probs):
  if speech_prob>=.5 and temp_end:temp_end=0
  if speech_prob>=.5 and not triggered:
   triggered=True;current_speech['start']=512*i;continue
  if speech_prob<.35 and triggered:
   if not temp_end:temp_end=512*i
   if 512*i-temp_end<8000:continue
   current_speech['end']=temp_end
   if current_speech['end']-current_speech['start']>1440:speeches.append(current_speech)
   temp_end=0;current_speech={};triggered=False;continue
 if current_speech and length-current_speech['start']>1440:
  current_speech['end']=length;speeches.append(current_speech)
 result=[]
 for s in speeches:
  v={'start_sample':max(0,s['start']-4800),'end_sample':min(length,s['end']+3200)}
  if result and v['start_sample']<=result[-1]['end_sample']:result[-1]['end_sample']=max(result[-1]['end_sample'],v['end_sample'])
  else:result.append(v)
 return result

if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('binary');a=p.parse_args();rng=random.Random(512)
 cases=[{'probabilities':[.9]*4,'real_samples':1537},{'probabilities':[.9]*3,'real_samples':1440}]
 for _ in range(300):
  n=rng.randint(1,200);cases.append({'probabilities':[rng.choice([0,.349,.35,.499,.5,.501,.9]) for _ in range(n)],'real_samples':512*n-rng.randrange(512)})
 for x in [0,.35,.5,.9]:
  cases.append({'probabilities':[x]*100,'real_samples':51200})
 completed=subprocess.run([a.binary],input=''.join(json.dumps(c)+'\n' for c in cases),text=True,capture_output=True,check=True)
 results=[json.loads(l) for l in completed.stdout.splitlines()];assert len(results)==len(cases)
 for c,r in zip(cases,results):assert r==reference(c['probabilities'],c['real_samples']),(c,r)
 print(f'V4 endpoint reference: {len(cases)} cases passed (seed 512)')
