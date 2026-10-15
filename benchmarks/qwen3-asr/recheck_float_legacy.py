"""Preserve the exact previous kuruma/hon SNR0 failures, not only a new noise seed.
Same diagnostic float ASR probe; wait until the main batch finishes. No overlap.
"""
import argparse
import json
import pathlib
import resource
import subprocess
import time

import numpy as np
import torch

from compare_vad_gain import quantize, transform
from diagnose_gate import endpoints
from evaluate_float_asr import hash_file, model_hashes
from evaluate_robustness import RATE, mix, noise, read_pcm
from reference_current_vad import load_official, infer


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root',type=pathlib.Path);parser.add_argument('--tag',required=True)
    args=parser.parse_args()
    if not args.tag or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-_' for c in args.tag):parser.error('Invalid tag')
    root=args.root.resolve();repo=pathlib.Path(__file__).resolve().parents[2]
    if root==repo or repo in root.parents:parser.error('Artifacts remain outside Git')
    out=root/'diagnostics'/args.tag;out.mkdir(parents=True,exist_ok=False)
    (out/'frozen-plan.json').write_text(json.dumps(dict(sources=['車','本'],seed=[7212028,7212029],snr=0,
        exact_previous_pcm16_signal=True,representations=['float32','pcm16'],asr_input_gain=1,
        gate='same .7/peak100/>90ms',model_replaced=False,correction=False),ensure_ascii=False,indent=2)+'\n')
    directory=root/'diagnostics/current-vad-validation-r2'
    old=json.loads((directory/'manifest.json').read_text())
    torch.set_num_threads(1);vad,_,_=load_official(root)
    rows=[]
    for i,reference in enumerate(['車','本']):
        converted=directory/f'lexical-converted-{i}.wav'
        if hash_file(converted)!=old['source_records'][i]['converted_sha256']:raise ValueError('Source changed')
        values,gain=mix(read_pcm(converted),noise('white',len(read_pcm(converted)),7212028+i),0)
        legacy=next(r for r in old['rows'] if r['id']==f'lexical-{i}-white-snr0')
        if hash_file(legacy['path'])!=legacy['sha256']:raise ValueError('Legacy fixture changed')
        np.testing.assert_array_equal(quantize(values),np.asarray(read_pcm(pathlib.Path(legacy['path'])),dtype=np.float32))
        for fmt,samples in [('float32',np.asarray(values,dtype=np.float32)),('pcm16',quantize(values))]:
            identifier=f'legacy-{i}-{fmt}';path=out/(identifier+'.f32le');path.write_bytes(samples.astype('<f4').tobytes())
            adjusted,_=transform(samples,'peak100');prob=infer(vad,adjusted);spans=endpoints(prob,len(samples),.7)
            rows.append(dict(id=identifier,path=str(path),sha256=hash_file(path),samples=len(samples),reference=reference,
                language='Japanese',role='speech',suite='legacy-noise',representation=fmt,pair_id=f'legacy-{i}',
                gate_detected=bool(spans),gate_speech_samples=sum(s['end_sample']-s['start_sample'] for s in spans),
                legacy_wav_sha256=legacy['sha256'],source_record=old['source_records'][i],common_gain=gain))
    hashes=model_hashes(root)
    manifest=dict(rows=rows,model_hashes=hashes,exact_legacy_signal=True)
    (out/'manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n')
    keys=['id','path','samples','language','role','gate_detected','gate_speech_samples']
    req=out/'requests.jsonl';req.write_text(''.join(json.dumps({k:r[k] for k in keys})+'\n' for r in rows))
    # Explicit serialization with the larger ASR experiment.
    done=root/'diagnostics/float-asr-r1/run.exit';deadline=time.monotonic()+1500
    while not done.exists():
        if time.monotonic()>deadline:raise TimeoutError('Main ASR has not finished; never overlap')
        time.sleep(10)
    if done.read_text().strip()!='0':raise ValueError('Main experiment failed; followup blocked')
    binary=root/'worker-target/release/examples/float_asr_probe'
    def limit():resource.setrlimit(resource.RLIMIT_AS,(18*1024**3,18*1024**3))
    for repeat in (0,1):
        with (out/f'raw-{repeat}.jsonl').open('x') as output:
            subprocess.run([str(binary),str(root/'artifacts/model-1.7B'),'6',str(req)],stdout=output,
                           check=True,timeout=120,preexec_fn=limit)
        responses=list(map(json.loads,(out/f'raw-{repeat}.jsonl').read_text().splitlines()))
        if len(responses)!=4 or {r['id'] for r in responses}!={r['id'] for r in rows}:raise ValueError('Missing legacy results')
        byid={r['id']:r for r in rows}
        enriched=[dict(r,reference=byid[r['id']]['reference'],fixture_sha256=byid[r['id']]['sha256'],repeat=repeat) for r in responses]
        (out/f'results-{repeat}.jsonl').write_text(''.join(json.dumps(r,ensure_ascii=False)+'\n' for r in enriched))
        if model_hashes(root)!=hashes:raise ValueError('Model/cache changed')
    print('Exact previous PCM16 signals preserved; float32 counterparts tested; two fresh processes; no concurrent ASR.')


if __name__=='__main__':main()
