import test from 'node:test';
import assert from 'node:assert/strict';
import {score,summarize} from './summarize_float_asr.mjs';
function fixture() {
  const rows=[{id:'a',reference:'いいえ',language:'Japanese',samples:16000,sha256:'a',role:'speech',gate_detected:true,suite:'quiet',representation:'float32',pair_id:'pair'},
    {id:'b',reference:'いいえ',language:'Japanese',samples:16000,sha256:'b',role:'speech',gate_detected:false,suite:'quiet',representation:'pcm16',pair_id:'pair'},
    {id:'n',reference:'',language:'Japanese',samples:16000,sha256:'n',role:'negative',gate_detected:false,suite:'negative',representation:'existing',pair_id:null}];
  const a=rows.map(s=>({id:s.id,reference:s.reference,fixture_sha256:s.sha256,input_samples:s.samples,
    text:s.role==='speech'?'いいえ。':'',asr_calls:Number(s.role==='speech'),gate_detected:s.gate_detected,
    known_speech_diagnostic_bypass:s.role==='speech'&&!s.gate_detected,confirmed:false,asr_input_gain:1,threads:6,
    precision:'decoder-INT8-encoder-prefill-F32',inference_seconds:1,review_reasons:[],text_tokens:3}));
  return [{rows},a,structuredClone(a)];
}
test('diagnosis and gated projection are distinct; missing gate counts deletions',()=>{
  const [m,a,b]=fixture(),r=summarize(m,a,b);
  assert.equal(r.actual_asr_calls_first_pass,2);
  assert.equal(r.rows[1].diagnostic_score.character_errors,0);
  assert.equal(r.rows[1].projected_score.character_errors,3);
  assert.equal(r.rows[1].projection_not_an_additional_actual_call,true);
  assert.equal(r.negative_nonempty.length,0);
});
test('word substitution is not normalized away and contractions stay raw-scored',()=>{
  assert.equal(score('いいえ','家。','Japanese').character_errors,3);
  assert.equal(score('you are',"you're",'English').word_errors,2);
});
test('missing/hash/gate mismatch and negative bypass reject',()=>{
  for(const mutate of [a=>a.pop(),a=>a[0].fixture_sha256='bad',a=>a[0].gate_detected=false,a=>a[2].known_speech_diagnostic_bypass=true]) {
    const [m,a,b]=fixture();mutate(a);assert.throws(()=>summarize(m,a,b));
  }
});
test('repeat differences are retained, not falsely called reproducible',()=>{
  const [m,a,b]=fixture();b[0].text='家。';
  assert.deepEqual(summarize(m,a,b).repeat_unstable_ids,['a']);
});
