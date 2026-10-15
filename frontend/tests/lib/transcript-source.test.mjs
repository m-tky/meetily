import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const ts = require('typescript');
const react = require('react');
const { renderToStaticMarkup } = require('react-dom/server');
function load(file, dependencies = {}) {
  const result = ts.transpileModule(readFileSync(new URL(file, import.meta.url), 'utf8'), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX, target: ts.ScriptTarget.ES2020 },
  });
  const module = { exports: {} };
  new Function('require', 'module', 'exports', result.outputText)(
    (name) => dependencies[name] ?? require(name), module, module.exports);
  return module.exports;
}
const source = load('../../src/lib/transcript-source.ts');
const { TranscriptInputBadge } = load('../../src/components/TranscriptInputBadge.tsx', {
  '@/lib/transcript-source': source,
});
const event = { text: '同じ発言', timestamp: '00:01', sequence_id: 0, speaker: 'mic' };

test('only capture tags have labels, not guessed people or legacy mixed audio', () => {
  assert.equal(source.transcriptSourceLabel('mic'), 'Mic');
  assert.equal(source.transcriptSourceLabel('system'), 'PC');
  for (const v of [undefined, null, '', 'Audio', 'speaker_1', 'Alice']) {
    assert.equal(source.transcriptSourceLabel(v), null);
  }
});
test('source results render by audio start even when a long lane completes later', () => {
  const early = { audio_start_time: 0, chunk_start_time: 99, sequence_id: 2 };
  const late = { audio_start_time: 4, sequence_id: 1 };
  assert.deepEqual([late, early].sort(source.compareTranscriptTimes), [early, late]);
  assert.ok(source.compareTranscriptTimes({ audio_start_time: 1, sequence_id: 0 }, { audio_start_time: 1, sequence_id: 1 }) < 0);
});
test('sequence zero is a valid event ID and duplicate delivery is idempotent', () => {
  assert.equal(source.sameTranscriptEvent(event, { ...event }), true);
});
test('identical simultaneous words from two inputs are both retained', () => {
  assert.equal(source.sameTranscriptEvent(event, { ...event, sequence_id: 1, speaker: 'system' }), false);
});
test('legacy fallback also distinguishes source without inventing a source', () => {
  const a = { ...event, sequence_id: undefined };
  assert.equal(source.sameTranscriptEvent(a, { ...a, speaker: 'system' }), false);
  assert.equal(source.sameTranscriptEvent({ ...a, speaker: undefined }, { ...a, speaker: null }), true);
});
test('microphone badge renders capture origin not You/person identification', () => {
  const html = renderToStaticMarkup(react.createElement(TranscriptInputBadge, { source: 'mic' }));
  assert.match(html, />Mic</); assert.doesNotMatch(html, />You</);
  assert.match(html, /not speaker identification/);
});
test('computer badge renders PC without remote participant numbering', () => {
  const html = renderToStaticMarkup(react.createElement(TranscriptInputBadge, { source: 'system' }));
  assert.match(html, />PC</); assert.doesNotMatch(html, /Speaker [0-9]/);
});
test('legacy mixed audio and unrecognized speaker strings render no invented badge', () => {
  for (const source of [undefined, null, 'speaker_1']) {
    assert.equal(renderToStaticMarkup(react.createElement(TranscriptInputBadge, { source })), '');
  }
});
