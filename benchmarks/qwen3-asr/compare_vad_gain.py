"""Authorized step2: frozen VAD-only gain comparison after official wrapper parity.
Same positive .7 / negative .55 / minimum >90ms, no threshold search or ASR.
Keep all previous failures; add predeclared unseen transformations and paired
float32-vs-additional-PCM16 attenuation (sources themselves are already PCM16).
"""
import argparse
from collections import Counter
import json
import math
import pathlib

import numpy as np
import torch

from compare_current_vad import sha
from diagnose_gate import endpoints
from evaluate_robustness import RATE, mix, noise, read_pcm, rms
from reference_current_vad import infer, load_official
from validate_current_vad import fresh_tones

POLICIES = ('none', 'peak20', 'peak100', 'peak300', 'slow-rms100')
SEED = 9182029


def freeze_plan():
    return dict(policies=POLICIES, positive=.7, negative=.55, minimum_samples_strictly_greater=1440,
                peak_target=.1, slow_rms_target=.02, slow_gain_cap=100,
                slow_gain_rise_seconds=1.0, slow_gain_fall_seconds=.05,
                slow_peak_guard=.95, slow_initial_gain=1, slow_frame_samples=512,
                hard_energy_rejection=False, seed=SEED, retuning=False,
                known_windows=343, new_transform_windows=100,
                quantization_pair_sources=33, attenuation_db=[-40, -60],
                source_holdout=False, asr_calls=0, model_adopted=False,
                correction_implemented=False, cloud_inference=False)


def quantize(values):
    """Same nearest-even PCM16 quantization as previous write_pcm; no file overwrite."""
    values = np.asarray(values, dtype=np.float64)
    return (np.clip(np.rint(values * 32768), -32768, 32767) / 32768).astype(np.float32)


def transform(values, policy):
    values = np.asarray(values, dtype=np.float32)
    if values.ndim != 1 or not len(values) or not np.isfinite(values).all() or np.max(np.abs(values)) > 1:
        raise ValueError('Invalid float PCM')
    if policy not in POLICIES:
        raise ValueError('Unknown frozen policy')
    frames = math.ceil(len(values) / 512)
    if policy != 'slow-rms100':
        peak = np.max(np.abs(values))
        cap = {'none': 1, 'peak20': 20, 'peak100': 100, 'peak300': 300}[policy]
        gain = np.float32(1 if peak == 0 else np.clip(np.float32(.1) / peak, 1, cap))
        return values * gain, [float(gain)] * frames
    output = np.empty_like(values)
    gain, gains = 1.0, []
    for start in range(0, len(values), 512):
        chunk = values[start:start + 512]
        energy = float(np.sqrt(np.mean(chunk.astype(np.float64) ** 2)))
        peak = float(np.max(np.abs(chunk)))
        desired = min(100.0, max(1.0, .02 / energy)) if energy > 0 else 1.0
        tau = 1.0 if desired > gain else .05
        gain += (desired - gain) * (1 - math.exp(-.032 / tau))
        # Limit present-frame gain, don't drop a low-energy frame or clip it.
        gain = min(gain, max(1.0, .95 / peak)) if peak else gain
        g = np.float32(gain)
        output[start:start + len(chunk)] = chunk * g
        gains.append(float(g))
    if np.max(np.abs(output)) > 1:
        raise ValueError('Unexpected clipping')
    return output, gains


def source_rows(root):
    original = json.loads((root / 'artifacts/manifest.json').read_text())
    sources = [dict(r, path=str(root / 'artifacts' / r['wav'])) for r in original['rows'] if r['purpose'] == 'accuracy']
    old_lexical = json.loads((root / 'robustness/lexical-r1/manifest.json').read_text())
    new_lexical = json.loads((root / 'diagnostics/current-vad-validation-r2/manifest.json').read_text())
    expected = dict(iie=next(r['sha256'] for r in old_lexical['rows'] if r['id'] == 'iie-clean'),
                    kuruma=new_lexical['source_records'][0]['converted_sha256'],
                    hon=new_lexical['source_records'][1]['converted_sha256'])
    for tag, file, label in [('lexical-r1', 'iie-clean.wav', 'iie'),
                              ('../diagnostics/current-vad-validation-r2', 'lexical-converted-0.wav', 'kuruma'),
                              ('../diagnostics/current-vad-validation-r2', 'lexical-converted-1.wav', 'hon')]:
        path = (root / 'robustness' / tag / file).resolve()
        if sha(path) != expected[label]:
            raise ValueError('Published lexical source changed')
        sources.append(dict(id=label, path=str(path), sha256=expected[label], language='Japanese',
                            reference={'iie': 'いいえ', 'kuruma': '車', 'hon': '本'}[label], purpose='accuracy'))
    if len(sources) != 33:
        raise ValueError('Expected30 FLEURS +3 lexical sources')
    return sources


