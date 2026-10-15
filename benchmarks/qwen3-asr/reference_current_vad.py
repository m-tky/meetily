"""Authorized step1: execute the unmodified pinned official Python/Torch wrapper.
Reference-only CPU environment; no ASR, capture, playback or model adoption.
Compare Rust on the SAME wheel runtime and separately the historical Nix runtime.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import subprocess

import numpy as np
import onnxruntime as ort
import torch

from compare_current_vad import sha
from diagnose_gate import endpoints
from evaluate_robustness import read_pcm
from prepare_current_vad import REVISION

ATOL = 1e-5


def load_official(root):
    directory = root / 'vad-models' / ('silero-' + REVISION)
    provenance = json.loads((directory / 'provenance.json').read_text())
    for file in provenance['files']:
        path = directory / file['file']
        data = path.read_bytes()
        if sha(path) != file['sha256'] or hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest() != file['git_blob_sha1']:
            raise ValueError('Pinned source/model changed')
    spec = importlib.util.spec_from_file_location('pinned_official_silero_utils', directory / 'utils_vad.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    model = module.OnnxWrapper(str(directory / 'silero_vad.onnx'), force_onnx_cpu=True)
    if model.session.get_providers() != ['CPUExecutionProvider']:
        raise ValueError('CPU-only reference required')
    return model, directory, provenance


def infer(model, samples):
    samples = np.asarray(samples, dtype=np.float32)
    if samples.ndim != 1 or not 0 < len(samples) <= 16000 * 30 or not np.isfinite(samples).all() or np.max(np.abs(samples)) > 1:
        raise ValueError('Invalid bounded float PCM')
    output = model.audio_forward(torch.from_numpy(samples.copy()), 16000)
    values = output.numpy().ravel().tolist()
    endpoints(values, len(samples))  # validates frame count/range/finiteness
    return values


def probability_map(records):
    result = {}
    for r in records:
        if r.get('event') == 'model_schema':
            continue
        key = (r['file'], r['policy'])
        if key in result:
            raise ValueError('Duplicate neural key')
        endpoints(r['probabilities'], r['real_samples'])
        result[key] = r
    return result


def compare(reference, other):
    if set(reference) != set(other):
        raise ValueError('Incomplete comparison')
    details = []
    for key, r in reference.items():
        candidate = other[key]
        if candidate['real_samples'] != r['real_samples'] or candidate['gain'] != r['gain']:
            raise ValueError('PCM/gain contract mismatch')
        a, b = np.asarray(r['probabilities']), np.asarray(candidate['probabilities'])
        if a.shape != b.shape:
            raise ValueError('Frame geometry mismatch')
        error = float(np.max(np.abs(a - b)))
        gates_equal = all(endpoints(a.tolist(), r['real_samples'], t) == endpoints(b.tolist(), r['real_samples'], t) for t in (.5, .7))
        details.append(dict(file=key[0], policy=key[1], frames=len(a), max_absolute_error=error,
                            bit_equal=bool(np.array_equal(a, b)), gate_intervals_equal=gates_equal,
                            within_tolerance=error <= ATOL))
    return dict(records=len(details), frames=sum(r['frames'] for r in details),
                absolute_tolerance=ATOL, max_absolute_error=max(r['max_absolute_error'] for r in details),
                bit_equal_records=sum(r['bit_equal'] for r in details),
                gate_mismatches=[r for r in details if not r['gate_intervals_equal']],
                probability_mismatches=[r for r in details if not r['within_tolerance']],
                passed=all(r['within_tolerance'] and r['gate_intervals_equal'] for r in details), details=details)


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
        parser.error('Runtime must remain outside Git')
    out = root / 'diagnostics' / args.tag
    out.mkdir(parents=True, exist_ok=False)
    (out / 'frozen-plan.json').write_text(json.dumps(dict(absolute_tolerance=ATOL,
        thresholds=[.5, .7], minimum_samples=1440, scope='official VAD wrapper parity only; no ASR'), indent=2) + '\n')
    torch.set_num_threads(1)
    model, directory, provenance = load_official(root)
    baseline = {}
    fixtures = []
    for tag, raw_name, manifest_name in [('current-vad-r2', 'probabilities.jsonl', 'fixtures.json'),
                                         ('current-vad-validation-r2', 'current-probabilities.jsonl', 'manifest.json')]:
        d = root / 'diagnostics' / tag
        if tag == 'current-vad-r2':
            manifest = json.loads((root / 'diagnostics/gate-r2/fixtures.json').read_text())
            digest = json.loads((d / 'report.json').read_text())['current_probabilities_sha256']
        else:
            manifest = json.loads((d / manifest_name).read_text())['rows']
            digest = json.loads((d / 'report.json').read_text())['raw_sha256']['current']
        if sha(d / raw_name) != digest:
            raise ValueError('Historical raw evidence changed')
        fixtures.extend(manifest)
        mapped = probability_map(map(json.loads, (d / raw_name).read_text().splitlines()))
        for key, record in mapped.items():
            if key in baseline and baseline[key] != record:
                raise ValueError('Conflicting baseline')
            baseline[key] = record
    for row in fixtures:
        if sha(pathlib.Path(row['path'])) != row['sha256']:
            raise ValueError('Public fixture changed')
    binary = root / 'worker-target/release/examples/current_vad_probe'
    old_report = json.loads((root / 'diagnostics/current-vad-r2/report.json').read_text())
    if sha(binary) != old_report['probe_binary_sha256']:
        raise ValueError('Original Rust probe changed')
    paths = list(dict.fromkeys(r['path'] for r in fixtures))
    libraries = list((pathlib.Path(ort.__file__).parent / 'capi').glob('libonnxruntime.so.*'))
    if len(libraries) != 1:
        raise ValueError('Ambiguous wheel runtime')
    env = dict(os.environ, ORT_DYLIB_PATH=str(libraries[0]))
    with (out / 'rust-wheel.jsonl').open('x') as output:
        subprocess.run([str(binary), str(directory / 'silero_vad.onnx'), *paths],
                       stdout=output, env=env, check=True, timeout=300)
    rust_wheel = probability_map(map(json.loads, (out / 'rust-wheel.jsonl').read_text().splitlines()))
    reference = {}
    with (out / 'official-python.jsonl').open('x') as output:
        for path in paths:
            samples = np.asarray(read_pcm(pathlib.Path(path)), dtype=np.float32)
            for policy in ('none', 'peak20', 'peak100'):
                old = baseline[(path, policy)]
                values = samples * np.float32(old['gain'])
                record = dict(file=path, policy=policy, gain=old['gain'], real_samples=len(samples),
                              probabilities=infer(model, values))
                output.write(json.dumps(record) + '\n')
                output.flush()
                reference[(path, policy)] = record
    same = compare(reference, rust_wheel)
    historical = compare(reference, baseline)
    report = dict(official_source_unmodified=True, actual_torch_used=True, fake_torch=False,
                  model_provenance=provenance, numpy_version=np.__version__, torch_version=torch.__version__,
                  python_ort_version=ort.__version__, torch_cuda=torch.version.cuda,
                  providers=model.session.get_providers(), wheel_ort_sha256=sha(libraries[0]),
                  wheel_ort_path=str(libraries[0]), nix_ort_path=old_report['ort_dylib_path'],
                  nix_ort_sha256=sha(pathlib.Path(old_report['ort_dylib_path'])),
                  rust_binary_sha256=sha(binary), request_windows=len(fixtures), unique_wavs=len(paths),
                  same_wheel_runtime=same, historical_nix_runtime=historical,
                  raw_sha256={n: sha(out / n) for n in ('rust-wheel.jsonl', 'official-python.jsonl')},
                  passed=same['passed'] and historical['passed'], asr_calls=0, policy_adopted=False)
    (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({k: v for k, v in report.items() if k not in ('same_wheel_runtime', 'historical_nix_runtime', 'model_provenance')}, indent=2))
    for label, result in [('same-wheel', same), ('historical-Nix', historical)]:
        print(label, {k: v for k, v in result.items() if k != 'details'})
    if not report['passed']:
        raise SystemExit('Official wrapper parity failed; do not proceed to mitigation adoption')


if __name__ == '__main__':
    main()
