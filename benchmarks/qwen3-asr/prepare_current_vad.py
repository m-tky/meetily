"""User-approved official-current VAD comparison: download pinned public files only.
No replacement of embedded v4, no app/Cargo/dependency/cache edits.
Verify each file's Git blob SHA1 against the pinned tree and record SHA256.
"""
import argparse
import hashlib
import json
import pathlib
import urllib.request

REVISION = '1e261b036686cd0017d500ee96acd1c4ba572a9d'
MODEL = 'src/silero_vad/data/silero_vad.onnx'
MODEL_BLOB = '80c5592ef1f4c9ede3e357bbd02eb863358a6a9d'
FILES = [MODEL, 'src/silero_vad/utils_vad.py', 'LICENSE', 'pyproject.toml', 'src/silero_vad/__init__.py']


def fetch(url, maximum):
    request = urllib.request.Request(url, headers={'User-Agent': 'MeetilyPublicASREvaluation/0.1'})
    with urllib.request.urlopen(request, timeout=90) as response:
        data = response.read(maximum + 1)
    if len(data) > maximum:
        raise ValueError('Public artifact exceeds size bound')
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=pathlib.Path)
    args = parser.parse_args()
    root = args.root.resolve()
    repo = pathlib.Path(__file__).resolve().parents[2]
    if root == repo or repo in root.parents:
        parser.error('Model must stay outside Git')
    out = root / 'vad-models' / ('silero-' + REVISION)
    out.mkdir(parents=True, exist_ok=False)
    tree_url = f'https://api.github.com/repos/snakers4/silero-vad/git/trees/{REVISION}?recursive=1'
    tree = json.loads(fetch(tree_url, 4 * 1024 * 1024))
    if tree.get('truncated'):
        raise ValueError('Incomplete pinned tree')
    bypath = {r['path']: r for r in tree['tree']}
    if bypath[MODEL]['sha'] != MODEL_BLOB:
        raise ValueError('Unexpected pinned model')
    metadata = dict(repository='https://github.com/snakers4/silero-vad', revision=REVISION,
                    model_path=MODEL, model_git_blob_sha1=MODEL_BLOB,
                    license='MIT', user_approved_comparison=True, worker_replaced=False, files=[])
    for name in FILES:
        info = bypath[name]
        if info['type'] != 'blob' or info['size'] > 5 * 1024 * 1024:
            raise ValueError('Unexpected public file')
        url = f'https://raw.githubusercontent.com/snakers4/silero-vad/{REVISION}/{name}'
        data = fetch(url, info['size'])
        blob = b'blob ' + str(len(data)).encode() + b'\0' + data
        if len(data) != info['size'] or hashlib.sha1(blob).hexdigest() != info['sha']:
            raise ValueError('Pinned Git blob mismatch')
        path = out / pathlib.Path(name).name
        path.write_bytes(data)
        metadata['files'].append(dict(file=path.name, upstream_path=name, url=url,
                                      bytes=len(data), git_blob_sha1=info['sha'],
                                      sha256=hashlib.sha256(data).hexdigest()))
    (out / 'provenance.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(out)


if __name__ == '__main__':
    main()
