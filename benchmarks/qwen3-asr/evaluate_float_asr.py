"""Approved same1.7B INT8 PCM16-vs-float32 ASR diagnosis; no app/default changes.
Known speech is decoded even if gate misses, to separate two causes. Negatives
are never ungated. Gated text scores are explicitly PROJECTIONS of this diagnosis.
"""
import argparse
import hashlib
import json
import pathlib
import resource
import subprocess

import numpy as np
import torch

from compare_vad_gain import source_rows, quantize, transform
from diagnose_gate import endpoints
from evaluate_robustness import RATE, mix, noise, read_pcm
from reference_current_vad import load_official, infer


def hash_file(path):
    h = hashlib.sha256()
    with pathlib.Path(path).open('rb') as source:
        for chunk in iter(lambda: source.read(8 * 1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()


def plan():
    return dict(asr_model='same1.7B decoder-INT8 encoder/prefillF32', threads=6,
                gate='pinned-current .7/.55/>90ms, VAD-only peak100', asr_input_gain=1,
                context='full original input', known_speech_diagnostic_bypass=True,
                negative_bypass=False, baseline=33, quiet_pairs=66, short_extra_pairs=12,
                sentence_noise_pairs=6, negative_windows=65, total_windows=182,
                retuning=False, new_source_holdout=False, correction=False,
                model_replacement=False, application_integration=False)


def model_hashes(root):
    directory = root / 'artifacts/model-1.7B'
    provenance = json.loads((directory / 'provenance.json').read_text())
    for file in provenance['files']:
        if hash_file(directory / file['file']) != file['sha256']:
            raise ValueError('Official ASR source model changed')
    return {p.name: hash_file(p) for p in sorted(directory.iterdir()) if p.is_file()}


def prepare(root, out):
    torch.set_num_threads(1)
    vad, _, vad_provenance = load_official(root)
    sources = source_rows(root)
    rows = []
    def add(identifier, values, source, suite, representation, condition, pair_id=None, **extra):
        values = np.asarray(values, dtype=np.float32)
        if not 0 < len(values) <= 30 * RATE or not np.isfinite(values).all() or np.max(np.abs(values)) > 1:
            raise ValueError('Invalid prepared bounded float PCM')
        path = out / 'inputs' / (identifier + '.f32le')
        path.parent.mkdir(exist_ok=True)
        path.write_bytes(values.astype('<f4').tobytes())
        vad_values, gains = transform(values, 'peak100')
        probabilities = infer(vad, vad_values)
        spans = endpoints(probabilities, len(values), .7)
        row = dict(id=identifier, path=str(path), sha256=hash_file(path), samples=len(values),
                   audio_seconds=len(values)/RATE, reference=source['reference'], language=source['language'],
                   source_path=source['path'], source_sha256=source['sha256'], suite=suite,
                   representation=representation, condition=condition, pair_id=pair_id,
                   role='speech' if source['reference'] else 'negative', gate_detected=bool(spans),
                   gate_intervals=spans, gate_speech_samples=sum(s['end_sample']-s['start_sample'] for s in spans),
                   vad_gain=gains[0], max_probability=max(probabilities),
                   input_peak=float(np.max(np.abs(values))), **extra)
        rows.append(row)
    for source in sources:
        if hash_file(source['path']) != source['sha256']:
            raise ValueError('Source changed')
        original = np.asarray(read_pcm(pathlib.Path(source['path'])), dtype=np.float64)
        add(source['id']+'-baseline', original, source, 'baseline', 'original_pcm16', 'clean')
        quiet = original * .01
        for fmt, values in [('float32', quiet.astype(np.float32)), ('pcm16', quantize(quiet))]:
            add(source['id']+'-quiet-'+fmt, values, source, 'quiet', fmt, 'gain-40dB', pair_id=source['id']+'-quiet')
    for index, source in enumerate(sources[-3:]):
        original = np.asarray(read_pcm(pathlib.Path(source['path'])), dtype=np.float64)
        variants = [('padded', np.concatenate([np.zeros(RATE), original, np.zeros(RATE)]), {}),
                    ('white-snr0', *mix(original.tolist(), noise('white', len(original), 5122026), 0))]
        for variant in variants:
            condition, values = variant[:2]
            extra = dict(common_gain=variant[2]) if condition=='white-snr0' else {}
            for fmt, input_values in [('float32', np.asarray(values, dtype=np.float32)), ('pcm16', quantize(values))]:
                add(source['id']+'-'+condition+'-'+fmt, input_values, source, 'short-extra', fmt,
                    condition, pair_id=source['id']+'-'+condition, **extra)
    for language in ('Japanese', 'English', 'Thai'):
        source = next(s for s in sources if s['language']==language)
        original = read_pcm(pathlib.Path(source['path']))
        values, common_gain = mix(original, noise('white', len(original), 5122026), 0)
        for fmt, input_values in [('float32', np.asarray(values,dtype=np.float32)), ('pcm16', quantize(values))]:
            add(source['id']+'-noise-'+fmt, input_values, source, 'sentence-noise', fmt, 'white-snr0',
                pair_id=source['id']+'-noise', common_gain=common_gain)
    old = json.loads((root / 'diagnostics/vad-gain-r1/manifest.json').read_text())
    negatives = [r for r in old['rows'] if r['suite'] in ('known', 'new-transforms') and not r['speech']]
    if len(negatives)!=65: raise ValueError('All65 previous negatives must remain included')
    for index, row in enumerate(negatives):
        if hash_file(row['path'])!=row['sha256']:raise ValueError('Negative fixture changed')
        values=np.frombuffer(pathlib.Path(row['path']).read_bytes(),dtype='<f4')
        source=dict(row,path=row['path'],sha256=row['sha256'])
        add('negative-'+str(index),values,source,'negative','existing_float_transport','negative',original_id=row['id'])
    manifest=dict(plan=plan(),model_hashes=model_hashes(root),vad_provenance=vad_provenance,
                  original_sources_already_pcm16=True,source_holdout=False,
                  snr_definition='whole-clip RMS including pauses before additional PCM16',rows=rows)
    validate(manifest)
    (out/'manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n')
    # Negatives before speech and again after speech (last negative is silence).
    ordered=[r for r in rows if r['role']=='negative'][:-1]+[r for r in rows if r['role']=='speech']+[rows[-1]]
    allowed=('id','language','path','samples','role','gate_detected','gate_speech_samples')
    (out/'requests.jsonl').write_text(''.join(json.dumps({k:r[k] for k in allowed},ensure_ascii=False)+'\n' for r in ordered))


def validate(manifest):
    from collections import Counter
    rows=manifest['rows']
    if Counter(r['suite'] for r in rows)!=Counter({'baseline':33,'quiet':66,'short-extra':12,'sentence-noise':6,'negative':65}):
        raise ValueError('Incomplete study')
    if len({r['id'] for r in rows})!=182 or len({r['path'] for r in rows})!=182:
        raise ValueError('Duplicate inputs')
    for r in rows:
        if r['role']!=('speech' if r['reference'] else 'negative') or not 0<r['samples']<=30*RATE:
            raise ValueError('Invalid source labels/bounds')
    paired=[r for r in rows if r['pair_id']]
    if len({r['pair_id'] for r in paired})!=42 or any(
        Counter(r['representation'] for r in paired if r['pair_id']==pair)!=Counter({'pcm16':1,'float32':1})
        for pair in {r['pair_id'] for r in paired}):raise ValueError('Incomplete representation pair')


def run(root,out):
    manifest=json.loads((out/'manifest.json').read_text());validate(manifest)
    if manifest['plan']!=plan():raise ValueError('Frozen plan changed')
    for row in manifest['rows']:
        if hash_file(row['path'])!=row['sha256']:raise ValueError('Input changed')
    if model_hashes(root)!=manifest['model_hashes']:raise ValueError('Model/cache changed')
    binary=root/'worker-target/release/examples/float_asr_probe'
    binary_hash=hash_file(binary)
    def limit():resource.setrlimit(resource.RLIMIT_AS,(18*1024**3,18*1024**3))
    for repeat in (0,1):
        raw=out/('raw-'+str(repeat)+'.jsonl')
        with raw.open('x') as output:
            subprocess.run([str(binary),str(root/'artifacts/model-1.7B'),'6',str(out/'requests.jsonl')],
                           stdout=output,check=True,timeout=1200,preexec_fn=limit)
        responses=list(map(json.loads,raw.read_text().splitlines()))
        byid={r['id']:r for r in manifest['rows']}
        if len(responses)!=182 or len({r['id'] for r in responses})!=182:raise ValueError('Incomplete responses')
        results=[]
        for r in responses:
            source=byid[r['id']]
            expected=int(source['role']=='speech' or source['gate_detected'])
            if r['asr_calls']!=expected or r['input_samples']!=source['samples'] or r['confirmed']:
                raise ValueError('Response policy/length mismatch')
            results.append(dict(r,reference=source['reference'],suite=source['suite'],
                                representation=source['representation'],pair_id=source['pair_id'],
                                fixture_sha256=source['sha256'],repeat=repeat,
                                gated_projection_text=r['text'] if source['gate_detected'] else '',
                                gated_projection_not_second_actual_call=True))
        (out/('results-'+str(repeat)+'.jsonl')).write_text(''.join(json.dumps(r,ensure_ascii=False)+'\n' for r in results))
        if model_hashes(root)!=manifest['model_hashes']:raise ValueError('Model/cache mutated during inference')
    (out/'run-provenance.json').write_text(json.dumps(dict(binary_sha256=binary_hash,
        manifest_sha256=hash_file(out/'manifest.json'),requests_sha256=hash_file(out/'requests.jsonl'),
        raw_sha256={str(i):hash_file(out/('raw-'+str(i)+'.jsonl')) for i in (0,1)},
        fresh_processes=2,worker_policy_changed=False,model_replaced=False,correction_implemented=False),indent=2)+'\n')
    print('Completed182requests per fresh process; actual117speech ASR calls plus gated negatives. No default/app replacement.')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root',type=pathlib.Path);parser.add_argument('--tag',required=True)
    parser.add_argument('--prepare-only',action='store_true');parser.add_argument('--run-only',action='store_true')
    args=parser.parse_args()
    if args.prepare_only and args.run_only:parser.error('Choose one phase')
    if not args.tag or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-_' for c in args.tag):parser.error('Invalid tag')
    root=args.root.resolve();repo=pathlib.Path(__file__).resolve().parents[2]
    if root==repo or repo in root.parents:parser.error('Artifacts must stay outside Git')
    out=root/'diagnostics'/args.tag
    if not args.run_only:
        out.mkdir(parents=True,exist_ok=False)
        (out/'frozen-plan.json').write_text(json.dumps(plan(),indent=2)+'\n')
        prepare(root,out)
    if not args.prepare_only:run(root,out)


if __name__=='__main__':main()
