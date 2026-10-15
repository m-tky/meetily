"""Known-public-speech ASR/gate isolation. Never a worker fallback policy.
Quiet PCM is also restored100x diagnostically; quantization cannot be undone.
No new models/dependencies, no real recording or language-auto changes.
"""
import argparse
import hashlib
import json
import pathlib
import resource
import subprocess

from evaluate_robustness import pcm_metadata, read_pcm, write_pcm


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
    sources = []
    for tag, ids in [('synthetic-r1', ['fleurs-en_us-validation-0-1548-clean',
                                     'fleurs-en_us-validation-0-1548-gain-20dB',
                                     'fleurs-en_us-validation-0-1548-gain-40dB',
                                     'fleurs-en_us-validation-0-1548-white-snr0']),
                     ('lexical-r1', ['iie-clean', 'iie-padded', 'iie-gain-40dB'])]:
        directory = root / 'robustness' / tag
        manifest = json.loads((directory / 'manifest.json').read_text())
        byid = {r['id']: r for r in manifest['rows']}
        for identifier in ids:
            source = byid[identifier]
            path = directory / source['wav']
            if hashlib.sha256(path.read_bytes()).hexdigest() != source['sha256']:
                raise ValueError('Fixture hash mismatch')
            sources.append(dict(source, path=str(path), source_tag=tag))
            if source.get('gain_db') == -40:
                # Same hashed quantized PCM, NOT original losslessly restored.
                values = [x * 100 for x in read_pcm(path)]
                restored = out / (identifier + '-restored100.wav')
                digest = write_pcm(restored, values)
                sources.append(dict(source, id=identifier + '-restored100',
                                    condition=source['condition'] + '-restored100',
                                    path=str(restored), wav=restored.name, sha256=digest,
                                    source_tag=tag, input_gain=100, **pcm_metadata(restored)))
    tsv = out / 'inputs.tsv'
    tsv.write_text(''.join(f"{r['id']}\t{r['language']}\t{r['path']}\n" for r in sources))
    manifest = dict(diagnostic_gate_bypass=True, worker_policy_changed=False,
                    source_attribution='FLEURS: Conneau et al. (2022), CC-BY-4.0; Lingua Libre: Zsrtrgh (2025-11-03), CC-BY-SA-4.0',
                    quiet_restoration='100x gain on quantized PCM16; not lossless', rows=sources)
    (out / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n')

    def limit():
        resource.setrlimit(resource.RLIMIT_AS, (18 * 1024**3, 18 * 1024**3))

    binary = root / 'worker-target/release/examples/ungated_speech_probe'
    with (out / 'raw.jsonl').open('x') as output:
        subprocess.run([str(binary), str(root / 'artifacts/model-1.7B'), '6', str(tsv)],
                       stdout=output, check=True, timeout=240, preexec_fn=limit)
    byid = {r['id']: r for r in sources}
    results = []
    seen = set()
    for row in map(json.loads, (out / 'raw.jsonl').read_text().splitlines()):
        key = (row['id'], row['repeat'])
        if key in seen or row['id'] not in byid or row['repeat'] not in (0, 1):
            raise ValueError('Duplicate/unknown diagnostic response')
        seen.add(key)
        source = byid[row['id']]
        if row['input_samples'] != round(source['audio_seconds'] * 16000):
            raise ValueError('Diagnostic input length mismatch')
        results.append(dict(row, reference=source['reference'], fixture_sha256=source['sha256']))
    if len(results) != len(sources) * 2:
        raise ValueError('Incomplete diagnostic responses')
    (out / 'results.jsonl').write_text(''.join(json.dumps(r, ensure_ascii=False) + '\n' for r in results))
    for row in results:
        print(json.dumps(dict(id=row['id'], repeat=row['repeat'], text=row['text']), ensure_ascii=False))


if __name__ == '__main__':
    main()
