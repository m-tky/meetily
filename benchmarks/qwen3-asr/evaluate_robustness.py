"""Bounded public-speech/synthetic-noise probe, not meeting acceptance.

No capture, playback or cloud inference. No model/VAD policy changes.
Noise SNR uses whole-clip RMS (including pauses), NOT active-speech RMS.
All misses, flagged text and non-speech false positives remain in raw results.
"""
import argparse
import hashlib
import json
import math
import pathlib
import random
import struct
import time
import wave

from worker_client import Worker

RATE = 16000
SEED = 5122026


def rms(values):
    return math.sqrt(sum(x * x for x in values) / len(values))


def noise(kind, length, seed=SEED):
    rng = random.Random(seed)
    values = []
    low = 0.0
    for i in range(length):
        white = rng.uniform(-1, 1)
        if kind == 'white':
            value = white
        elif kind == 'lowpass':
            low = .96 * low + .04 * white
            value = low
        elif kind == 'bursts':
            # Artificial click/burst train, not recorded keyboard noise.
            value = white * math.exp(-(i % 4000) / 80)
        elif kind == 'tones':
            # Non-speech harmonic proxy, NOT music.
            value = sum(math.sin(2 * math.pi * hz * i / RATE)
                        for hz in (110, 220, 440)) / 3
        else:
            raise ValueError('Unknown noise type')
        values.append(value)
    scale = rms(values)
    return [x / scale for x in values]


def mix(speech, background, snr_db):
    amplitude = rms(speech) / (10 ** (snr_db / 20))
    values = [s + amplitude * n for s, n in zip(speech, background)]
    # One common gain prevents clipping while preserving speech/noise ratio.
    gain = min(1.0, .98 / max(abs(x) for x in values))
    return [x * gain for x in values], gain


