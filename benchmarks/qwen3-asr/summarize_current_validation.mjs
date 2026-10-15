import {readFileSync,writeFileSync} from 'node:fs';
import {pathToFileURL} from 'node:url';
import {normalize,distance} from '../../frontend/scripts/asr-metrics.mjs';

export function summarize(manifest,a,b) {
  const expected=manifest.rows.filter(r=>r.suite==='new-lexical'||r.condition==='silence');
  if(expected.length!==14||a.length!==14||b.length!==14) throw Error('Missing/extra controls');
  for(const records of [a,b]) {
    if(new Set(records.map(r=>r.id)).size!==14) throw Error('Duplicate controls');
    records.forEach((r,i)=>{
      const source=expected[i];
      if(r.id!==source.id||r.reference!==source.reference||r.fixture_sha256!==source.sha256||r.audio_seconds!==source.audio_seconds) throw Error('Fixture/reference mismatch');
      if(r.existing_v4_full_input_control!==true||r.current_vad_used!==false) throw Error('Wrong pipeline');
      if(!['recognized','review_required','no_speech'].includes(r.worker.status)||typeof r.text!=='string'||r.text!==r.worker.text) throw Error('Invalid/incomplete inference');
    });
  }
  const signature=r=>JSON.stringify([r.id,r.fixture_sha256,r.text,r.worker.status,r.worker.asr_calls,r.worker.confirmed,r.worker.gate_intervals,r.worker.segments.map(s=>s.review_reasons)]);
  if(a.some((r,i)=>signature(r)!==signature(b[i]))) throw Error('ASR control did not reproduce');
  const speech=a.filter(r=>r.reference);
  const errors=speech.reduce((n,r)=>n+distance(normalize(r.reference),normalize(r.text)),0);
  const chars=speech.reduce((n,r)=>n+Array.from(normalize(r.reference)).length,0);
  return {existing_v4_full_input_control:true,current_vad_used:false,
    normalization:'Existing CER: NFKC/lowercase/remove Unicode punctuation+whitespace; no lexical substitutions',
    speech_windows:speech.length,normalized_matches:speech.filter(r=>normalize(r.text)===normalize(r.reference)).length,
    character_errors:errors,reference_characters:chars,cer:errors/chars,
    review_required:speech.filter(r=>r.worker.status==='review_required').length,
    all_unconfirmed:a.every(r=>r.worker.confirmed===false),
    silences:a.filter(r=>!r.reference).map(r=>({id:r.id,text:r.text,asr_calls:r.worker.asr_calls})),
    fresh_worker_passes:2,reproduced_text_status_gate_review:true,
    failed_rows:speech.filter(r=>normalize(r.text)!==normalize(r.reference))};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href) {
  const [manifest,first,second,out]=process.argv.slice(2);
  const jsonl=path=>readFileSync(path,'utf8').trim().split('\n').map(JSON.parse);
  const report=summarize(JSON.parse(readFileSync(manifest,'utf8')),jsonl(first),jsonl(second));
  if(out) writeFileSync(out,JSON.stringify(report,null,2)+'\n');
  else console.log(JSON.stringify(report,null,2));
}
