import test from 'node:test';
import assert from 'node:assert/strict';
import { words, summarize } from './metrics.mjs';
const manifest={rows:[
 {id:'j',language:'Japanese',purpose:'accuracy',reference:'会議。',audio_seconds:2},
 {id:'e',language:'English',purpose:'accuracy',reference:"Don't stop.",audio_seconds:3},
 {id:'t',language:'Thai',purpose:'accuracy',reference:'สวัสดี',audio_seconds:4},
 {id:'s',language:'Japanese',purpose:'silence',reference:'',audio_seconds:5},
 {id:'l',language:'Japanese',purpose:'latency-prefix',reference:null,audio_seconds:2},
]};
const row=(id,text,seconds)=>({id,text,audio_seconds:seconds,elapsed_seconds:0.5,language:manifest.rows.find(r=>r.id===id).language,status:'ok',rep:0,peak_rss_kib:1024,load_seconds:1,first_token_seconds:0.2});
test('English WER uses whitespace words and preserves contractions as one word',()=>{
 assert.deepEqual(words("DON'T stop!"),['dont','stop']);
 const report=summarize(manifest,[row('e','Do stop',3)]);
 assert.equal(report.groups.find(g=>g.language==='English').wer,0.5);
});
test('Thai keeps combining marks; silence is not mixed into speech CER',()=>{
 const report=summarize(manifest,[row('j','会議',2),row('t','สวัสดี',4),row('s','。',5)]);
 assert.equal(report.groups.find(g=>g.language==='Japanese').cer,0);
 assert.equal(report.groups.find(g=>g.language==='Thai').cer,0);
 assert.equal(report.silence[0].text,'。');
 assert.equal(report.peak_rss_mib,1);
});
test('prefix-only audio is timing-only; missing accuracy rows are explicit',()=>{
 const report=summarize(manifest,[row('l','会議',2)]);
 assert.equal(report.rows[0].cer,null);
 assert.equal(report.latency[0].median_seconds,0.5);
 assert.equal(report.groups[0].complete,false);
});
test('Candle control separator is not counted as a spoken recognition error; raw output retained',()=>{
 const report=summarize(manifest,[{...row('j','<asr_text>会議',2),engine:'candle-rust'}]);
 assert.equal(report.groups[0].cer,0);
 assert.equal(report.rows[0].raw_text,'<asr_text>会議');
});
test('unknown ID, mismatched duration, failures and duplicates reject',()=>{
 assert.throws(()=>summarize(manifest,[row('j','',3)]));
 assert.throws(()=>summarize(manifest,[{...row('j','',2),status:'error'}]));
 assert.throws(()=>summarize(manifest,[{...row('j','',2),id:'unknown'}]));
 assert.throws(()=>summarize(manifest,[row('j','',2),row('j','',2)]));
});
