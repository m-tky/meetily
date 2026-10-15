"""Same worker's diagnostic full/crop comparison on licensed short-word audio.
Only synthetic zero padding changes; the human source is never silently trimmed.
Results do not authorize an automatic full/crop fallback or word replacement.
"""
import argparse
import hashlib
import json
import pathlib
import time

from evaluate_robustness import RATE, pcm_metadata, read_pcm, write_pcm
from worker_client import Worker


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=pathlib.Path)
    parser.add_argument('--tag', required=True)
    args = parser.parse_args()
    if not args.tag or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-_' for c in args.tag):
        parser.error('Invalid tag')
    root = args.root.resolve()
    repo = pathlib.Path(__file__).resolve().parents[2]
    if root == repo or repo in root.parents:
        parser.error('Artifacts must remain outside Git')
    out = root / 'diagnostics' / args.tag
    out.mkdir(parents=True, exist_ok=False)
    source_dir = root / 'robustness/lexical-r1'
    source_manifest = json.loads((source_dir / 'manifest.json').read_text())
    inputs = []
    for r in source_manifest['rows']:
        if r['purpose'] == 'accuracy':
            path = source_dir / r['wav']
            if hashlib.sha256(path.read_bytes()).hexdigest() != r['sha256']:
                raise ValueError('Source hash mismatch')
            inputs.append(dict(r, path=str(path)))
    original = next(r for r in inputs if r['id'] == 'iie-clean')
    speech = read_pcm(pathlib.Path(original['path']))
    for where in ('before', 'after', 'both'):
        for ms in (100, 300, 1000):
            if where == 'both' and ms == 1000:
                continue  # Already present as iie-padded, no duplication.
            prefix = [0.0] * (ms * 16) if where in ('before', 'both') else []
            suffix = [0.0] * (ms * 16) if where in ('after', 'both') else []
            values = prefix + speech + suffix
            identifier = f'iie-{where}-{ms}ms'
            path = out / (identifier + '.wav')
            digest = write_pcm(path, values)
            inputs.append(dict(id=identifier, language='Japanese', reference='いいえ',
                               condition=f'{where}-{ms}ms', purpose='accuracy', path=str(path),
                               wav=path.name, sha256=digest, audio_seconds=len(values) / RATE,
                               synthetic_pre_samples=len(prefix), synthetic_post_samples=len(suffix),
                               **pcm_metadata(path)))
    rows = []
    for crop in (False, True):
        for source in inputs:
            rows.append(dict(source, id=source['id'] + ('-crop' if crop else '-full'),
                             source_id=source['id'], crop_context=crop))
    manifest = dict(dataset='Lingua Libre short-word context diagnostics',
                    source_manifest_sha256=hashlib.sha256((source_dir / 'manifest.json').read_bytes()).hexdigest(),
                    attribution=source_manifest['attribution'], license=source_manifest['license'],
                    license_url=source_manifest['license_url'], source_url=source_manifest['source_url'],
                    context_only_synthetic_zero_padding=True, worker_policy_changed=False, rows=rows)
    (out / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n')
    with (out / 'results.jsonl').open('x') as output:
        for crop in (False, True):
            with Worker(root / 'worker-target/release/qwen-speech-worker',
                        root / 'artifacts/model-1.7B', crop_context=crop) as worker:
                for index, row in enumerate(r for r in rows if r['crop_context'] == crop):
                    path = pathlib.Path(row['path'])
                    if hashlib.sha256(path.read_bytes()).hexdigest() != row['sha256']:
                        raise ValueError('Fixture hash mismatch')
                    tick = time.monotonic()
                    response = worker.request(dict(op='transcribe', id=f'context-{index}',
                                                   wav=str(path), language=row['language']))
                    status = response['status']
                    result = dict(id=row['id'], source_id=row['source_id'], language=row['language'],
                                  condition=row['condition'], reference=row['reference'], rep=0,
                                  audio_seconds=row['audio_seconds'], fixture_sha256=row['sha256'],
                                  text=response.get('text', ''), worker=response,
                                  elapsed_seconds=response.get('inference_seconds', 0),
                                  request_seconds=time.monotonic() - tick,
                                  status='ok' if status in ('recognized', 'review_required', 'no_speech') else 'error')
                    output.write(json.dumps(result, ensure_ascii=False) + '\n')
                    output.flush()
                    print(json.dumps(dict(id=row['id'], text=result['text'], status=status),
                                     ensure_ascii=False), flush=True)


if __name__ == '__main__':
    main()