def read_pcm(path):
    with wave.open(str(path), 'rb') as wav:
        if (wav.getnchannels(), wav.getframerate(), wav.getsampwidth()) != (1, RATE, 2):
            raise ValueError('Expected mono 16k PCM16')
        data = wav.readframes(wav.getnframes())
    return [x / 32768 for x in struct.unpack('<' + 'h' * (len(data) // 2), data)]


def pcm_metadata(path):
    values = read_pcm(path)
    return dict(pcm_rms=rms(values), pcm_peak=max(abs(x) for x in values),
                pcm_saturated_samples=sum(x in (-1.0, 32767 / 32768) for x in values))


def write_pcm(path, values):
    pcm = [max(-32768, min(32767, round(x * 32768))) for x in values]
    with wave.open(str(path), 'wb') as wav:
        wav.setnchannels(1)
        wav.setframerate(RATE)
        wav.setsampwidth(2)
        wav.writeframes(struct.pack('<' + 'h' * len(pcm), *pcm))
    return hashlib.sha256(path.read_bytes()).hexdigest()


def prepare(root, out):
    original = json.loads((root / 'artifacts/manifest.json').read_text())
    selected = []
    for language, count in [('Japanese', 2), ('English', 1), ('Thai', 1)]:
        selected.extend([r for r in original['rows']
                         if r['purpose'] == 'accuracy' and r['language'] == language][:count])
    if len(selected) != 4:
        raise ValueError('Missing public source clips')
    rows = []

    def add(identifier, values, language, reference, condition, **metadata):
        path = out / (identifier + '.wav')
        digest = write_pcm(path, values)
        rows.append(dict(id=identifier, wav=path.name, sha256=digest,
                         language=language, reference=reference, condition=condition,
                         purpose='silence' if reference == '' else 'accuracy',
                         audio_seconds=len(values) / RATE, **pcm_metadata(path), **metadata))

    for source in selected:
        path = root / 'artifacts' / source['wav']
        if hashlib.sha256(path.read_bytes()).hexdigest() != source['sha256']:
            raise ValueError('Source hash mismatch')
        speech = read_pcm(path)
        variants = [('clean', speech, {})]
        for db in (-20, -40):
            variants.append((f'gain{db}dB', [s * 10 ** (db / 20) for s in speech],
                             dict(gain_db=db)))
        for kind, snrs in [('white', (20, 10, 0)), ('lowpass', (10,)), ('bursts', (10,))]:
            background = noise(kind, len(speech))
            for snr in snrs:
                values, gain = mix(speech, background, snr)
                variants.append((f'{kind}-snr{snr}', values,
                                 dict(noise_kind=kind, nominal_snr_db=snr, common_gain=gain)))
        for condition, values, metadata in variants:
            add(source['id'] + '-' + condition, values, source['language'],
                source['reference'], condition, source_id=source['id'],
                source_sha256=source['sha256'], **metadata)
    for kind in ('white', 'lowpass', 'bursts', 'tones'):
        background = noise(kind, RATE * 5)
        for amplitude in (.002, .01, .05, .1):
            add(f'negative-{kind}-{amplitude}', [x * amplitude for x in background],
                'Japanese', '', 'negative', noise_kind=kind, nominal_noise_rms=amplitude)
    add('negative-silence', [0.0] * (RATE * 5), 'Japanese', '', 'negative', noise_kind='silence')
    manifest = dict(dataset='google/fleurs + deterministic synthetic noise',
                    license='FLEURS CC-BY-4.0', attribution='Conneau et al., FLEURS (2022)',
                    seed=SEED, snr_definition='whole-clip RMS before PCM16 quantization',
                    real_environment_noise=False, genuine_acknowledgements=False,
                    rows=rows)
    (out / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n')
    return manifest


def run(root, out, manifest):
    rows = manifest['rows']
    tail = dict(next(r for r in rows if r['id'] == 'negative-silence'),
                id='negative-silence-after')
    rows.append(tail)
    (out / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n')
    # Refuse to overwrite a measurement: use a fresh --tag for each pass.
    with (out / 'results.jsonl').open('x') as output, Worker(
            root / 'worker-target/release/qwen-speech-worker',
            root / 'artifacts/model-1.7B', timeout=120) as worker:
        # Negatives before AND after speech reveal lazy loading and state carry.
        ordered = [r for r in rows if r['purpose'] == 'silence' and r is not tail] + [
            r for r in rows if r['purpose'] == 'accuracy'] + [tail]
        for index, row in enumerate(ordered):
            path = out / row['wav']
            if hashlib.sha256(path.read_bytes()).hexdigest() != row['sha256']:
                raise ValueError('Fixture hash mismatch')
            tick = time.monotonic()
            response = worker.request(dict(op='transcribe', id=f'robust-{index}',
                                           wav=str(path), language=row['language']))
            status = response.get('status')
            result = dict(id=row['id'], language=row['language'], condition=row['condition'],
                          reference=row['reference'], audio_seconds=row['audio_seconds'],
                          text=response.get('text', ''), rep=0,
                          status='ok' if status in ('recognized', 'review_required', 'no_speech') else 'error',
                          elapsed_seconds=response.get('inference_seconds', 0),
                          request_seconds=time.monotonic() - tick,
                          engine='vad-gated-native-rust', threads=6,
                          worker=response, fixture_sha256=row['sha256'])
            output.write(json.dumps(result, ensure_ascii=False) + '\n')
            output.flush()
            print(json.dumps(dict(id=row['id'], status=status,
                                  asr_calls=response.get('asr_calls'), text=result['text']),
                             ensure_ascii=False), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=pathlib.Path)
    parser.add_argument('--tag', required=True)
    parser.add_argument('--prepare-only', action='store_true')
    args = parser.parse_args()
    if not args.tag or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-_' for c in args.tag):
        parser.error('Tag must contain lowercase ASCII letters, digits, - or _')
    root = args.root.resolve()
    repo = pathlib.Path(__file__).resolve().parents[2]
    if root == repo or repo in root.parents:
        parser.error('Audio/builds must remain outside repository')
    out = root / 'robustness' / args.tag
    out.mkdir(parents=True, exist_ok=False)
    manifest = prepare(root, out)
    if not args.prepare_only:
        run(root, out, manifest)


if __name__ == '__main__':
    main()
