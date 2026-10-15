"""Approved same-fixture comparison: pinned current official ONNX vs embedded v4.
Neural wrapper follows the current 16k contract; endpoints use SAME worker rules.
This is window-presence evidence, not app integration or aligned speech recall.
"""
import argparse
import hashlib
import json
import os
import pathlib
import subprocess

from diagnose_gate import endpoints
from prepare_current_vad import REVISION


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


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
    model_dir = root / 'vad-models' / ('silero-' + REVISION)
    provenance = json.loads((model_dir / 'provenance.json').read_text())
    for file in provenance['files']:
        if sha(model_dir / file['file']) != file['sha256']:
            raise ValueError('Pinned model/source hash changed')
    fixtures = json.loads((root / 'diagnostics/gate-r2/fixtures.json').read_text())
    for fixture in fixtures:
        if sha(pathlib.Path(fixture['path'])) != fixture['sha256']:
            raise ValueError('Fixture changed')
    v4_path = root / 'diagnostics/gate-r2/probabilities.jsonl'
    v4_report = json.loads((root / 'diagnostics/gate-r2/report.json').read_text())
    if sha(v4_path) != v4_report['probabilities_sha256']:
        raise ValueError('V4 reference changed')
    v4 = {(r['file'], r['policy']): r for r in map(json.loads, v4_path.read_text().splitlines())}
    paths = list(dict.fromkeys(r['path'] for r in fixtures))
    binary = root / 'worker-target/release/examples/current_vad_probe'
    binary_sha256 = sha(binary)
    with (out / 'probabilities.jsonl').open('x') as output:
        subprocess.run([str(binary), str(model_dir / 'silero_vad.onnx'), *paths],
                       stdout=output, timeout=180, check=True)
    records = list(map(json.loads, (out / 'probabilities.jsonl').read_text().splitlines()))
    if records[0].get('event') != 'model_schema':
        raise ValueError('Missing schema validation')
    current = {}
    for record in records[1:]:
        key = (record['file'], record['policy'])
        if key in current or record['new_frame_samples'] != 512 or record['context_samples'] != 64:
            raise ValueError('Duplicate/invalid current VAD result')
        if not record['fresh_state_per_input']:
            raise ValueError('State carry forbidden')
        current[key] = record
    if len(current) != len(paths) * 3:
        raise ValueError('Incomplete current VAD output')
    details = []
    groups = []
    for model_name, neural in [('v4', v4), ('current-pinned', current)]:
        for fixture in fixtures:
            for policy in ('none', 'peak20', 'peak100'):
                record = neural[(fixture['path'], policy)]
                if record['real_samples'] != round(fixture['audio_seconds'] * 16000):
                    raise ValueError('Neural input length mismatch')
                probs = record['probabilities']
                intervals = endpoints(probs, record['real_samples'])
                details.append(dict(model=model_name, suite=fixture['suite'], id=fixture['id'],
                                    language=fixture['language'], speech=fixture['purpose'] == 'accuracy',
                                    gain_policy=policy, gain=record['gain'],
                                    detected=bool(intervals), max_probability=max(probs), intervals=intervals))
        for suite in ('synthetic-r1', 'lexical-r1', 'original-30'):
            for policy in ('none', 'peak20', 'peak100'):
                selected = [r for r in details if (r['model'], r['suite'], r['gain_policy'])
                            == (model_name, suite, policy)]
                groups.append(dict(model=model_name, suite=suite, gain_policy=policy,
                                   speech_windows=sum(r['speech'] for r in selected),
                                   negative_windows=sum(not r['speech'] for r in selected),
                                   missed_speech=[r['id'] for r in selected if r['speech'] and not r['detected']],
                                   false_positive=[r['id'] for r in selected if not r['speech'] and r['detected']]))
    grid = []
    for model_name, neural in [('v4', v4), ('current-pinned', current)]:
        for policy in ('none', 'peak20', 'peak100'):
            for threshold in (.3, .5, .7, .9):
                for minimum_ms in (90, 200):
                    missed, false_positive = [], []
                    for fixture in fixtures:
                        record = neural[(fixture['path'], policy)]
                        detected = bool(endpoints(record['probabilities'], record['real_samples'],
                                                  threshold, minimum_ms * 16))
                        identifier = fixture['suite'] + ':' + fixture['id']
                        if fixture['purpose'] == 'accuracy' and not detected:
                            missed.append(identifier)
                        elif fixture['purpose'] == 'silence' and detected:
                            false_positive.append(identifier)
                    grid.append(dict(model=model_name, gain_policy=policy, threshold=threshold,
                                     minimum_ms=minimum_ms, missed_speech=missed, false_positive=false_positive))
    report = dict(official_repository_revision=REVISION, package_version='6.2.3 (pinned pyproject)',
                  model_provenance=provenance, probe_binary_sha256=binary_sha256,
                  user_approved_comparison=True, worker_policy_changed=False,
                  new_model_adopted=False, quality_accepted=False, asr_rerun=False,
                  endpoint_settings='Same worker: pos .5 / neg .35, >90ms speech, 500ms silent hold, pre300/post200/merge',
                  frame_contract='current: 512 new +64 context, state[2,1,128], scalar i64 sr=16000; v4:512,h/c[2,1,64]',
                  onnx_schema=records[0], current_probabilities_sha256=sha(out / 'probabilities.jsonl'),
                  v4_probabilities_sha256=sha(v4_path),
                  ort_dylib_path=os.environ['ORT_DYLIB_PATH'],
                  ort_dylib_sha256=sha(pathlib.Path(os.environ['ORT_DYLIB_PATH'])),
                  groups=groups, details=details, parameter_grid=grid,
                  grid_is_exploratory_not_held_out=True)
    (out / 'report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(dict(output=str(out), groups=groups), ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
