"""Frozen-candidate VAD validation on unused transformations and new lexical speakers.
Not fully independent source holdout: 26 FLEURS clean clips were in original30.
No capture/playback/cloud ASR; downloaded human words are not conversational acks.
"""
import argparse
from collections import Counter
import hashlib
import json
import math
import pathlib
import subprocess
import urllib.parse

from compare_current_vad import sha
from diagnose_gate import endpoints
from evaluate_robustness import RATE, mix, noise, pcm_metadata, read_pcm, rms, write_pcm
from evaluate_short_speech import fetch
from prepare_current_vad import REVISION
from worker_client import Worker

SEEDS = (7212027, 7212028)
LEXICAL = [('File:LL-Q5287 (jpn)-葵心-車.wav', '車', '葵心'),
           ('File:LL-Q5287 (jpn)-Fukujinzuke-本.wav', '本', 'Fukujinzuke')]
SETTINGS = [('v4-default', 'v4', .5), ('current-default-equivalent', 'current', .5),
            ('current-frozen-candidate', 'current', .7)]


def fresh_tones(length, frequencies):
    values = [sum(math.sin(2 * math.pi * hz * i / RATE + math.pi / 7)
                  for hz in frequencies) / len(frequencies) for i in range(length)]
    scale = rms(values)
    return [v / scale for v in values]


def freeze_plan():
    return dict(candidate=dict(model_revision=REVISION, gain_policy='peak20', positive=.7,
                               negative=.55, minimum_ms_strictly_greater=90),
                controls=[dict(model='v4', gain_policy='peak20', positive=.5, minimum_ms=90),
                          dict(model='current', gain_policy='peak20', positive=.5, minimum_ms=90)],
                retuning=False, seeds=SEEDS, source_holdout=False,
                unseen_transformations_of_other_26_fleurs=True,
                lexical_sources=LEXICAL, conversational_acknowledgements=False,
                speech_conditions=['clean', 'gain-20dB', 'gain-40dB', 'white-snr20',
                                   'white-snr10', 'white-snr0', 'lowpass-snr10', 'bursts-snr10'],
                negatives='2 seeds x white/lowpass/bursts/new-frequency-tones x4RMS +2silences',
                scope='VAD presence only; no ASR rerun/model adoption/app integration')


