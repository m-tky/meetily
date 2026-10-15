"""Same-v4 diagnostic gain/threshold/minimum ablation; no policy adoption.
Endpoint loop adapted from Silero v4 MIT rules (worker/licenses/Silero-MIT.txt).
Only window presence is measured, not annotated boundaries or speech recall.
Raw probability arrays stay outside Git; compact report includes their SHA256.
"""
import argparse
import hashlib
import json
import math
import pathlib
import subprocess
import struct


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def endpoints(probabilities, length, threshold=.5, minimum=1440):
    if not 0 < threshold < 1 or minimum < 0 or length <= 0:
        raise ValueError('Invalid diagnostic settings')
    if len(probabilities) != math.ceil(length / 512) or any(
            not math.isfinite(p) or not 0 <= p <= 1 for p in probabilities):
        raise ValueError('Invalid probabilities')
    positive = f32(threshold)
    negative = f32(threshold - .15)
    begin = None
    candidate_end = None
    raw = []
    for index, p in enumerate(probabilities):
        p = f32(p)
        current = index * 512
        if p >= positive:
            candidate_end = None
            if begin is None:
                begin = current
                continue
        if p < negative and begin is not None:
            if candidate_end is None:
                candidate_end = current
            if current - candidate_end < 8000:
                continue
            if candidate_end - begin > minimum:
                raw.append((begin, candidate_end))
            candidate_end = None
            begin = None
    if begin is not None and length - begin > minimum:
        raw.append((begin, length))
    merged = []
    for begin, end in raw:
        span = dict(start_sample=max(0, begin - 4800), end_sample=min(length, end + 3200))
        if merged and span['start_sample'] <= merged[-1]['end_sample']:
            merged[-1]['end_sample'] = max(merged[-1]['end_sample'], span['end_sample'])
        else:
            merged.append(span)
    return merged


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
    fixtures = []
    observed = {}
    for tag in ('synthetic-r1', 'lexical-r1'):
        directory = root / 'robustness' / tag
        manifest = json.loads((directory / 'manifest.json').read_text())
        fixtures.extend(dict(r, suite=tag, path=str(directory / r['wav'])) for r in manifest['rows'])
        for r in map(json.loads, (directory / 'results.jsonl').read_text().splitlines()):
            observed[(tag, r['id'])] = r['worker']['status'] != 'no_speech'
    directory = root / 'artifacts'
    manifest = json.loads((directory / 'manifest.json').read_text())
    fixtures.extend(dict(r, suite='original-30', path=str(directory / r['wav']))
                    for r in manifest['rows'] if r['purpose'] in ('accuracy', 'silence'))
    (out / 'fixtures.json').write_text(json.dumps(fixtures, ensure_ascii=False, indent=2) + '\n')
    for r in fixtures:
        if hashlib.sha256(pathlib.Path(r['path']).read_bytes()).hexdigest() != r['sha256']:
            raise ValueError('Fixture hash mismatch')
    binary = root / 'worker-target/release/examples/vad_gain_probe'
    binary_sha256 = hashlib.sha256(binary.read_bytes()).hexdigest()
    paths = list(dict.fromkeys(r['path'] for r in fixtures))
    with (out / 'probabilities.jsonl').open('x') as output:
        subprocess.run([str(binary), *paths], stdout=output, check=True, timeout=180)
    probabilities = {}
    for record in map(json.loads, (out / 'probabilities.jsonl').read_text().splitlines()):
        key = (record['file'], record['policy'])
        if key in probabilities:
            raise ValueError('Duplicate neural output')
        if endpoints(record['probabilities'], record['real_samples']) != record['default_intervals']:
            raise ValueError('Diagnostic default does not match Rust endpoint implementation')
        probabilities[key] = record
    if len(probabilities) != len(paths) * 3:
        raise ValueError('Incomplete neural outputs')
    details = []
    for fixture in fixtures:
        for policy in ('none', 'peak20', 'peak100'):
            record = probabilities[(fixture['path'], policy)]
            if abs(record['real_samples'] / 16000 - fixture['audio_seconds']) > .0001:
                raise ValueError('Duration mismatch')
            p = record['probabilities']
            for threshold in (.3, .5, .7, .9):
                for minimum_ms in (90, 200):
                    spans = endpoints(p, record['real_samples'], threshold, minimum_ms * 16)
                    detected = bool(spans)
                    if policy == 'peak20' and threshold == .5 and minimum_ms == 90:
                        old = observed.get((fixture['suite'], fixture['id']))
                        if old is not None and detected != old:
                            raise ValueError('Current gate does not reproduce recorded detection')
                    details.append(dict(suite=fixture['suite'], id=fixture['id'], language=fixture['language'],
                                        speech=fixture['purpose'] == 'accuracy', policy=policy,
                                        gain=record['gain'], threshold=threshold, minimum_ms=minimum_ms,
                                        max_probability=max(p), detected=detected, intervals=spans))
    groups = []
    for suite in ('synthetic-r1', 'lexical-r1', 'original-30'):
        for policy in ('none', 'peak20', 'peak100'):
            for threshold in (.3, .5, .7, .9):
                for minimum_ms in (90, 200):
                    selected = [r for r in details if (r['suite'], r['policy'], r['threshold'], r['minimum_ms'])
                                == (suite, policy, threshold, minimum_ms)]
                    groups.append(dict(suite=suite, policy=policy, threshold=threshold, minimum_ms=minimum_ms,
                                       speech_windows=sum(r['speech'] for r in selected),
                                       negative_windows=sum(not r['speech'] for r in selected),
                                       missed_speech=[r['id'] for r in selected if r['speech'] and not r['detected']],
                                       false_positive=[r['id'] for r in selected if not r['speech'] and r['detected']]))
    report = dict(binary_sha256=binary_sha256, frame_samples=512,
                  neural_model='same pinned Silero v4', worker_policy_changed=False,
                  quality_accepted=False, unannotated_window_presence_only=True,
                  negative_threshold='positive threshold minus .15', minimum_speech='strictly greater than minimum_ms',
                  input_windows=len(fixtures), unique_wavs=len(paths),
                  probabilities_sha256=hashlib.sha256((out / 'probabilities.jsonl').read_bytes()).hexdigest(),
                  groups=groups, details=details)
    (out / 'report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(dict(output=str(out), input_windows=len(fixtures), parity='all default endpoints match Rust',
                          policy_changed=False)))


if __name__ == '__main__':
    main()
