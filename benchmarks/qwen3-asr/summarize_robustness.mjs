// Keep missed speech, review-required text and negative insertions in scoring.
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { normalize, distance } from '../../frontend/scripts/asr-metrics.mjs';
import { words } from './metrics.mjs';

const dir = resolve(process.argv[2]);
const manifest = JSON.parse(readFileSync(join(dir, 'manifest.json'), 'utf8'));
const input = readFileSync(join(dir, 'results.jsonl'), 'utf8').trim().split('\n').map(JSON.parse);
const byId = new Map(manifest.rows.map(r => [r.id, r]));
if (byId.size !== manifest.rows.length) throw new Error('Duplicate manifest ID');
const seen = new Set();
const rows = input.map(r => {
  const source = byId.get(r.id);
  if (!source || seen.has(r.id)) throw new Error('Unknown or duplicate result ID');
  seen.add(r.id);
  if (r.fixture_sha256 !== source.sha256 || r.language !== source.language ||
      r.reference !== source.reference || r.condition !== source.condition ||
      Math.abs(r.audio_seconds - source.audio_seconds) > .0001) {
    throw new Error('Fixture metadata mismatch');
  }
  const ref = normalize(source.reference), text = normalize(r.text);
  const negative = source.purpose === 'silence';
  return {...r, purpose: source.purpose, reference_characters: Array.from(ref).length,
    character_errors: distance(ref, text), reference_words: words(source.reference).length,
    word_errors: distance(words(source.reference), words(r.text)),
    normalized_exact_match: ref === text,
    negative_gate_pass: negative ? r.worker.status === 'no_speech' &&
      r.worker.asr_calls === 0 && r.text === '' : null};
});
if (seen.size !== byId.size) throw new Error('Incomplete results: no silent exclusion permitted');
const groups = [];
const speech = rows.filter(r => r.purpose === 'accuracy');
for (const language of new Set(speech.map(r => r.language))) {
  for (const condition of new Set(speech.filter(r => r.language === language).map(r => r.condition))) {
    const rr = speech.filter(r => r.language === language && r.condition === condition);
    const chars = rr.reduce((n, r) => n + r.reference_characters, 0);
    const errors = rr.reduce((n, r) => n + r.character_errors, 0);
    const wc = rr.reduce((n, r) => n + r.reference_words, 0);
    const we = rr.reduce((n, r) => n + r.word_errors, 0);
    groups.push({language, condition, clips: rr.length,
      missed_speech: rr.filter(r => r.worker.status === 'no_speech').map(r => r.id),
      worker_errors: rr.filter(r => r.status !== 'ok').map(r => r.id),
      cer: chars ? errors / chars : null, character_errors: errors, reference_characters: chars,
      wer: language === 'English' && wc ? we / wc : null,
      normalized_exact_matches: rr.filter(r => r.normalized_exact_match).length});
  }
}
const negatives = rows.filter(r => r.purpose === 'silence');
const report = {normalization: 'Existing CER NFKC/lowercase/remove Unicode punctuation+whitespace; English WER punctuation removal and whitespace words. Raw text retained.',
  complete: true, total_requests: rows.length, speech_requests: speech.length,
  worker_errors: rows.filter(r => r.status !== 'ok').map(r => r.id),
  negative_requests: negatives.length,
  negative_gate_passes: negatives.filter(r => r.negative_gate_pass).length,
  negative_failures: negatives.filter(r => !r.negative_gate_pass).map(r => ({id: r.id, text: r.text, status: r.worker.status, asr_calls: r.worker.asr_calls})),
  quality_accepted: false, real_environment_noise_verified: false,
  spontaneous_acknowledgements_verified: false, application_integrated: false,
  groups, rows};
writeFileSync(join(dir, 'report.json'), JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify({...report, rows: undefined}, null, 2));
