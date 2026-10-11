import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

// Explicit policy: NFKC, lower-case, remove Unicode punctuation/whitespace.
// Do not equate numerals with readings or remove currency/math symbols.
export function normalize(text) {
  return text.normalize('NFKC').toLowerCase().replace(/[\p{P}\p{White_Space}]/gu, '');
}

export function distance(reference, hypothesis) {
  const a = Array.from(reference), b = Array.from(hypothesis);
  let previous = Array.from({ length: b.length + 1 }, (_, i) => i);
  for (let i = 0; i < a.length; i++) {
    const next = [i + 1];
    for (let j = 0; j < b.length; j++) {
      next.push(Math.min(next[j] + 1, previous[j + 1] + 1,
        previous[j] + Number(a[i] !== b[j])));
    }
    previous = next;
  }
  return previous[b.length];
}

export function score(rows) {
  if (!rows.length) throw new Error('No ASR rows');
  const scored = rows.map(row => {
    if (!(row.audio_seconds > 0) || !(row.elapsed_seconds >= 0)
        || !Number.isFinite(row.audio_seconds) || !Number.isFinite(row.elapsed_seconds)) {
      throw new Error('Invalid duration');
    }
    const reference = normalize(row.reference), hypothesis = normalize(row.text);
    const characters = Array.from(reference).length;
    const errors = distance(reference, hypothesis);
    return { ...row, characters, errors, cer: characters ? errors / characters : null,
      rtf: row.elapsed_seconds / row.audio_seconds };
  });
  const characters = scored.reduce((n, r) => n + r.characters, 0);
  const errors = scored.reduce((n, r) => n + r.errors, 0);
  const audio = scored.reduce((n, r) => n + r.audio_seconds, 0);
  const elapsed = scored.reduce((n, r) => n + r.elapsed_seconds, 0);
  const speech = scored.filter(r => r.characters > 0);
  const speechCharacters = speech.reduce((n, r) => n + r.characters, 0);
  const speechErrors = speech.reduce((n, r) => n + r.errors, 0);
  const speechAudio = speech.reduce((n, r) => n + r.audio_seconds, 0);
  const speechElapsed = speech.reduce((n, r) => n + r.elapsed_seconds, 0);
  const silence = scored.filter(r => r.reference.trim() === '');
  return { normalization: 'NFKC + lower-case; remove Unicode punctuation and whitespace',
    rows: scored, characters, errors, cer: characters ? errors / characters : null,
    audio_seconds: audio, elapsed_seconds: elapsed, rtf: elapsed / audio,
    speech: { characters: speechCharacters, errors: speechErrors,
      cer: speechCharacters ? speechErrors / speechCharacters : null,
      audio_seconds: speechAudio, elapsed_seconds: speechElapsed,
      rtf: speechAudio ? speechElapsed / speechAudio : null },
    silence: { rows: silence.length, raw_nonempty_rows: silence.filter(r => r.text.trim() !== '').length,
      normalized_insertions: silence.reduce((n, r) => n + r.errors, 0) } };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const rows = readFileSync(process.argv[2], 'utf8').trim().split('\n').map(JSON.parse);
  console.log(JSON.stringify(score(rows), null, 2));
}
