"""Extract the fixed first ten public FLEURS Thai validation rows, no dataset cache."""
import json
import pathlib
import sys
import pyarrow.parquet as pq

parquet, root = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
# The first row group is large; batch_size bounds decoded row materialization.
batch = next(pq.ParquetFile(parquet).iter_batches(batch_size=10))
rows = []
for index, row in enumerate(batch.to_pylist()):
    audio = row['audio']
    path = root / f"thai-public-{index}-{row['id']}.wav"
    path.write_bytes(audio['bytes'])
    rows.append({'row_idx': index, 'row': {'id': row['id'], 'transcription': row['transcription'],
        'audio': [{'local': str(path)}]}})
(root / 'thai-rows.json').write_text(json.dumps({'rows': rows}, ensure_ascii=False), encoding='utf-8')
