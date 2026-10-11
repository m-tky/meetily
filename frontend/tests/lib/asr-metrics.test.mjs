import test from 'node:test';
import assert from 'node:assert/strict';
import { normalize, distance, score } from '../../scripts/asr-metrics.mjs';

test('normalization policy is explicit and keeps numeral/reading differences', () => {
  assert.equal(normalize('ＡＩ、 会議。\n１２３'), 'ai会議123');
  assert.notEqual(normalize('2人'), normalize('二人'));
  assert.equal(normalize('¥100'), '¥100');
});
test('Unicode character edit distance handles insertion/deletion/substitution', () => {
  assert.equal(distance('会議', '会期'), 1);
  assert.equal(distance('', '会議'), 2);
  assert.equal(distance('会議', ''), 2);
  assert.equal(distance('𠮷田', '田'), 1);
});
test('corpus CER and RTF are weighted, not average of per-row ratios', () => {
  const report = score([
    { reference: 'あ', text: 'い', audio_seconds: 1, elapsed_seconds: 2 },
    { reference: 'あいう', text: 'あいう', audio_seconds: 3, elapsed_seconds: 1 },
  ]);
  assert.equal(report.cer, 0.25);
  assert.equal(report.rtf, 0.75);
});
test('silence raw output remains visible even when punctuation normalization erases it', () => {
  const report = score([{ reference: '', text: '。', audio_seconds: 5, elapsed_seconds: 1 }]);
  assert.equal(report.silence.raw_nonempty_rows, 1);
  assert.equal(report.silence.normalized_insertions, 0);
  assert.equal(report.speech.cer, null);
  assert.equal(report.speech.rtf, null);
});
test('empty reference reports insertions without fake CER; bad durations reject', () => {
  assert.equal(score([{ reference: '', text: '幻覚', audio_seconds: 5, elapsed_seconds: 1 }]).cer, null);
  assert.throws(() => score([]));
  assert.throws(() => score([{ reference: 'あ', text: '', audio_seconds: 0, elapsed_seconds: 1 }]));
});
