import { stat, mkdir, rename, unlink, rm } from 'node:fs/promises';
import { createReadStream, createWriteStream } from 'node:fs';
import { pipeline } from 'node:stream/promises';
import { Readable } from 'node:stream';
import { createHash } from 'node:crypto';
export async function hash(path) {
  const h = createHash('sha256');
  for await (const b of createReadStream(path)) h.update(b);
  return h.digest('hex');
}
export async function rangeDownload(url, path, expectedHash, size) {
  if ((await stat(path).catch(() => null))?.size === size && await hash(path) === expectedHash) return;
  const dir = path + '.chunks'; await mkdir(dir, {recursive:true});
  const chunkSize = 64 * 1024 * 1024, count = Math.ceil(size/chunkSize);
  let next = 0;
  await Promise.all(Array.from({length:Math.min(8,count)}, async () => {
    while (next < count) {
      const i = next++, start = i*chunkSize, end = Math.min(size-1, start+chunkSize-1);
      const file = `${dir}/${i}`;
      if ((await stat(file).catch(()=>null))?.size === end-start+1) continue;
      for(let attempt=0;attempt<3;attempt++) {
        try {
          let partial = (await stat(file+'.part').catch(()=>null))?.size ?? 0;
          if (partial > end-start+1) {await unlink(file+'.part');partial=0;}
          if (partial === end-start+1) {await rename(file+'.part',file);break;}
          const resume = start + partial;
          const r = await fetch(url + `?download=true&part=${i}&retry=${attempt}`, {headers:{Range:`bytes=${resume}-${end}`},signal:AbortSignal.timeout(300000)});
          if(r.status!==206 || r.headers.get('content-range')!==`bytes ${resume}-${end}/${size}`) {
            await r.body?.cancel(); throw new Error('Incorrect HTTP Range response');
          }
          await pipeline(Readable.fromWeb(r.body), createWriteStream(file+'.part', {flags:partial?'a':'w'}));
          if((await stat(file+'.part')).size!==end-start+1) throw new Error('Truncated chunk');
          await rename(file+'.part',file);
          console.error(`Verified range ${i+1}/${count}`); break;
        } catch(e) {
          // Preserve verified-range partial bytes for bounded resume. The
          // assembled artifact still MUST match the official whole-file hash.
          if(attempt===2) throw e;
        }
      }
    }
  }));
  async function* all() { for(let i=0;i<count;i++) for await(const b of createReadStream(`${dir}/${i}`)) yield b; }
  await pipeline(Readable.from(all()),createWriteStream(path+'.assembled'));
  if(await hash(path+'.assembled')!==expectedHash) throw new Error('Final SHA256 mismatch');
  await rename(path+'.assembled',path);
  await rm(dir,{recursive:true});
}
