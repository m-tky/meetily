// Compact gate/context/ASR isolation evidence; never an acceptance selector.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { resolve, join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { normalize, distance } from '../../frontend/scripts/asr-metrics.mjs';
import { words } from './metrics.mjs';

const [rootArg, gateTag, contextTag, ungatedTag, outputTag] = process.argv.slice(2);
if (!outputTag || ![gateTag, contextTag, ungatedTag, outputTag].every(t => /^[a-z0-9_-]+$/.test(t))) {
  throw new Error('Usage: summarize_diagnostics.mjs RUNTIME_ROOT GATE_TAG CONTEXT_TAG UNGATED_TAG OUTPUT_TAG');
}
const root = resolve(rootArg);
const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
if (root === repo || root.startsWith(repo + '/')) throw new Error('Artifacts must stay outside Git');
const base = join(root, 'diagnostics');
const json = (tag, name) => JSON.parse(readFileSync(join(base, tag, name), 'utf8'));
const lines = (tag, name) => readFileSync(join(base, tag, name), 'utf8').trim().split('\n').map(JSON.parse);
const out = join(base, outputTag);
mkdirSync(out); // Refuse to overwrite a prior diagnostic summary.

const gate = json(gateTag, 'report.json');
const grid = [];
for (const policy of ['none', 'peak20', 'peak100']) {
  for (const threshold of [.3, .5, .7, .9]) {
    for (const minimum_ms of [90, 200]) {
      const groups = gate.groups.filter(g => g.policy === policy && g.threshold === threshold && g.minimum_ms === minimum_ms);
      if (groups.length !== 3) throw new Error('Incomplete gate grid');
      grid.push({policy, threshold, minimum_ms,
        missed_speech_windows: groups.reduce((n, g) => n + g.missed_speech.length, 0),
        false_positive_windows: groups.reduce((n, g) => n + g.false_positive.length, 0), groups});
    }
  }
}
const gateReport = {binary_sha256: gate.binary_sha256, frame_samples: gate.frame_samples,
  input_windows: gate.input_windows, unique_wavs: gate.unique_wavs,
  probabilities_sha256: gate.probabilities_sha256,
  default_endpoint_parity: 'Every neural probability sequence matched Rust default endpoints; recorded detection reproduced',
  annotation_scope: 'window presence only, not word boundaries or actual speech duration',
  any_zero_miss_zero_false_positive_grid_setting: grid.some(g => !g.missed_speech_windows && !g.false_positive_windows),
  adopted_policy: null, quality_accepted: false, grid,
  failed_fixture_maxima: gate.details.filter(d => d.threshold === .5 && d.minimum_ms === 90 && [
    'negative-bursts-0.002', 'negative-bursts-0.01',
    'fleurs-en_us-validation-0-1548-gain-40dB', 'fleurs-en_us-validation-0-1548-white-snr0'].includes(d.id))};
writeFileSync(join(out, 'gate-summary.json'), JSON.stringify(gateReport, null, 2) + '\n');

function score(reference, text) {
  const chars = Array.from(normalize(reference)).length;
  const errors = distance(normalize(reference), normalize(text));
  const refWords = words(reference);
  return {character_errors: errors, reference_characters: chars, cer: chars ? errors / chars : null,
    word_errors: distance(refWords, words(text)), reference_words: refWords.length,
    normalized_exact_match: normalize(reference) === normalize(text)};
}
const contextManifest = json(contextTag, 'manifest.json');
const contextMap = new Map(contextManifest.rows.map(r => [r.id, r]));
const contextSeen = new Set();
const contextRows = lines(contextTag, 'results.jsonl').map(r => {
  const source = contextMap.get(r.id);
  if (!source || contextSeen.has(r.id) || r.status !== 'ok' || r.fixture_sha256 !== source.sha256) {
    throw new Error('Invalid context result');
  }
  contextSeen.add(r.id);
  const mode = source.crop_context ? 'vad_envelope' : 'full_input';
  if ((r.worker.asr_calls > 0 && r.worker.recognition_context !== mode) || r.worker.confirmed !== false) {
    throw new Error('Context mode mismatch');
  }
  return {...r, recognition_context: mode, ...score(source.reference, r.text)};
});
if (contextSeen.size !== contextMap.size) throw new Error('Incomplete context results');
const modes = ['full_input', 'vad_envelope'].map(mode => {
  const rr = contextRows.filter(r => r.recognition_context === mode);
  return {recognition_context: mode, windows: rr.length, detected: rr.filter(r => r.worker.asr_calls > 0).length,
    normalized_exact_matches: rr.filter(r => r.normalized_exact_match).length,
    wrong_unflagged: rr.filter(r => !r.normalized_exact_match && r.worker.status === 'recognized').map(r => r.id)};
});
writeFileSync(join(out, 'context-summary.json'), JSON.stringify({quality_accepted: false,
  single_speaker_isolated_word_only: true, modes, rows: contextRows}, null, 2) + '\n');

const ungatedManifest = json(ungatedTag, 'manifest.json');
const ungatedMap = new Map(ungatedManifest.rows.map(r => [r.id, r]));
const seen = new Set();
const ungatedRows = lines(ungatedTag, 'results.jsonl').map(r => {
  const source = ungatedMap.get(r.id), key = `${r.id}:${r.repeat}`;
  if (!source || seen.has(key) || ![0, 1].includes(r.repeat) || r.fixture_sha256 !== source.sha256 ||
      !r.gate_bypassed_for_diagnosis || r.confirmed !== false) throw new Error('Invalid ungated result');
  seen.add(key);
  const s = score(source.reference, r.text);
  return {...r, ...s, wer: source.language === 'English' && s.reference_words ? s.word_errors / s.reference_words : null};
});
if (seen.size !== ungatedMap.size * 2) throw new Error('Incomplete ungated responses');
writeFileSync(join(out, 'ungated-summary.json'), JSON.stringify({quality_accepted: false,
  diagnostic_only_gate_bypass: true, worker_fallback_adopted: false, rows: ungatedRows}, null, 2) + '\n');
console.log(JSON.stringify({output: out, gate_settings: grid.length,
  zero_error_gate_setting: gateReport.any_zero_miss_zero_false_positive_grid_setting, context: modes,
  ungated_requests: ungatedRows.length}, null, 2));
