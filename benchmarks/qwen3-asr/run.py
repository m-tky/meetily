"""Run independent CPU processes sequentially, never alter Meetily or capture audio."""
import argparse
import hashlib
import json
import os
import pathlib
import resource
import subprocess
import sys
import time

p = argparse.ArgumentParser()
p.add_argument('root', type=pathlib.Path)
p.add_argument('--engine', choices=['c', 'candle', 'native', 'crisp'], required=True)
p.add_argument('--size', choices=['0.6B','1.7B'], required=True)
p.add_argument('--threads', type=int, default=6)
p.add_argument('--suite', choices=['pilot','accuracy','latency','stream','paced','reconcile'], required=True)
p.add_argument('--repeats', type=int, default=1)
p.add_argument('--timeout', type=int, default=1800)
p.add_argument('--omp-wait-policy',choices=['ACTIVE','PASSIVE'])
p.add_argument('--cpu-set', help='Optional comma-separated logical CPUs; applies only to this benchmark child')
a=p.parse_args()
root=a.root.resolve()
artifacts=root/'artifacts'
manifest=json.loads((artifacts/'manifest.json').read_text())
model=artifacts/(f'q4-{a.size.lower()}' if a.engine=='crisp' else f'model-{a.size}')
provenance=json.loads((model/'provenance.json').read_text())
# Downloader verified whole-file SHA256; this controller rechecks sizes only.
files=provenance.get('files',[{'file':provenance.get('filename'),'bytes':provenance.get('bytes')}])
for file in files:
    path=model/file['file']
    if path.stat().st_size!=file['bytes']:
        raise RuntimeError('Model size mismatch')
# Audio hash check is cheap and excludes conversion/download from timing.
for row in manifest['rows']:
    if hashlib.sha256((artifacts/row['wav']).read_bytes()).hexdigest()!=row['sha256']:
        raise RuntimeError('Fixture hash mismatch')
if a.suite=='pilot':
    rows=[next(r for r in manifest['rows'] if r['language']==lang and r['purpose']=='accuracy') for lang in ['Japanese','English','Thai']]
elif a.suite=='latency':
    rows=[r for r in manifest['rows'] if r['purpose']=='latency-prefix']
elif a.suite in ('stream','paced','reconcile'):
    rows=[next(r for r in manifest['rows'] if r['language']==lang and r['purpose']=='accuracy') for lang in ['Japanese','English','Thai']]
else:
    rows=[r for r in manifest['rows'] if r['purpose'] in ('accuracy','silence')]
cpus=None
if a.cpu_set:
    cpus={int(v) for v in a.cpu_set.split(',')}
    if not cpus or not cpus.issubset(os.sched_getaffinity(0)):raise RuntimeError('Invalid CPU affinity')
label=f'{a.engine}-{a.size}-t{a.threads}-{a.suite}-r{a.repeats}'
if cpus:label+='-cpu'+ '_'.join(map(str,sorted(cpus)))
if a.omp_wait_policy:label+='-omp-'+a.omp_wait_policy.lower()
out=root/'results';out.mkdir(exist_ok=True)
inputs=out/f'{label}.tsv'
inputs.write_text(''.join(f"{r['id']}\t{r['language']}\t{artifacts/r['wav']}\n" for r in rows))
mode='paced-reconcile' if a.suite=='reconcile' else a.suite if a.suite in ('stream','paced') else 'offline'
if a.engine=='c':
    command=[str(root/'c-runner'),str(model),str(a.threads),mode,str(inputs),str(a.repeats)]
elif a.engine=='crisp':
    if mode not in ('offline','paced','paced-reconcile'):raise RuntimeError('Crisp wrapper exposes offline/paced API only')
    command=[str(root/'crisp-target/qwen-crisp-bench'),str(model/provenance['filename']),str(a.threads),mode,str(inputs),str(a.repeats)]
elif a.engine=='native':
    command=[str(root/'native-lp64-target/release/qwen-native-bench'),str(model),str(a.threads),mode,str(inputs),str(a.repeats)]
else:
    if mode!='offline':raise RuntimeError('Candle streaming not selected for this wrapper')
    command=[str(root/'candle-target/release/qwen-candle-bench'),str(model),str(a.threads),str(inputs),str(a.repeats)]
env=os.environ.copy()
env.update(OPENBLAS_NUM_THREADS=str(a.threads),OMP_NUM_THREADS=str(a.threads),MKL_NUM_THREADS=str(a.threads),RAYON_NUM_THREADS=str(a.threads))
if a.omp_wait_policy:
    env['OMP_WAIT_POLICY']=a.omp_wait_policy
    if a.omp_wait_policy=='PASSIVE':env['GOMP_SPINCOUNT']='0'
def limit_address_space():
    # Safety guard on this no-swap desktop. Never run two model processes.
    resource.setrlimit(resource.RLIMIT_AS, (18 * 1024**3, 18 * 1024**3))
    if cpus:os.sched_setaffinity(0,cpus)
started=time.time()
with (out/f'{label}.jsonl').open('w') as stdout, (out/f'{label}.stderr').open('w') as stderr:
    try:
        completed=subprocess.run(command,stdout=stdout,stderr=stderr,env=env,timeout=a.timeout,check=False,preexec_fn=limit_address_space)
        exit_code=completed.returncode
    except subprocess.TimeoutExpired:
        exit_code='timeout'
metadata={'label':label,'model_revision':provenance['revision'],'command':command,'start_unix':started,
    'end_unix':time.time(),'exit_code':exit_code,'fixture_ids':[r['id'] for r in rows],
    'address_space_cap_gib':18,'cpu_affinity':sorted(cpus) if cpus else None,'omp_wait_policy':a.omp_wait_policy,
    'note':'No parallel ASR process. CPU desktop not exclusively reserved. Model sizes rechecked; downloader verified publisher SHA256 (Crisp Q4 is a third-party conversion).'}
(out/f'{label}.run.json').write_text(json.dumps(metadata,indent=2))
print(json.dumps({'label':label,'exit_code':exit_code,'seconds':time.time()-started}),flush=True)
if exit_code!=0:sys.exit(1)
