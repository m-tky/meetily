"""Full fixed public corpus after VAD: never discard review-required text from CER."""
import argparse,hashlib,json,pathlib,time
from worker_client import Worker
p=argparse.ArgumentParser();p.add_argument('root',type=pathlib.Path);a=p.parse_args();root=a.root.resolve();art=root/'artifacts'
manifest=json.loads((art/'manifest.json').read_text());out=root/'worker-fixtures/gated-accuracy.jsonl'
selected=[r for r in manifest['rows'] if r['purpose'] in ['accuracy','silence']]
with Worker(root/'worker-target/release/qwen-speech-worker',art/'model-1.7B') as w,out.open('w') as f:
 for index,row in enumerate(selected):
  path=art/row['wav'];assert hashlib.sha256(path.read_bytes()).hexdigest()==row['sha256']
  tick=time.monotonic();r=w.request({'op':'transcribe','id':f'corpus-{index}','wav':str(path),'language':row['language']})
  if r['status'] not in ['recognized','review_required','no_speech']:raise RuntimeError(r)
  result={'id':row['id'],'language':row['language'],'audio_seconds':row['audio_seconds'],'text':r['text'],'status':'ok','rep':0,'elapsed_seconds':r.get('inference_seconds',0),'request_seconds':time.monotonic()-tick,'engine':'vad-gated-native-rust','mode':'window-final','precision':'decoder-INT8-encoder-prefill-F32','threads':6,'worker':r}
  f.write(json.dumps(result,ensure_ascii=False)+'\n');f.flush()
  print(json.dumps({'id':row['id'],'worker_status':r['status'],'asr_calls':r['asr_calls']}),flush=True)
