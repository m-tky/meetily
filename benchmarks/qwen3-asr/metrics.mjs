import { readFileSync, writeFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import { normalize, distance } from '../../frontend/scripts/asr-metrics.mjs';
export function words(text) {
  const s=text.normalize('NFKC').toLowerCase().replace(/\p{P}/gu,'').trim();
  return s ? s.split(/\p{White_Space}+/u) : [];
}
export function summarize(manifest, input) {
  const byId=new Map(manifest.rows.map(r=>[r.id,r]));
  const rows=input.map(r=>{
    const source=byId.get(r.id); if(!source) throw new Error('Unknown fixture ID');
    if(r.status!=='ok'||!Number.isFinite(r.elapsed_seconds)||r.elapsed_seconds<0) throw new Error('Invalid/incomplete inference');
    if(Math.abs(r.audio_seconds-source.audio_seconds)>0.001) throw new Error('Audio duration mismatch');
    // Candle's forced-language API leaves the ASR protocol separator in text.
    // Exclude ONLY that exact leading control marker, keep raw evidence.
    const raw_text=r.raw_text??r.text;
    const text=r.engine==='candle-rust' ? r.text.replace(/^<asr_text>/u,'').trim() : r.text;
    const ref=source.reference;
    const chars=ref===null?null:Array.from(normalize(ref)).length;
    const errors=ref===null?null:distance(normalize(ref),normalize(text));
    const refWords=ref===null?[]:words(ref);
    return {...r,text,raw_text,purpose:source.purpose,reference:ref,cer:chars?errors/chars:null,
      character_errors:errors,reference_characters:chars,
      word_errors:ref===null?null:distance(refWords,words(text)),reference_words:refWords.length,
      rtf:r.elapsed_seconds/r.audio_seconds};
  });
  const groups=[];
  for(const language of ['Japanese','English','Thai']) {
    const selected=rows.filter(r=>r.language===language&&r.purpose==='accuracy'&&r.rep===0);
    const expected=manifest.rows.filter(r=>r.language===language&&r.purpose==='accuracy');
    if(new Set(selected.map(r=>r.id)).size!==selected.length) throw new Error('Duplicate accuracy rows');
    const chars=selected.reduce((n,r)=>n+r.reference_characters,0), errors=selected.reduce((n,r)=>n+r.character_errors,0);
    const wc=selected.reduce((n,r)=>n+r.reference_words,0), we=selected.reduce((n,r)=>n+r.word_errors,0);
    const audio=selected.reduce((n,r)=>n+r.audio_seconds,0), elapsed=selected.reduce((n,r)=>n+r.elapsed_seconds,0);
    groups.push({language,clips:selected.length,expected_clips:expected.length,complete:selected.length===expected.length,
      cer:chars?errors/chars:null,character_errors:errors,reference_characters:chars,
      wer:language==='English'&&wc?we/wc:null,word_errors:language==='English'?we:null,
      audio_seconds:audio,elapsed_seconds:elapsed,rtf:audio?elapsed/audio:null});
  }
  const latency=rows.filter(r=>r.purpose==='latency-prefix');
  const latencyGroups=[];
  for(const id of new Set(latency.map(r=>r.id))) {
    const rr=latency.filter(r=>r.id===id);
    const sorted=rr.map(r=>r.elapsed_seconds).sort((a,b)=>a-b);
    latencyGroups.push({id,runs:rr.length,audio_seconds:rr[0].audio_seconds,min_seconds:sorted[0],
      median_seconds:sorted[Math.floor(sorted.length/2)],max_seconds:sorted.at(-1),
      first_token_seconds:rr.map(r=>r.first_token_seconds),raw_nonempty_runs:rr.filter(r=>r.text.trim()).length});
  }
  const silence=rows.filter(r=>r.purpose==='silence');
  return {normalization:'CER: NFKC/lower-case/remove Unicode punctuation+whitespace, codepoints (not graphemes); English WER: punctuation removed, whitespace words',
    groups,latency:latencyGroups,load_seconds:rows[0]?.load_seconds??null,
    peak_rss_mib:rows.length?Math.max(...rows.map(r=>r.peak_rss_kib))/1024:null,
    silence:silence.map(r=>({text:r.text,elapsed_seconds:r.elapsed_seconds,character_insertions:r.character_errors})),rows};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href) {
  const m=JSON.parse(readFileSync(process.argv[2],'utf8'));
  const rows=readFileSync(process.argv[3],'utf8').trim().split('\n').filter(Boolean).map(JSON.parse);
  const result=summarize(m,rows);
  if(process.argv[4]) writeFileSync(process.argv[4],JSON.stringify(result,null,2)+'\n');
  else console.log(JSON.stringify(result,null,2));
}
