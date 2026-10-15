import test from 'node:test';
import assert from 'node:assert/strict';
import {summarize} from './summarize_current_validation.mjs';

function fixture() {
  const rows=Array.from({length:14},(_,i)=>({id:String(i),suite:i===0||i===13?'new-negatives':'new-lexical',condition:i===0||i===13?'silence':'clean',reference:i===0||i===13?'':'本',sha256:String(i),audio_seconds:1}));
  const a=rows.map(r=>({id:r.id,reference:r.reference,fixture_sha256:r.sha256,audio_seconds:1,text:r.reference?'本。':'',existing_v4_full_input_control:true,current_vad_used:false,worker:{status:r.reference?'review_required':'no_speech',text:r.reference?'本。':'',asr_calls:r.reference?1:0,confirmed:false,segments:[],inference_seconds:1}}));
  return [{rows},a,structuredClone(a)];
}
test('scores raw short-word failure, keeps review and ignores timings only',()=>{
  const [m,a,b]=fixture();
  a[1].text=a[1].worker.text=b[1].text=b[1].worker.text='ほう。';
  b[1].worker.inference_seconds=999;
  const r=summarize(m,a,b);
  assert.equal(r.normalized_matches,11);
  assert.equal(r.character_errors,2);
  assert.equal(r.review_required,12);
  assert.equal(r.current_vad_used,false);
  assert.equal(r.failed_rows[0].text,'ほう。');
});
test('rejects missing/duplicate controls, wrong hash, pipeline and reference',()=>{
  for(const mutate of [a=>a.pop(),a=>a[1].id=a[0].id,a=>a[1].fixture_sha256='bad',a=>a[1].reference='ほう',a=>a[1].current_vad_used=true]) {
    const [m,a,b]=fixture();mutate(a);assert.throws(()=>summarize(m,a,b));
  }
});
test('different text/status across fresh workers is not labeled reproducible',()=>{
  const [m,a,b]=fixture();b[1].text=b[1].worker.text='ほう。';
  assert.throws(()=>summarize(m,a,b),/reproduce/);
});
