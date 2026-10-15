"""One licensed, human-recorded isolated Japanese word; NOT conversation acceptance.

Reference comes from Wikimedia/Lingua Libre metadata, not an ASR transcript.
Do not present this single speaker's pronunciation of 'いいえ' as proof that
natural はい/うん, acknowledgements in noise or all quiet speech are protected.
Audio stays outside Git; retain CC-BY-SA attribution and transformation details.
"""
import argparse
import hashlib
import json
import pathlib
import subprocess
import urllib.parse
import urllib.request

from evaluate_robustness import RATE, SEED, mix, noise, pcm_metadata, read_pcm, run, write_pcm

TITLE = 'File:LL-Q5287 (jpn)-Zsrtrgh-いいえ.wav'
REFERENCE = 'いいえ'


def fetch(url, limit):
    request = urllib.request.Request(url, headers={'User-Agent': 'MeetilyPublicASREvaluation/0.1'})
    with urllib.request.urlopen(request, timeout=60) as response:
        data = response.read(limit + 1)
    if len(data) > limit:
        raise ValueError('Public source exceeds size bound')
    return data


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
        parser.error('Audio/builds must stay outside repository')
    out = root / 'robustness' / args.tag
    out.mkdir(parents=True, exist_ok=False)
    params = dict(action='query', format='json', titles=TITLE,
                  prop='imageinfo|revisions', rvprop='ids',
                  iiprop='url|timestamp|sha1|extmetadata')
    api = 'https://commons.wikimedia.org/w/api.php?' + urllib.parse.urlencode(params)
    metadata = json.loads(fetch(api, 1024 * 1024))
    (out / 'source-metadata.json').write_text(json.dumps(metadata, ensure_ascii=False, indent=2) + '\n')
    page, = metadata['query']['pages'].values()
    info = page['imageinfo'][0]
    license_name = info['extmetadata']['LicenseShortName']['value']
    if license_name != 'CC BY-SA 4.0':
        raise ValueError('Source license changed; review required')
    url = info['url'].split('?')[0]
    if not url.startswith('https://upload.wikimedia.org/'):
        raise ValueError('Unexpected audio host')
    original = fetch(url, 5 * 1024 * 1024)
    if hashlib.sha1(original).hexdigest() != info['sha1']:
        raise ValueError('Wikimedia source hash mismatch')
    (out / 'original.wav').write_bytes(original)
    subprocess.run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-i', str(out / 'original.wav'),
                    '-ar', str(RATE), '-ac', '1', '-c:a', 'pcm_s16le', str(out / 'converted.wav')], check=True)
    speech = read_pcm(out / 'converted.wav')
    if not 0 < len(speech) <= RATE * 30:
        raise ValueError('Invalid source duration')
    variants = [('clean', speech, {})]
    variants.append(('padded', [0.0] * RATE + speech + [0.0] * RATE, {}))
    for db in (-20, -40):
        variants.append((f'gain{db}dB', [s * 10 ** (db / 20) for s in speech], dict(gain_db=db)))
    for snr in (10, 0):
        values, gain = mix(speech, noise('white', len(speech)), snr)
        variants.append((f'white-snr{snr}', values, dict(nominal_snr_db=snr, common_gain=gain)))
    rows = []
    for condition, values, extra in variants + [('negative', [0.0] * RATE * 5, {})]:
        identifier = 'negative-silence' if condition == 'negative' else 'iie-' + condition
        path = out / (identifier + '.wav')
        digest = write_pcm(path, values)
        rows.append(dict(id=identifier, language='Japanese', wav=path.name, sha256=digest,
                         reference='' if condition == 'negative' else REFERENCE,
                         condition=condition, purpose='silence' if condition == 'negative' else 'accuracy',
                         audio_seconds=len(values) / RATE, **pcm_metadata(path), **extra))
    manifest = dict(dataset='Wikimedia Commons / Lingua Libre', source_title=TITLE,
                    source_api=api, source_url=info['descriptionurl'],
                    source_page_revision=page['revisions'][0]['revid'],
                    source_audio_timestamp=info['timestamp'], source_audio_sha1=info['sha1'],
                    source_audio_sha256=hashlib.sha256(original).hexdigest(),
                    attribution='Speaker and recorder: Zsrtrgh (2025-11-03), Lingua Libre',
                    license=license_name, license_url='https://creativecommons.org/licenses/by-sa/4.0/',
                    modifications='mono 16k PCM16 resample, zero padding, attenuation and synthetic white noise',
                    reference_basis='Published transcription on the source file page',
                    independently_listened=False, spontaneous_conversation=False,
                    genuine_lexical_short_word=True, genuine_conversational_acknowledgements=False,
                    seed=SEED, snr_definition='whole-clip RMS before PCM16 quantization', rows=rows)
    (out / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n')
    run(root, out, manifest)


if __name__ == '__main__':
    main()
