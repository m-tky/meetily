import {mkdir,writeFile} from 'node:fs/promises';
import {resolve,join,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {rangeDownload,hash} from './download.mjs';
const root=resolve(process.argv[2]??'../runtime/qwen3-asr/artifacts');
const repo=resolve(dirname(fileURLToPath(import.meta.url)),'../..');
if(root===repo||root.startsWith(repo+'/'))throw Error('Keep model artifacts outside repository');
for(const size of ['0.6b','1.7b']){
 const id=`cstr/qwen3-asr-${size}-GGUF`;
 const response=await fetch(`https://huggingface.co/api/models/${id}?blobs=true`,{signal:AbortSignal.timeout(60000)});
 if(!response.ok)throw Error('Model metadata failed');
 const meta=await response.json();const filename=`qwen3-asr-${size}-q4_k.gguf`;
 const file=meta.siblings.find(f=>f.rfilename===filename);
 if(!file?.lfs?.sha256||!meta.sha)throw Error('Missing provenance');
 const dir=join(root,`q4-${size}`);await mkdir(dir,{recursive:true});
 const url=`https://huggingface.co/${id}/resolve/${meta.sha}/${filename}`;
 await rangeDownload(url,join(dir,filename),file.lfs.sha256,file.size);
 await writeFile(join(dir,'provenance.json'),JSON.stringify({id,revision:meta.sha,base_model:meta.cardData.base_model,
   original_base_revision:null,license:meta.cardData.license,filename,bytes:file.size,sha256:await hash(join(dir,filename)),
   note:'Third-party conversion. Original source revision not published. Mixed Q4/Q8/F16/F32 must be inspected; not all tensors INT4.'},null,2)+'\n');
 console.error(size+' Q4 verified');
}
