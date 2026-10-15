"""Repeat selected failures with fresh workers, without changing gate/model policy."""
import argparse
import hashlib
import json
import pathlib

from worker_client import Worker


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=pathlib.Path)
    parser.add_argument('--noise-tag', default='synthetic-r1')
    parser.add_argument('--short-tag', default='lexical-r1')
    parser.add_argument('--output-tag', required=True)
    args = parser.parse_args()
    for tag in (args.noise_tag, args.short_tag, args.output_tag):
        if not tag or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-_' for c in tag):
            parser.error('Invalid tag')
    root = args.root.resolve()
    repo = pathlib.Path(__file__).resolve().parents[2]
    if root == repo or repo in root.parents:
        parser.error('Artifacts must remain outside Git')
    selected = []
    for tag, ids in [(args.noise_tag, ['negative-bursts-0.002', 'negative-bursts-0.01',
                                     'fleurs-en_us-validation-0-1548-gain-40dB',
                                     'fleurs-en_us-validation-0-1548-white-snr0']),
                     (args.short_tag, ['iie-clean', 'iie-padded', 'iie-gain-40dB'])]:
        directory = root / 'robustness' / tag
        manifest = json.loads((directory / 'manifest.json').read_text())
        by_id = {r['id']: r for r in manifest['rows']}
        selected.extend((directory, by_id[identifier]) for identifier in ids)
    output_path = root / 'robustness' / (args.output_tag + '.jsonl')
    with output_path.open('x') as output:
        for repeat in range(2):
            with Worker(root / 'worker-target/release/qwen-speech-worker',
                        root / 'artifacts/model-1.7B') as worker:
                for index, (directory, row) in enumerate(selected):
                    path = directory / row['wav']
                    if hashlib.sha256(path.read_bytes()).hexdigest() != row['sha256']:
                        raise ValueError('Fixture hash mismatch')
                    response = worker.request(dict(op='transcribe', id=f'recheck-{repeat}-{index}',
                                                   wav=str(path), language=row['language']))
                    result = dict(fixture_id=row['id'], fixture_sha256=row['sha256'],
                                  source_tag=directory.name, repeat=repeat,
                                  reference=row['reference'], worker=response)
                    output.write(json.dumps(result, ensure_ascii=False) + '\n')
                    output.flush()
                    print(json.dumps(result, ensure_ascii=False), flush=True)


if __name__ == '__main__':
    main()
