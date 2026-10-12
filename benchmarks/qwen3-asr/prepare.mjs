// Public, credential-free downloads. No audio capture or cloud inference.
import { mkdir, stat, writeFile, rename, unlink, open } from 'node:fs/promises';
import { createReadStream, createWriteStream } from 'node:fs';
import { pipeline } from 'node:stream/promises';
import { Readable } from 'node:stream';
import { createHash } from 'node:crypto';
import { resolve, join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';
import { rangeDownload } from './download.mjs';
const root = resolve(process.argv[2]);
const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
if (root === repo || root.startsWith(repo + '/')) throw new Error('Artifacts must stay outside repo');
const task = process.argv[3];
await mkdir(root, { recursive: true });
async function sha(path) {
  const h = createHash('sha256');
  for await (const b of createReadStream(path)) h.update(b);
  return h.digest('hex');
}
async function json(url) {
  const r = await fetch(url, { signal: AbortSignal.timeout(180000) });
  if (!r.ok) throw new Error(`Public metadata HTTP ${r.status}`);
  return r.json();
}
async function download(url, path, expected) {
  if (expected && await stat(path).catch(() => null) && await sha(path) === expected) return;
  for (let attempt = 0; attempt < 3; attempt++) {
    try {
      const r = await fetch(url, { signal: AbortSignal.timeout(3600000) });
      if (!r.ok) throw new Error(`Public artifact HTTP ${r.status}`);
      await pipeline(Readable.fromWeb(r.body), createWriteStream(path + '.part'));
      if (expected && await sha(path + '.part') !== expected) throw new Error('Artifact SHA mismatch');
      await rename(path + '.part', path);
      return;
    } catch (e) {
      await unlink(path + '.part').catch(() => {});
      if (attempt === 2) throw e;
      console.error(`Retry ${attempt + 1}: ${e.message}`);
    }
  }
}
if (task === 'models') {
  for (const size of ['0.6B', '1.7B']) {
    const id = `Qwen/Qwen3-ASR-${size}`;
    const info = await json(`https://huggingface.co/api/models/${id}?blobs=true`);
    const dir = join(root, `model-${size}`);
    await mkdir(dir, { recursive: true });
    const files = info.siblings.filter(x => /^(config\.json|generation_config\.json|model.*\.(safetensors|json)|vocab\.json|merges\.txt|tokenizer.*\.json|preprocessor_config\.json)$/.test(x.rfilename));
    const metadata = { id, revision: info.sha, license: 'Apache-2.0', files: [] };
    // Parallel shards, but models are prepared sequentially so 0.6B can run early.
    await Promise.all(files.map(async f => {
      const path = join(dir, f.rfilename);
      console.error(`Download/verify ${id}/${f.rfilename} (${f.size ?? f.lfs?.size ?? '?'} bytes)`);
      const url = `https://huggingface.co/${id}/resolve/${info.sha}/${f.rfilename}`;
      if (f.lfs?.sha256 && f.size > 64 * 1024 * 1024) await rangeDownload(url, path, f.lfs.sha256, f.size);
      else await download(url, path, f.lfs?.sha256);
      const entry = { file: f.rfilename, sha256: await sha(path), bytes: (await stat(path)).size };
      if (f.rfilename.endsWith('.safetensors')) {
        const handle = await open(path);
        try {
          const len = Buffer.alloc(8); await handle.read(len, 0, 8, 0);
          const n = Number(len.readBigUInt64LE());
          if (n > 20_000_000) throw new Error('Unexpected safetensors header');
          const header = Buffer.alloc(n); await handle.read(header, 0, n, 8);
          const tensors = JSON.parse(header.toString('utf8'));
          entry.dtypes = [...new Set(Object.entries(tensors).filter(([k]) => k !== '__metadata__').map(([,v]) => v.dtype))];
        } finally { await handle.close(); }
      }
      metadata.files.push(entry);
    }));
    metadata.files.sort((a,b) => a.file.localeCompare(b.file));
    await writeFile(join(dir, 'provenance.json'), JSON.stringify(metadata, null, 2));
    console.log(`${size} ready`);
  }
} else if (task === 'audio') {
  const rows = [];
  for (const [config, language] of [['ja_jp', 'Japanese'], ['en_us', 'English'], ['th_th', 'Thai']]) {
    const api = `https://datasets-server.huggingface.co/rows?dataset=google/fleurs&config=${config}&split=validation&offset=0&length=10`;
    let d;
    if (config === 'th_th') {
      // The row server cannot scan this 327MB row group. Download/hash the
      // published validation Parquet, then decode only the first ten rows.
      const tree = await json('https://huggingface.co/api/datasets/google/fleurs/tree/main/parquet-data/th_th');
      const meta = tree.find(x => x.path.endsWith('validation-00000-of-00001.parquet'));
      if (!meta?.lfs?.oid) throw new Error('Missing Parquet hash');
      const datasetInfo = await json('https://huggingface.co/api/datasets/google/fleurs');
      const parquet = join(root, 'th_th-validation.parquet');
      await rangeDownload(`https://huggingface.co/datasets/google/fleurs/resolve/${datasetInfo.sha}/${meta.path}`,parquet,meta.lfs.oid,meta.size);
      execFileSync(process.env.BENCH_PYTHON ?? 'python3', [join(dirname(fileURLToPath(import.meta.url)), 'extract_parquet.py'),parquet,root]);
      const {readFile} = await import('node:fs/promises');
      d = JSON.parse(await readFile(join(root, 'thai-rows.json'),'utf8'));
      await writeFile(join(root,'thai-provenance.json'),JSON.stringify({revision:datasetInfo.sha,path:meta.path,sha256:meta.lfs.oid,bytes:meta.size},null,2));
    } else d = await json(api);
    if (d.rows?.length !== 10) throw new Error(`Expected ten fixed rows for ${config}`);
    for (const {row_idx, row} of d.rows) {
      const id = `fleurs-${config}-validation-${row_idx}-${row.id}`;
      const source = join(root, `${id}-source.wav`), wav = join(root, `${id}.wav`);
      if (row.audio[0].local) await import('node:fs/promises').then(m => m.copyFile(row.audio[0].local, source));
      else await download(row.audio[0].src, source);
      execFileSync('ffmpeg', ['-hide_banner', '-loglevel', 'error', '-y', '-i', source, '-ar', '16000', '-ac', '1', '-c:a', 'pcm_s16le', wav]);
      const info = JSON.parse(execFileSync('ffprobe', ['-v', 'error', '-show_entries', 'format=duration', '-of', 'json', wav]));
      const duration = Number(info.format.duration);
      if (!(duration > 0 && duration <= 60)) throw new Error('Unexpected audio duration');
      rows.push({id, config, language, row_idx, reference: row.transcription, wav: `${id}.wav`, audio_seconds: duration, sha256: await sha(wav), source_sha256: await sha(source), purpose: 'accuracy', api});
      console.log(`${id} ${duration}s`);
    }
  }
  for (const language of ['Japanese','English','Thai']) {
    const first = rows.find(r => r.language === language && r.audio_seconds >= 8);
    if (!first) throw new Error('No >=8s source for latency prefixes');
    for (const seconds of [2,4,8]) {
      const id = `latency-${language.toLowerCase()}-${seconds}s`;
      const wav = join(root, `${id}.wav`);
      execFileSync('ffmpeg', ['-hide_banner', '-loglevel', 'error', '-y', '-i', join(root, first.wav), '-t', String(seconds), '-c:a', 'pcm_s16le', wav]);
      const info = JSON.parse(execFileSync('ffprobe', ['-v','error','-show_entries','format=duration','-of','json',wav]));
      rows.push({id, language, wav: `${id}.wav`, audio_seconds: Number(info.format.duration), reference: null, purpose: 'latency-prefix', source_id: first.id, sha256: await sha(wav)});
    }
  }
  const silence = join(root, 'silence-5s.wav');
  execFileSync('ffmpeg', ['-hide_banner','-loglevel','error','-y','-f','lavfi','-i','anullsrc=r=16000:cl=mono','-t','5','-c:a','pcm_s16le',silence]);
  rows.push({id:'silence-5s', language:'Japanese', wav:'silence-5s.wav',audio_seconds:5,reference:'',purpose:'silence',sha256:await sha(silence)});
  await writeFile(join(root, 'manifest.json'), JSON.stringify({dataset:'google/fleurs',split:'validation',revision:null,license:'CC-BY-4.0',attribution:'Conneau et al., FLEURS (2022)',rows}, null, 2));
  await writeFile(join(root,'inputs.tsv'),rows.map(r=>[r.id,r.language,join(root,r.wav)].join('\t')).join('\n')+'\n');
} else throw new Error('Use: prepare.mjs <external-artifact-dir> models|audio');
