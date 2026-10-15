import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';

const script = new URL('./summarize_robustness.mjs', import.meta.url);
function fixture(callback) {
  const dir = mkdtempSync(join(tmpdir(), 'robustness-report-'));
  const sources = [
    {id: 'speech', reference: 'いいえ', purpose: 'accuracy', condition: 'clean'},
    {id: 'noise', reference: '', purpose: 'silence', condition: 'negative'},
  ].map(r => ({...r, language: 'Japanese', audio_seconds: 1, sha256: r.id}));
  const rows = sources.map(r => ({...r, fixture_sha256: r.sha256, status: 'ok',
    text: r.id === 'speech' ? '' : 'はい。', worker: {status: r.id === 'speech' ? 'no_speech' : 'recognized', asr_calls: r.id === 'speech' ? 0 : 1}}));
  function execute(input) {
    writeFileSync(join(dir, 'manifest.json'), JSON.stringify({rows: sources}));
    writeFileSync(join(dir, 'results.jsonl'), input.map(r => JSON.stringify(r)).join('\n'));
    execFileSync(process.execPath, [script.pathname, dir], {stdio: 'pipe'});
    return JSON.parse(readFileSync(join(dir, 'report.json')));
  }
  try { callback(rows, execute); } finally { rmSync(dir, {recursive: true, force: true}); }
}

test('missed speech counts full deletion and non-speech insertions remain raw', () => {
  fixture((rows, execute) => {
    const report = execute(rows);
    assert.equal(report.groups[0].cer, 1);
    assert.deepEqual(report.groups[0].missed_speech, ['speech']);
    assert.equal(report.negative_gate_passes, 0);
    assert.equal(report.negative_failures[0].text, 'はい。');
    assert.equal(report.quality_accepted, false);
  });
});

test('incomplete and duplicated measurements reject', () => {
  fixture((rows, execute) => {
    assert.throws(() => execute(rows.slice(0, 1)));
    assert.throws(() => execute([...rows, rows[0]]));
  });
});

test('mismatched fixture hashes reject', () => {
  fixture((rows, execute) => {
    assert.throws(() => execute([{...rows[0], fixture_sha256: 'wrong'}, rows[1]]));
  });
});
