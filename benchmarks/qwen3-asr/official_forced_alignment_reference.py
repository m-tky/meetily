"""Official qwen-asr0.0.6/nagisa control, CPU-only/local weights, identical raw texts.
Standalone comparison, never edits the existing worker/dependency or ASR result.
"""
import argparse
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import time

os.environ.update(HF_HUB_OFFLINE='1',TRANSFORMERS_OFFLINE='1',HF_HUB_DISABLE_TELEMETRY='1',DO_NOT_TRACK='1')


def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--model',type=Path,required=True)
    ap.add_argument('--directory',type=Path,required=True);ap.add_argument('--native-pass',type=int,default=0)
    a=ap.parse_args();d=a.directory.resolve();out=d/f'official-control-{a.native_pass}';out.mkdir(exist_ok=False)
    import torch
    torch.set_num_threads(6);torch.set_num_interop_threads(1)
    from qwen_asr import Qwen3ForcedAligner
    tick=time.monotonic()
    model=Qwen3ForcedAligner.from_pretrained(str(a.model.resolve()),dtype=torch.float32,device_map='cpu',attn_implementation='eager')
    load=time.monotonic()-tick
    plan=json.loads((d/'plan.json').read_text());jobs=[]
    for f in plan['fixtures']:
        native=json.loads((d/f'pass-{a.native_pass}'/(f['id']+'.json')).read_text())
        raw=native['response'].get('text','')
        if raw:jobs.append(dict(id=f['id']+'-raw',wav=f['path'],text=raw,text_origin='identical_native_asr_raw',fixture=f['id']))
        if f.get('reference'):jobs.append(dict(id=f['id']+'-oracle',wav=f['path'],text=f['reference'],text_origin='published_reference_control_not_ASR',fixture=f['id']))
    turns=next((f for f in plan['fixtures'] if f['id']=='turns-20'),plan['fixtures'][0])
    for i,text in enumerate(plan['wrong_text_controls']):jobs.append(dict(id=f'wrong-{i}',wav=turns['path'],text=text,text_origin='intentionally_wrong_control',fixture='turns-20'))
    jobs.append(dict(id='wrong-silence',wav=str(d/'silence-5.wav'),text='はい',text_origin='intentionally_wrong_control',fixture='silence-5'))
    rows=[]
    for job in jobs:
        tick=time.monotonic()
        try:
            aligned=model.align(audio=job['wav'],text=job['text'],language='Japanese')[0]
            units=[dict(text=x.text,start_seconds=x.start_time,end_seconds=x.end_time,confirmed=False) for x in aligned]
            r=dict(job,status='aligned',units=units,inference_seconds=time.monotonic()-tick,
                   unit='official_nagisa_word',confirmed=False,recognition_verified=False)
        except Exception as error:
            r=dict(job,status='failed',error=type(error).__name__,message=str(error)[:500],inference_seconds=time.monotonic()-tick)
        (out/(job['id']+'.json')).write_text(json.dumps(r,ensure_ascii=False,indent=2)+'\n')
        rows.append(dict(id=job['id'],status=r['status'],seconds=r['inference_seconds'],units=len(r.get('units',[]))))
        print(json.dumps(rows[-1]),flush=True)
    import qwen_asr.inference.qwen3_forced_aligner as source
    source_path=Path(source.__file__)
    (out/'provenance.json').write_text(json.dumps(dict(model_dir=str(a.model.resolve()),model_load_seconds=load,
        package_versions={k:importlib.metadata.version(k) for k in ('qwen-asr','torch','transformers','nagisa')},
        source_path=str(source_path),source_sha256=hashlib.sha256(source_path.read_bytes()).hexdigest(),
        cpu_threads=6,dtype='float32',attention='eager',automatic_downloads_disabled=True),indent=2)+'\n')
    (out/'summary.json').write_text(json.dumps(rows,indent=2)+'\n')


if __name__=='__main__':main()
