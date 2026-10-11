// Download public data/models only. No microphone, playback, app state, or cloud ASR.
// Artifacts belong OUTSIDE the repo. Requires Node 18+ and ffmpeg on PATH.
import { mkdir, writeFile, rename, unlink, stat } from 'node:fs/promises';
import { createReadStream, createWriteStream } from 'node:fs';
import { Readable } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import { createHash } from 'node:crypto';
import { resolve, join, relative, isAbsolute, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

async function get(url, timeout = 180_000) {
  const r = await fetch(url, { signal: AbortSignal.timeout(timeout) });
  if (!r.ok) throw new Error(`HTTP ${r.status} from public fixture source`);
  return r;
}
async function hash(path) {
  const h = createHash('sha256');
  for await (const chunk of createReadStream(path)) h.update(chunk);
  return h.digest('hex');
}
async function download(url, path, expectedHash) {
  if (expectedHash && await stat(path).catch(() => null) && await hash(path) === expectedHash) return;
  const temp = `${path}.part`;
  try {
    const response = await get(url, 1_200_000);
    await pipeline(Readable.fromWeb(response.body), createWriteStream(temp));
    if (expectedHash && await hash(temp) !== expectedHash) throw new Error('SHA256 mismatch');
    await rename(temp, path);
  } finally { await unlink(temp).catch(() => {}); }
}

const root = resolve(process.argv[2] ?? '../runtime/asr');
const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const rel = relative(repository, root);
if (rel === '' || (!rel.startsWith(`..${process.platform === 'win32' ? '\\' : '/'}`) && rel !== '..' && !isAbsolute(rel))) {
  throw new Error('Keep bulky fixture artifacts outside the repository');
}
await mkdir(join(root, 'models'), { recursive: true });
const dataset = 'google/fleurs', config = 'ja_jp', split = 'validation';
const api = `https://datasets-server.huggingface.co/rows?dataset=${dataset}&config=${config}&split=${split}&offset=0&length=3`;
const data = await (await get(api)).json();
if (data.rows?.length !== 3) throw new Error('Expected exactly three fixed rows');
const rows = [];
for (const { row_idx, row } of data.rows) {
  const stem = `fleurs-ja-validation-${row_idx}-${row.id}`;
  const wav = join(root, `${stem}.wav`), pcm = join(root, `${stem}.f32`);
  await download(row.audio[0].src, wav);
  execFileSync('ffmpeg', ['-hide_banner', '-loglevel', 'error', '-y', '-i', wav,
    '-ac', '1', '-ar', '16000', '-f', 'f32le', pcm]);
  const samples = (await stat(pcm)).size / 4;
  if (!Number.isInteger(samples) || samples <= 0 || samples > 60 * 16000) throw new Error('Unexpected audio length');
  rows.push({ id: stem, reference: row.transcription, audio: `${stem}.f32`,
    samples, wav_sha256: await hash(wav), pcm_sha256: await hash(pcm) });
}
await writeFile(join(root, 'silence.f32'), Buffer.alloc(5 * 16000 * 4));
const silence = { id: 'synthetic-silence-5s', reference: '', audio: 'silence.f32',
  samples: 5 * 16000, pcm_sha256: await hash(join(root, 'silence.f32')) };
const manifest = { dataset, config, split, api, revision: data.dataset_git_revision ?? null,
  license: 'CC-BY-4.0 (FLEURS dataset card)', attribution: 'Conneau et al., FLEURS (2022)',
  rows: [...rows, silence] };
await writeFile(join(root, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n');
const info = await (await get('https://huggingface.co/api/models/ggerganov/whisper.cpp?blobs=true')).json();
const models = [];
for (const name of ['base', 'small']) {
  const file = `ggml-${name}.bin`;
  const meta = info.siblings.find(x => x.rfilename === file);
  if (!meta?.lfs?.sha256 || meta.size > 500_000_000) throw new Error('Missing model integrity/size metadata');
  const url = `https://huggingface.co/ggerganov/whisper.cpp/resolve/${info.sha}/${file}`;
  console.error(`Downloading/verifying ${name} (${meta.size} bytes)`);
  await download(url, join(root, 'models', file), meta.lfs.sha256);
  models.push({ name, revision: info.sha, url, bytes: meta.size, sha256: meta.lfs.sha256 });
}
await writeFile(join(root, 'models.json'), JSON.stringify(models, null, 2) + '\n');
console.log(JSON.stringify({ root, rows: manifest.rows.length, models: models.map(m => m.name) }));