def prepare(root, out):
    original = json.loads((root / 'artifacts/manifest.json').read_text())
    old = json.loads((root / 'robustness/synthetic-r1/manifest.json').read_text())
    excluded = {r['source_id'] for r in old['rows'] if r['purpose'] == 'accuracy'}
    sources = [r for r in original['rows'] if r['purpose'] == 'accuracy' and r['id'] not in excluded]
    if len(sources) != 26:
        raise ValueError('Expected exactly26 unused transformation sources')
    rows, source_records = [], []

    def add(identifier, values, language, reference, condition, **extra):
        if not 0 < len(values) <= RATE * 30:
            raise ValueError('Input length outside bounded contract')
        path = out / (identifier + '.wav')
        digest = write_pcm(path, values)
        rows.append(dict(id=identifier, path=str(path), sha256=digest, language=language,
                         reference=reference, condition=condition,
                         purpose='accuracy' if reference else 'silence',
                         audio_seconds=len(values) / RATE, **pcm_metadata(path), **extra))

    add('silence-before', [0.0] * RATE * 5, 'Japanese', '', 'silence', suite='new-negatives')
    for index, source in enumerate(sources):
        path = root / 'artifacts' / source['wav']
        if sha(path) != source['sha256']:
            raise ValueError('Source changed')
        speech = read_pcm(path)
        common = dict(suite='fleurs-other26', source_id=source['id'], source_sha256=source['sha256'])
        add(source['id'] + '-clean', speech, source['language'], source['reference'], 'clean', **common)
        for db in (-20, -40):
            add(source['id'] + '-gain' + str(db) + 'dB', [v * 10 ** (db / 20) for v in speech],
                source['language'], source['reference'], 'gain' + str(db) + 'dB', gain_db=db, **common)
        for kind, snrs in [('white', (20, 10, 0)), ('lowpass', (10,)), ('bursts', (10,))]:
            background = noise(kind, len(speech), seed=SEEDS[0] + index)
            for snr in snrs:
                values, gain = mix(speech, background, snr)
                add(source['id'] + '-' + kind + '-snr' + str(snr), values,
                    source['language'], source['reference'], kind + '-snr' + str(snr),
                    noise_kind=kind, nominal_snr_db=snr, common_gain=gain,
                    noise_seed=SEEDS[0] + index, **common)
    for index, (title, reference, speaker) in enumerate(LEXICAL):
        api = 'https://commons.wikimedia.org/w/api.php?' + urllib.parse.urlencode(dict(
            action='query', format='json', titles=title, prop='imageinfo|revisions', rvprop='ids',
            iiprop='url|timestamp|sha1|extmetadata'))
        metadata = json.loads(fetch(api, 1024 * 1024))
        (out / ('lexical-source-' + str(index) + '.json')).write_text(json.dumps(metadata, ensure_ascii=False, indent=2) + '\n')
        page, = metadata['query']['pages'].values()
        info = page['imageinfo'][0]
        license_name = info['extmetadata']['LicenseShortName']['value']
        if license_name not in ('CC0', 'CC BY 4.0', 'CC BY-SA 4.0'):
            raise ValueError('Unexpected lexical license')
        url = info['url']
        if not url.startswith('https://upload.wikimedia.org/'):
            raise ValueError('Unexpected lexical audio host')
        data = fetch(url, 5 * 1024 * 1024)
        if hashlib.sha1(data).hexdigest() != info['sha1']:
            raise ValueError('Published source SHA1 mismatch')
        original_path = out / ('lexical-original-' + str(index) + '.wav')
        original_path.write_bytes(data)
        converted = out / ('lexical-converted-' + str(index) + '.wav')
        subprocess.run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-i', str(original_path),
                        '-ar', str(RATE), '-ac', '1', '-c:a', 'pcm_s16le', str(converted)], check=True)
        speech = read_pcm(converted)
        record = dict(title=title, published_reference=reference, speaker=speaker, api=api,
                      page_revision=page['revisions'][0]['revid'], source_url=info['descriptionurl'],
                      source_audio_sha1=info['sha1'], source_audio_sha256=sha(original_path),
                      converted_sha256=sha(converted), converted_seconds=len(speech) / RATE,
                      timestamp=info['timestamp'], extmetadata=info['extmetadata'],
                      license=license_name, license_url=info['extmetadata']['LicenseUrl']['value'],
                      reference_basis='Published Lingua Libre file transcription; not ASR/not listened',
                      modifications='mono16k PCM16, attenuation, silence padding and synthetic white noise')
        source_records.append(record)
        variants = [('clean', speech, {}), ('padded', [0.0] * RATE + speech + [0.0] * RATE,
                                              dict(silence_before_samples=RATE, silence_after_samples=RATE)),
                    ('gain-20dB', [v * .1 for v in speech], dict(gain_db=-20)),
                    ('gain-40dB', [v * .01 for v in speech], dict(gain_db=-40))]
        for snr in (10, 0):
            values, gain = mix(speech, noise('white', len(speech), SEEDS[1] + index), snr)
            variants.append(('white-snr' + str(snr), values,
                             dict(nominal_snr_db=snr, common_gain=gain, noise_seed=SEEDS[1] + index)))
        for condition, values, extra in variants:
            add('lexical-' + str(index) + '-' + condition, values, 'Japanese', reference, condition,
                suite='new-lexical', speaker=speaker, source_title=title,
                source_audio_sha256=sha(original_path), **extra)
    for index, seed in enumerate(SEEDS):
        for kind in ('white', 'lowpass', 'bursts', 'tones'):
            frequencies = [(83, 166, 332), (149, 298, 596)][index]
            values = fresh_tones(RATE * 5, frequencies) if kind == 'tones' else noise(kind, RATE * 5, seed)
            for amplitude in (.002, .01, .05, .1):
                add(f'negative-{seed}-{kind}-{amplitude}', [v * amplitude for v in values],
                    'Japanese', '', 'negative', suite='new-negatives', noise_kind=kind,
                    nominal_noise_rms=amplitude, seed=seed,
                    frequencies_hz=frequencies if kind == 'tones' else None)
    add('silence-after', [0.0] * RATE * 5, 'Japanese', '', 'silence', suite='new-negatives')
    manifest = dict(dataset='FLEURS26 unused transformations +2new Lingua Libre lexical speakers +synthetic noise',
                    fleurs_attribution='Conneau et al., FLEURS (2022), CC-BY-4.0',
                    source_records=source_records, excluded_fleurs_source_ids=sorted(excluded),
                    source_holdout=False, natural_acknowledgements=False, real_environment_noise=False,
                    independently_listened=False, snr_definition='whole-clip RMS including pauses before PCM16',
                    rows=rows)
    (out / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n')
    return manifest


def validate_manifest(manifest):
    rows = manifest['rows']
    if Counter(r['suite'] for r in rows) != Counter({'fleurs-other26': 208, 'new-lexical': 12, 'new-negatives': 34}):
        raise ValueError('Missing/extra validation windows')
    if len({r['id'] for r in rows}) != len(rows) or len({r['path'] for r in rows}) != len(rows):
        raise ValueError('Duplicate validation IDs/paths')
    for r in rows:
        if not isinstance(r['reference'], str) or r['purpose'] != ('accuracy' if r['reference'] else 'silence'):
            raise ValueError('Reference/purpose mismatch')
        if r['purpose'] != ('silence' if r['suite'] == 'new-negatives' else 'accuracy'):
            raise ValueError('Suite/reference mismatch')
        if not math.isfinite(r['audio_seconds']) or not 0 < r['audio_seconds'] <= 30:
            raise ValueError('Invalid duration')
    identities = [(s['title'], s['published_reference'], s['speaker']) for s in manifest['source_records']]
    if identities != LEXICAL:
        raise ValueError('Published lexical identities changed')


def measure(root, out):
    plan = json.loads((out / 'frozen-plan.json').read_text())
    if plan != json.loads(json.dumps(freeze_plan())):
        raise ValueError('Frozen plan changed')
    manifest = json.loads((out / 'manifest.json').read_text())
    validate_manifest(manifest)
    model_dir = root / 'vad-models' / ('silero-' + REVISION)
    provenance = json.loads((model_dir / 'provenance.json').read_text())
    for file in provenance['files']:
        if sha(model_dir / file['file']) != file['sha256']:
            raise ValueError('Pinned model/source changed')
    paths = [r['path'] for r in manifest['rows']]
    if len(set(paths)) != len(paths):
        raise ValueError('Duplicate fixture paths')
    for row in manifest['rows']:
        if sha(pathlib.Path(row['path'])) != row['sha256']:
            raise ValueError('Fixture changed')
    neural, binaries = {}, {}
    expected = dict(v4=json.loads((root / 'diagnostics/gate-r2/report.json').read_text())['binary_sha256'],
                    current=json.loads((root / 'diagnostics/current-vad-r2/report.json').read_text())['probe_binary_sha256'])
    for name, example in [('v4', 'vad_gain_probe'), ('current', 'current_vad_probe')]:
        binary = root / 'worker-target/release/examples' / example
        binaries[name] = dict(binary=str(binary), sha256=sha(binary))
        if binaries[name]['sha256'] != expected[name]:
            raise ValueError('Previously verified probe changed')
        args = [str(binary)] + ([str(model_dir / 'silero_vad.onnx')] if name == 'current' else []) + paths
        raw = out / (name + '-probabilities.jsonl')
        with raw.open('x') as output:
            subprocess.run(args, stdout=output, check=True, timeout=300)
        records = list(map(json.loads, raw.read_text().splitlines()))
        schema = records.pop(0) if name == 'current' else None
        if schema and schema.get('event') != 'model_schema':
            raise ValueError('Bad current schema')
        mapped = {(r['file'], r['policy']): r for r in records}
        if len(mapped) != len(paths) * 3 or len(records) != len(mapped):
            raise ValueError('Incomplete/duplicate neural outputs')
        neural[name] = mapped
    details = []
    for row in manifest['rows']:
        for label, model, threshold in SETTINGS:
            record = neural[model][(row['path'], 'peak20')]
            if record['real_samples'] != round(row['audio_seconds'] * RATE):
                raise ValueError('Input duration changed')
            if model == 'v4' and endpoints(record['probabilities'], record['real_samples']) != record['default_intervals']:
                raise ValueError('V4 endpoint parity failed')
            spans = endpoints(record['probabilities'], record['real_samples'], threshold)
            details.append(dict(id=row['id'], suite=row['suite'], language=row['language'],
                                condition=row['condition'], reference=row['reference'], speaker=row.get('speaker'),
                                setting=label, speech=row['purpose'] == 'accuracy', detected=bool(spans),
                                gain=record['gain'], max_probability=max(record['probabilities']), intervals=spans))
    groups = []
    for label, _, _ in SETTINGS:
        for suite in ('fleurs-other26', 'new-lexical', 'new-negatives'):
            rows = [r for r in details if r['setting'] == label and r['suite'] == suite]
            groups.append(dict(setting=label, suite=suite, windows=len(rows),
                               missed_speech=[r['id'] for r in rows if r['speech'] and not r['detected']],
                               false_positive=[r['id'] for r in rows if not r['speech'] and r['detected']]))
    report = dict(plan=plan, frozen_plan_sha256=sha(out / 'frozen-plan.json'), manifest_sha256=sha(out / 'manifest.json'),
                  probe_binaries=binaries, model_provenance_sha256=sha(model_dir / 'provenance.json'),
                  raw_sha256={name: sha(out / (name + '-probabilities.jsonl')) for name in neural},
                  windows=len(manifest['rows']), retuned=False, source_holdout=False,
                  quality_accepted=False, model_adopted=False, asr_rerun=False, groups=groups, details=details)
    with (out / 'report.json').open('x') as output:
        output.write(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(dict(windows=report['windows'], groups=groups), ensure_ascii=False, indent=2))


def measure_short_asr(root, out):
    """Existing v4/full-input worker CONTROL, not a new-VAD pipeline."""
    manifest = json.loads((out / 'manifest.json').read_text())
    validate_manifest(manifest)
    rows = [r for r in manifest['rows'] if r['suite'] == 'new-lexical' or r['condition'] == 'silence']
    fixed = pathlib.Path(__file__).resolve().parents[2] / 'docs/meeting-assistant/evidence/qwen3-asr/worker/fixed/provenance.json'
    binary = root / 'worker-target/release/qwen-speech-worker'
    if sha(binary) != json.loads(fixed.read_text())['binary_sha256']:
        raise ValueError('Existing worker changed')
    for row in rows:
        if sha(pathlib.Path(row['path'])) != row['sha256']:
            raise ValueError('Lexical/control fixture changed')
    with (out / 'short-asr-control.jsonl').open('x') as output, Worker(
            binary, root / 'artifacts/model-1.7B', timeout=120) as worker:
        for index, row in enumerate(rows):
            response = worker.request(dict(op='transcribe', id='new-short-' + str(index),
                                           wav=row['path'], language=row['language']))
            result = dict(id=row['id'], reference=row['reference'], condition=row['condition'],
                          fixture_sha256=row['sha256'], audio_seconds=row['audio_seconds'],
                          text=response.get('text', ''), worker=response,
                          existing_v4_full_input_control=True, current_vad_used=False)
            output.write(json.dumps(result, ensure_ascii=False) + '\n')
            output.flush()
            print(json.dumps(result, ensure_ascii=False), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=pathlib.Path)
    parser.add_argument('--tag', required=True)
    parser.add_argument('--prepare-only', action='store_true')
    parser.add_argument('--measure-only', action='store_true')
    parser.add_argument('--short-asr-only', action='store_true')
    args = parser.parse_args()
    if sum([args.prepare_only, args.measure_only, args.short_asr_only]) > 1:
        parser.error('Choose one phase')
    if not args.tag or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-_' for c in args.tag):
        parser.error('Invalid tag')
    root = args.root.resolve()
    repo = pathlib.Path(__file__).resolve().parents[2]
    if root == repo or repo in root.parents:
        parser.error('Audio/model/builds must remain outside Git')
    out = root / 'diagnostics' / args.tag
    if args.short_asr_only:
        measure_short_asr(root, out)
        return
    if not args.measure_only:
        out.mkdir(parents=True, exist_ok=False)
        (out / 'frozen-plan.json').write_text(json.dumps(freeze_plan(), ensure_ascii=False, indent=2) + '\n')
        prepare(root, out)
    if not args.prepare_only:
        measure(root, out)


if __name__ == '__main__':
    main()
