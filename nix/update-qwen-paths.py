"""Update only local ASR executable/model paths; never open auth or recording files."""
import json
import os
import pathlib
import sys
import tempfile

path, worker, model, ort = map(pathlib.Path, sys.argv[1:])
if not worker.is_file() or not ort.is_file() or not model.is_dir():
    raise SystemExit('Meetily: prepared worker/model/ORT path unavailable; existing settings unchanged')
path.parent.mkdir(parents=True, exist_ok=True)
if path.is_symlink():
    raise SystemExit('Meetily: refusing to replace a symlinked ASR settings file')
saved = json.loads(path.read_text()) if path.exists() else {}
if not isinstance(saved, dict):
    raise SystemExit('Meetily: invalid ASR settings; refusing replacement')
if path.exists():
    try:
        os.link(path, path.with_name(path.stem + '.pre-home-manager.json'))
    except FileExistsError:
        pass
saved.update(worker_path=str(worker), model_dir=str(model), ort_library=str(ort))
fd, temporary = tempfile.mkstemp(prefix='.qwen-paths-', dir=path.parent)
try:
    with os.fdopen(fd, 'w') as output:
        json.dump(saved, output)
        output.flush()
        os.fsync(output.fileno())
    os.replace(temporary, path)
finally:
    if os.path.exists(temporary):
        os.unlink(temporary)
print('Meetily local ASR paths configured; authentication and recording settings untouched.')
