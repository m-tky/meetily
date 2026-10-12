"""Generate Candle's tokenizer.json from the official LOCAL tokenizer artifacts.
No network/Hub credentials. Derived file hash is recorded, source files unmodified.
"""
import os
os.environ['HF_HUB_OFFLINE'] = '1'
os.environ['TRANSFORMERS_OFFLINE'] = '1'
import hashlib
import json
import pathlib
import sys
from transformers import AutoTokenizer
root = pathlib.Path(sys.argv[1])
for size in ('0.6B', '1.7B'):
    model = root / f'model-{size}'
    if not (model / 'tokenizer_config.json').exists():
        continue
    tokenizer = AutoTokenizer.from_pretrained(str(model), local_files_only=True, use_fast=True)
    target = model / 'tokenizer.json'
    tokenizer.backend_tokenizer.save(str(target))
    metadata = {'source': 'official local vocab/merges/tokenizer_config',
        'transformers': '4.57.6', 'sha256': hashlib.sha256(target.read_bytes()).hexdigest(),
        'vocab_size_with_added_tokens': len(tokenizer)}
    (model / 'derived-tokenizer.json').write_text(json.dumps(metadata, indent=2))
    print(size, metadata)
