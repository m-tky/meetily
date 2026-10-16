"""Explicit public-model preparation. No app profile, auth store, audio or inference access."""
import argparse
import fcntl
import hashlib
import json
import os
import pathlib
import subprocess
import tempfile
import urllib.request


def verify(path, entry):
    if not path.is_file() or path.stat().st_size != entry['bytes']:
        return False
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest() == entry['sha256']


def prepare(directory, manifest, initializer, *, check=False, offline=False):
    if directory.is_relative_to('/nix/store'):
        raise SystemExit('Use a writable model directory outside the Nix store.')
    if check and not directory.is_dir():
        raise SystemExit('Model directory unavailable.')
    if not check:
        directory.mkdir(parents=True, exist_ok=True, mode=0o700)
    for entry in manifest['files']:
        name = entry['file']
        if pathlib.Path(name).name != name:
            raise SystemExit('Invalid model manifest.')
        path = directory / name
        if verify(path, entry):
            continue
        if path.exists() or path.is_symlink():
            raise SystemExit(f'Existing model file differs: {name}; not overwritten.')
        if check or offline:
            raise SystemExit(f'Missing verified model file: {name}; download disabled.')
        print(f'Downloading public model file: {name}', flush=True)
        url = f"https://huggingface.co/{manifest['id']}/resolve/{manifest['revision']}/{name}"
        fd, temporary = tempfile.mkstemp(prefix='.qwen-download-', dir=directory)
        try:
            with os.fdopen(fd, 'wb') as target, urllib.request.urlopen(url, timeout=120) as source:
                count = 0
                while chunk := source.read(4 * 1024 * 1024):
                    count += len(chunk)
                    if count > entry['bytes']:
                        raise ValueError('unexpected download size')
                    target.write(chunk)
                target.flush()
                os.fsync(target.fileno())
            if not verify(pathlib.Path(temporary), entry):
                raise ValueError('checksum mismatch')
            os.link(temporary, path)  # Atomic publication; never replace a concurrent writer.
        except Exception:
            # Never expose a redirected signed URL, headers, or authorization details.
            raise SystemExit(f'Model download/checksum failed: {name}; existing files unchanged.') from None
        finally:
            if os.path.exists(temporary):
                os.unlink(temporary)
    if check:
        if not (directory / 'qwen-asr-int8.sidecar').is_file():
            raise SystemExit('Official files verified, but the local INT8 cache is missing.')
        print('Official model file hashes and local cache presence verified; no inference.')
        return
    subprocess.run([initializer, str(directory)], check=True)
    if not (directory / 'qwen-asr-int8.sidecar').is_file():
        raise SystemExit('Local INT8 cache was not created.')
    print('Prepared pinned Qwen3-ASR 1.7B; no audio/transcript was read or sent.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', required=True)
    parser.add_argument('--initializer', required=True)
    parser.add_argument('--check', action='store_true', help='Read-only hash/cache-presence verification')
    parser.add_argument('--offline', action='store_true', help='No downloads; initialize from existing verified official files')
    parser.add_argument('directory')
    args = parser.parse_args()
    directory = pathlib.Path(args.directory).expanduser().resolve()
    manifest = json.loads(pathlib.Path(args.manifest).read_text())
    if directory.is_relative_to('/nix/store'):
        raise SystemExit('Use a writable model directory outside the Nix store.')
    if args.check:
        prepare(directory, manifest, args.initializer, check=True)
    else:
        directory.mkdir(parents=True, exist_ok=True, mode=0o700)
        with (directory / '.qwen-preparation.lock').open('a') as lease:
            fcntl.flock(lease, fcntl.LOCK_EX | fcntl.LOCK_NB)
            prepare(directory, manifest, args.initializer, offline=args.offline)


if __name__ == '__main__':
    main()