def prepare(root, out):
    rows = []
    def add(identifier, values, source, suite, **extra):
        values = np.asarray(values, dtype=np.float32)
        if not 0 < len(values) <= RATE * 30 or not np.isfinite(values).all() or np.max(np.abs(values)) > 1:
            raise ValueError('Invalid bounded prepared waveform')
        path = out / 'float-pcm' / (identifier + '.f32le')
        path.parent.mkdir(exist_ok=True)
        path.write_bytes(values.astype('<f4').tobytes())
        rows.append(dict(id=identifier, path=str(path), sha256=sha(path), samples=len(values),
                         suite=suite, language=source['language'], reference=source['reference'],
                         speech=bool(source['reference']), source_sha256=source['sha256'],
                         source_path=source['path'], pcm_peak=float(np.max(np.abs(values))),
                         pcm_rms=float(np.sqrt(np.mean(values.astype(np.float64) ** 2))), **extra))
    fixtures = json.loads((root / 'diagnostics/gate-r2/fixtures.json').read_text())
    fixtures += json.loads((root / 'diagnostics/current-vad-validation-r2/manifest.json').read_text())['rows']
    for index, r in enumerate(fixtures):
        path = pathlib.Path(r['path'])
        if sha(path) != r['sha256']:
            raise ValueError('Old failure/control changed')
        add('known-' + str(index), read_pcm(path), r, 'known', original_id=r['id'], original_suite=r['suite'])
    sources = source_rows(root)
    for source in sources:
        if sha(pathlib.Path(source['path'])) != source['sha256']:
            raise ValueError('Source changed')
        original = np.asarray(read_pcm(pathlib.Path(source['path'])), dtype=np.float64)
        for db in (-40, -60):
            attenuated = original * 10 ** (db / 20)
            # Both branches share identical double-precision attenuation before
            # differing ONLY by added PCM16 quantization vs float32 retention.
            for fmt, values in [('float32', attenuated.astype(np.float32)), ('pcm16', quantize(attenuated))]:
                add(source['id'] + f'-{db}-{fmt}', values, source, 'quantization-pairs',
                    source_id=source['id'], attenuation_db=db, representation=fmt,
                    pair_id=source['id'] + '-' + str(db))
    for index, source in enumerate(sources[:30]):
        original = read_pcm(pathlib.Path(source['path']))
        variants = [('gain-50dB', [v * 10 ** (-50 / 20) for v in original])]
        for kind, snr in [('white', 5), ('bursts', 0)]:
            values, _ = mix(original, noise(kind, len(original), SEED + index), snr)
            variants.append((kind + '-snr' + str(snr), values))
        for condition, values in variants:
            add(source['id'] + '-new-' + condition, quantize(values), source,
                'new-transforms', condition=condition, noise_seed=SEED + index)
    negative_source = dict(path='deterministic-synthetic', sha256=None, language='Japanese', reference='')
    for kind in ('white', 'lowpass', 'bursts', 'tones'):
        values = fresh_tones(RATE * 5, (113, 226, 452)) if kind == 'tones' else noise(kind, RATE * 5, SEED)
        for level in (.002, .01):
            add(f'new-negative-{kind}-{level}', quantize([v * level for v in values]),
                negative_source, 'new-transforms', condition='negative', noise_kind=kind,
                nominal_rms=level, seed=SEED, tone_frequencies_hz=[113, 226, 452] if kind == 'tones' else None)
    for name in ('before', 'after'):
        add('new-silence-' + name, [0.0] * RATE * 5, negative_source, 'new-transforms', condition='silence')
    manifest = dict(source_holdout=False, original_sources_already_PCM16=True,
                    quantization_axis='additional attenuation-time PCM16 quantization vs float32 preservation',
                    snr_definition='whole-clip RMS including pauses; before PCM16',
                    real_noise=False, conversational_acknowledgements=False, rows=rows)
    if len(rows) != 575 or len({r['id'] for r in rows}) != len(rows):
        raise ValueError('Expected343 known+132 paired+100 new windows')
    (out / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n')
    return manifest


def validate_manifest(manifest):
    rows = manifest['rows']
    if Counter(r['suite'] for r in rows) != Counter({'known': 343, 'quantization-pairs': 132, 'new-transforms': 100}):
        raise ValueError('Missing/extra frozen gain windows')
    if len({r['id'] for r in rows}) != len(rows) or len({r['path'] for r in rows}) != len(rows):
        raise ValueError('Duplicate gain input IDs/paths')
    for row in rows:
        if not isinstance(row['reference'], str) or row['speech'] != bool(row['reference']):
            raise ValueError('Speech/reference mismatch')
        if not isinstance(row['samples'], int) or not 0 < row['samples'] <= RATE * 30:
            raise ValueError('Invalid bounded sample count')
    pairs = [r for r in rows if r['suite'] == 'quantization-pairs']
    if len({r['pair_id'] for r in pairs}) != 66 or any(
            Counter(r['representation'] for r in pairs if r['pair_id'] == pair) != Counter({'pcm16': 1, 'float32': 1})
            for pair in {r['pair_id'] for r in pairs}):
        raise ValueError('Incomplete quantization pair')


def measure(root, out):
    plan = json.loads((out / 'frozen-plan.json').read_text())
    if plan != json.loads(json.dumps(freeze_plan())):
        raise ValueError('Frozen gain comparison plan changed')
    parity = json.loads((root / 'diagnostics/official-vad-parity-r1/report.json').read_text())
    if not parity['passed'] or not parity['official_source_unmodified']:
        raise ValueError('Step1 parity must pass before step2')
    torch.set_num_threads(1)
    model, directory, _ = load_official(root)
    manifest = json.loads((out / 'manifest.json').read_text())
    validate_manifest(manifest)
    details = []
    with (out / 'probabilities.jsonl').open('x') as output:
        for row in manifest['rows']:
            path = pathlib.Path(row['path'])
            if sha(path) != row['sha256']:
                raise ValueError('Prepared float PCM changed')
            original = np.frombuffer(path.read_bytes(), dtype='<f4').copy()
            if len(original) != row['samples']:
                raise ValueError('Prepared length changed')
            for policy in POLICIES:
                samples, gains = transform(original, policy)
                probabilities = infer(model, samples)
                spans = endpoints(probabilities, len(samples), .7)
                record = dict(id=row['id'], policy=policy, real_samples=len(samples),
                              probabilities=probabilities, frame_gains=gains)
                output.write(json.dumps(record) + '\n')
                output.flush()
                details.append(dict(id=row['id'], original_id=row.get('original_id'),
                                    suite=row['suite'], speech=row['speech'], language=row['language'],
                                    representation=row.get('representation'), pair_id=row.get('pair_id'),
                                    attenuation_db=row.get('attenuation_db'), policy=policy,
                                    detected=bool(spans), max_probability=max(probabilities),
                                    gain_min=min(gains), gain_max=max(gains), intervals=spans))
    groups = []
    for suite in ('known', 'new-transforms', 'quantization-pairs'):
        for policy in POLICIES:
            selected = [d for d in details if d['suite'] == suite and d['policy'] == policy]
            groups.append(dict(suite=suite, policy=policy, speech_windows=sum(d['speech'] for d in selected),
                               negative_windows=sum(not d['speech'] for d in selected),
                               missed_speech=[d['id'] for d in selected if d['speech'] and not d['detected']],
                               false_positive=[d['id'] for d in selected if not d['speech'] and d['detected']]))
    report = dict(plan=plan, frozen_plan_sha256=sha(out / 'frozen-plan.json'),
                  manifest_sha256=sha(out / 'manifest.json'), raw_sha256=sha(out / 'probabilities.jsonl'),
                  step1_report_sha256=sha(root / 'diagnostics/official-vad-parity-r1/report.json'),
                  source_holdout=False, policy_adopted=False, quality_accepted=False,
                  asr_calls=0, correction_implemented=False, groups=groups, details=details)
    with (out / 'report.json').open('x') as output:
        output.write(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(dict(groups=groups), ensure_ascii=False, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=pathlib.Path)
    parser.add_argument('--tag', required=True)
    parser.add_argument('--prepare-only', action='store_true')
    parser.add_argument('--measure-only', action='store_true')
    args = parser.parse_args()
    if args.prepare_only and args.measure_only:
        parser.error('Choose one phase')
    if not args.tag or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-_' for c in args.tag):
        parser.error('Invalid tag')
    root = args.root.resolve()
    repo = pathlib.Path(__file__).resolve().parents[2]
    if root == repo or repo in root.parents:
        parser.error('Audio/builds remain outside Git')
    out = root / 'diagnostics' / args.tag
    if not args.measure_only:
        out.mkdir(parents=True, exist_ok=False)
        (out / 'frozen-plan.json').write_text(json.dumps(freeze_plan(), indent=2) + '\n')
        prepare(root, out)
    if not args.prepare_only:
        measure(root, out)


if __name__ == '__main__':
    main()
