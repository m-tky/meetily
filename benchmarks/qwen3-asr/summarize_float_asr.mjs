import {readFileSync,writeFileSync} from 'node:fs';
import {pathToFileURL} from 'node:url';
import {normalize,distance} from '../../frontend/scripts/asr-metrics.mjs';
import {words} from './metrics.mjs';

export function score(reference,text,language) {
  return {character_errors:distance(normalize(reference),normalize(text)),
    reference_characters:Array.from(normalize(reference)).length,
    word_errors:language==='English'?distance(words(reference),words(text)):null,
    reference_words:language==='English'?words(reference).length:null};
}
export function validateResponses(manifest,records) {
  const source=new Map(manifest.rows.map(r=>[r.id,r]));
  if(records.length!==source.size||new Set(records.map(r=>r.id)).size!==source.size) throw Error('Incomplete/duplicate responses');
  for(const r of records) {
    const s=source.get(r.id);if(!s)throw Error('Unknown response');
    if(r.input_samples!==s.samples||r.gate_detected!==s.gate_detected||r.asr_calls!==Number(s.role==='speech'||s.gate_detected))throw Error('Input/gate/call mismatch');
    if(r.fixture_sha256!==s.sha256||r.reference!==s.reference)throw Error('Hash/reference mismatch');
    if(r.confirmed!==false||r.asr_input_gain!==1||r.threads!==6||r.precision!=='decoder-INT8-encoder-prefill-F32')throw Error('Unexpected ASR policy');
    if(r.known_speech_diagnostic_bypass!==(s.role==='speech'&&!s.gate_detected))throw Error('Unauthorized bypass');
    if(typeof r.text!=='string'||!Number.isFinite(r.inference_seconds)||r.inference_seconds<0)throw Error('Incomplete inference');
    if(r.asr_calls===0&&r.text!=='')throw Error('Text without ASR');
  }
}
export function summarize(manifest,first,second,fixed=[]) {
  validateResponses(manifest,first);validateResponses(manifest,second);
  const byid=new Map(manifest.rows.map(r=>[r.id,r]));
  const rows=first.map(r=>{
    const s=byid.get(r.id), projected=s.gate_detected?r.text:'';
    return {...r,language:s.language,suite:s.suite,representation:s.representation,pair_id:s.pair_id,
      gate_missed:s.role==='speech'&&!s.gate_detected,raw_text:r.text,
      diagnostic_score:score(s.reference,r.text,s.language),
      projected_text:projected,projected_score:score(s.reference,projected,s.language),
      projection_not_an_additional_actual_call:true};
  });
  const groups=[];
  for(const suite of new Set(rows.map(r=>r.suite)))for(const fmt of new Set(rows.filter(r=>r.suite===suite).map(r=>r.representation)))for(const language of ['Japanese','English','Thai']) {
    const rr=rows.filter(r=>r.suite===suite&&r.representation===fmt&&r.language===language);if(!rr.length)continue;
    const metrics={};
    for(const key of ['diagnostic_score','projected_score']) {
      const chars=rr.reduce((n,r)=>n+r[key].reference_characters,0),errors=rr.reduce((n,r)=>n+r[key].character_errors,0);
      const wc=rr.reduce((n,r)=>n+(r[key].reference_words??0),0),we=rr.reduce((n,r)=>n+(r[key].word_errors??0),0);
      metrics[key]={character_errors:errors,reference_characters:chars,cer:chars?errors/chars:null,word_errors:language==='English'?we:null,reference_words:language==='English'?wc:null,wer:wc?we/wc:null};
    }
    groups.push({suite,representation:fmt,language,windows:rr.length,gate_misses:rr.filter(r=>r.gate_missed).length,actual_asr_calls:rr.reduce((n,r)=>n+r.asr_calls,0),...metrics});
  }
  const paired=[];
  for(const pair of new Set(rows.map(r=>r.pair_id).filter(Boolean))) {
    const rr=rows.filter(r=>r.pair_id===pair);if(rr.length!==2)throw Error('Incomplete pair');
    const f=rr.find(r=>r.representation==='float32'),p=rr.find(r=>r.representation==='pcm16');if(!f||!p)throw Error('Bad representation pair');
    const key=f.language==='English'?'word_errors':'character_errors';
    paired.push({pair_id:pair,suite:f.suite,language:f.language,float32_text:f.text,pcm16_text:p.text,
      diagnostic_error_delta_float_minus_pcm16:f.diagnostic_score[key]-p.diagnostic_score[key],
      projected_error_delta_float_minus_pcm16:f.projected_score[key]-p.projected_score[key],
      float32_gate:f.gate_detected,pcm16_gate:p.gate_detected});
  }
  const b=new Map(second.map(r=>[r.id,r]));
  const signature=r=>JSON.stringify([r.text,r.asr_calls,r.gate_detected,r.review_reasons,r.text_tokens,r.confirmed]);
  const unstable=first.filter(r=>signature(r)!==signature(b.get(r.id))).map(r=>r.id);
  const baseline=rows.filter(r=>r.suite==='baseline'&&r.id.startsWith('fleurs-'));
  const fixedMap=new Map(fixed.map(r=>[r.id,r.text]));
  const baselineMismatch=baseline.filter(r=>fixedMap.size&&fixedMap.get(r.id.replace(/-baseline$/,''))!==r.text).map(r=>r.id);
  return {normalization:'Existing CER/WER; no lexical/digit/synonym corrections',
    actual_asr_calls_first_pass:first.reduce((n,r)=>n+r.asr_calls,0),fresh_processes:2,
    repeat_stable_windows:first.length-unstable.length,repeat_unstable_ids:unstable,
    baseline_fleurs_windows:baseline.length,baseline_fixed_text_mismatch:baselineMismatch,
    groups,pairs:paired,rows,all_unconfirmed:first.every(r=>!r.confirmed),
    negative_nonempty:rows.filter(r=>r.reference===''&&r.text!==''),
    gated_scores_are_projections:true,model_replaced:false,application_integrated:false,
    correction_implemented:false,quality_accepted:false};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href) {
  const [manifest,a,b,fixed,out]=process.argv.slice(2);
  const jsonl=p=>readFileSync(p,'utf8').trim().split('\n').map(JSON.parse);
  const result=summarize(JSON.parse(readFileSync(manifest,'utf8')),jsonl(a),jsonl(b),jsonl(fixed));
  if(out)writeFileSync(out,JSON.stringify(result,null,2)+'\n');else console.log(JSON.stringify(result,null,2));
}
