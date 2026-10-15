"""Download only pinned official public ForcedAligner artifacts, verify Git/LFS hashes."""
import argparse
import hashlib
import json
from pathlib import Path
import urllib.request

REPO='Qwen/Qwen3-ForcedAligner-0.6B'
REV='c7cbfc2048c462b0d63a45797104fc9db3ad62b7'
FILES=('config.json','generation_config.json','merges.txt','model.safetensors',
       'preprocessor_config.json','tokenizer_config.json','vocab.json','README.md')


def sha(p):
    h=hashlib.sha256()
    with p.open('rb') as f:
        while b:=f.read(8*1024**2):h.update(b)
    return h.hexdigest()


def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('directory',type=Path)
    args=ap.parse_args();d=args.directory.resolve()
    app=Path(__file__).resolve().parents[2]
    if d==app or app in d.parents:ap.error('Model must stay outside app checkout')
    req=urllib.request.Request(f'https://huggingface.co/api/models/{REPO}/revision/{REV}?blobs=true',headers={'User-Agent':'local-public-aligner-probe'})
    with urllib.request.urlopen(req,timeout=60) as r:meta=json.load(r)
    if meta['sha']!=REV:raise ValueError('Pinned revision changed')
    siblings={x['rfilename']:x for x in meta['siblings']};d.mkdir(parents=True,exist_ok=True)
    rows=[]
    for name in FILES:
        info=siblings[name];p=d/name
        if not p.exists():
            part=d/(name+'.part')
            req=urllib.request.Request(f'https://huggingface.co/{REPO}/resolve/{REV}/{name}?download=true',headers={'User-Agent':'local-public-aligner-probe'})
            if info.get('lfs') and info['size']>100*1024**2:
                from concurrent.futures import ThreadPoolExecutor
                total=info['size'];count=8;step=(total+count-1)//count
                def chunk(i):
                    lo=i*step;hi=min(total,(i+1)*step)-1
                    pchunk=d/(name+f'.chunk-{i}')
                    import time
                    for attempt in range(12):
                        have=pchunk.stat().st_size if pchunk.exists() else 0
                        if have==hi-lo+1:return pchunk
                        if have>hi-lo+1:raise ValueError('Range file exceeds bound')
                        begin=lo+have
                        url=f'https://huggingface.co/{REPO}/resolve/{REV}/{name}?download=true&chunk={i}&resume={begin}'
                        q=urllib.request.Request(url,headers={'Range':f'bytes={begin}-{hi}','User-Agent':'local-public-aligner-probe'})
                        try:
                            with urllib.request.urlopen(q,timeout=30) as r,pchunk.open('ab') as f:
                                if r.status!=206 or r.headers.get('Content-Range')!=f'bytes {begin}-{hi}/{total}':
                                    raise ValueError('Range response mismatch')
                                while b:=r.read(1024**2):f.write(b)
                        except (OSError,TimeoutError) as error:
                            print(json.dumps(dict(chunk=i,retry=attempt,error=type(error).__name__,retained_bytes=pchunk.stat().st_size if pchunk.exists() else 0)),flush=True)
                            time.sleep(1)
                    if pchunk.stat().st_size!=hi-lo+1:raise ValueError('Range retries exhausted')
                    return pchunk
                with ThreadPoolExecutor(max_workers=count) as pool:chunks=list(pool.map(chunk,range(count)))
                with part.open('wb') as f:
                    for pchunk in chunks:
                        with pchunk.open('rb') as r:
                            while b:=r.read(8*1024**2):f.write(b)
            else:
                with urllib.request.urlopen(req,timeout=120) as r,part.open('wb') as f:
                    while b:=r.read(8*1024**2):f.write(b)
            if part.stat().st_size!=info['size']:raise ValueError('Downloaded size mismatch: '+name)
            part.rename(p)
        if p.stat().st_size!=info['size']:raise ValueError('Size mismatch: '+name)
        h=sha(p)
        if info.get('lfs'):
            if h!=info['lfs']['sha256']:raise ValueError('LFS hash mismatch: '+name)
        else:
            raw=p.read_bytes();git=hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest()
            if git!=info['blobId']:raise ValueError('Git blob mismatch: '+name)
        rows.append(dict(file=name,sha256=h,bytes=p.stat().st_size,git_blob=info['blobId']))
        print(json.dumps(rows[-1]),flush=True)
    provenance=dict(repository=REPO,revision=REV,license=meta.get('cardData',{}).get('license'),files=rows)
    target=d/'provenance.json'
    if target.exists() and json.loads(target.read_text())!=provenance:raise ValueError('Existing provenance differs')
    target.write_text(json.dumps(provenance,indent=2)+'\n')


if __name__=='__main__':main()
