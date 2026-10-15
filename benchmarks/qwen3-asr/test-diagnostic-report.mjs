import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';

const script = new URL('./summarize_diagnostics.mjs', import.meta.url);
function fixture(callback) {
  const root = mkdtempSync(join(tmpdir(), 'asr-diagnostics-'));
  const base = join(root, 'diagnostics');
  for (const tag of ['gate', 'context', 'ungated']) mkdirSync(join(base, tag), {recursive: true});
  const groups = [];
  for (const suite of ['a', 'b', 'c']) for (const policy of ['none', 'peak20', 'peak100']) {
    for (const threshold of [.3, .5, .7, .9]) for (const minimum_ms of [90, 200]) {
      groups.push({suite, policy, threshold, minimum_ms, missed_speech: ['speech'], false_positive: ['noise']});
    }
  }
  const gate = {groups, details: [], input_windows: 2, unique_wavs: 2};
  const contexts = [false, true].map(crop => ({id: crop ? 'crop' : 'full', reference: 'いいえ', sha256: 'word', crop_context: crop}));
  const rows = contexts.map(r => ({id: r.id, fixture_sha256: 'word', status: 'ok', text: r.id === 'full' ? '家。' : 'いいえ。',
    worker: {confirmed: false, asr_calls: 1, status: 'recognized', recognition_context: r.crop_context ? 'vad_envelope' : 'full_input'}}));
  const ungated = [0, 1].map(repeat => ({id: 'en', repeat, fixture_sha256: 'en', text: 'yes', confirmed: false, gate_bypassed_for_diagnosis: true}));
  let index = 0;
  function execute(contextRows = rows, ungatedRows = ungated) {
    const json = (tag, file, value) => writeFileSync(join(base, tag, file), JSON.stringify(value));
    const lines = (tag, file, value) => writeFileSync(join(base, tag, file), value.map(r => JSON.stringify(r)).join('\n'));
    json('gate', 'report.json', gate);
    json('context', 'manifest.json', {rows: contexts}); lines('context', 'results.jsonl', contextRows);
    json('ungated', 'manifest.json', {rows: [{id: 'en', sha256: 'en', reference: 'yes', language: 'English'}]});
    lines('ungated', 'results.jsonl', ungatedRows);
    const tag = `summary-${index++}`;
    execFileSync(process.execPath, [script.pathname, root, 'gate', 'context', 'ungated', tag], {stdio: 'pipe'});
    return file => JSON.parse(readFileSync(join(base, tag, file)));
  }
  try { callback(rows, ungated, execute); } finally { rmSync(root, {recursive: true, force: true}); }
}

test('modes remain separate and wrong unflagged text is retained', () => {
  fixture((rows, ungated, execute) => {
    const read = execute();
    const c = read('context-summary.json');
    assert.equal(c.modes[0].normalized_exact_matches, 0);
    assert.equal(c.modes[1].normalized_exact_matches, 1);
    assert.deepEqual(c.modes[0].wrong_unflagged, ['full']);
    assert.equal(c.rows[0].text, '家。');
    assert.equal(read('gate-summary.json').any_zero_miss_zero_false_positive_grid_setting, false);
  });
});

test('incorrect crop mode and missing context result reject', () => {
  fixture((rows, ungated, execute) => {
    assert.throws(() => execute([{...rows[0], worker: {...rows[0].worker, recognition_context: 'vad_envelope'}}, rows[1]]));
    assert.throws(() => execute(rows.slice(0, 1)));
  });
});

test('incomplete/duplicate ungated repeats reject', () => {
  fixture((rows, ungated, execute) => {
    assert.throws(() => execute(rows, ungated.slice(0, 1)));
    assert.throws(() => execute(rows, [ungated[0], ungated[0]]));
  });
});
