import React from 'react';
import { expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { MinutesDocument } from '../../src/components/MinutesDocument';
import { acceptMinutesSnapshot, beginMinutesEdit, editArguments, downloadMinutes, type MinutesSnapshot } from '../../src/lib/minutes-document';
const snapshot: MinutesSnapshot = { revision: 4, updated_at: 'synthetic', items: [
  { id: 'item-1', kind: 'decision', text: 'User <script>text</script>', status: 'active', sources: ['source-1'], owner: null, due: null },
], user_edits: [] };

test('unsaved edit keeps its base revision and does not mutate incoming AI data', () => {
  const draft = beginMinutesEdit(snapshot, 'item-1'); draft.text = 'local unsaved edit';
  const newer = { ...snapshot, revision: 5, items: [{ ...snapshot.items[0], text: 'new AI result' }] };
  expect(acceptMinutesSnapshot(newer, snapshot)).toBe(newer);
  expect(acceptMinutesSnapshot(snapshot, newer)).toBe(newer);
  expect(draft.baseRevision).toBe(4); expect(snapshot.items[0].text).not.toBe(draft.text);
  expect(newer.items[0].text).toBe('new AI result');
  const args = editArguments({ kind: 'meeting', id: 'm1' }, draft);
  expect(args.expectedRevision).toBe(4); expect(args.itemId).toBe('item-1');
  expect(args.patch.owner).toBeNull(); expect('sources' in args.patch).toBe(false); expect('id' in args.patch).toBe(false);
});
test('read/edit/export controls and protected user provenance render as escaped text', () => {
  const edited = { ...snapshot, user_edits: [{ base_revision: 3, edited_at: 'synthetic', before: { ...snapshot.items[0], text: 'original' }, after: snapshot.items[0] }] };
  const html = renderToStaticMarkup(<MinutesDocument snapshot={edited} target={{ kind: 'meeting', id: 'm1' }} onSnapshot={() => {}} />);
  expect(html).toContain('Export JSON'); expect(html).toContain('Export Markdown'); expect(html).toContain('protected');
  expect(html).toContain('Edit history'); expect(html).toContain('&lt;script&gt;'); expect(html).not.toContain('<script>');
});
test('local export downloads the requested canonical format/revision, no arbitrary file write', async () => {
  const originalDocument = (globalThis as any).document;
  const originalCreate = URL.createObjectURL; const originalRevoke = URL.revokeObjectURL;
  let captured!: Blob; let clicked = 0; let removed = 0; let revoked = 0;
  const link = { href: '', download: '', click: () => { clicked++; }, remove: () => { removed++; } };
  (globalThis as any).document = { createElement: () => link, body: { appendChild: () => {} } };
  URL.createObjectURL = blob => { captured = blob as Blob; return 'blob:synthetic'; };
  URL.revokeObjectURL = () => { revoked++; };
  try {
    downloadMinutes({ revision: 4, json: '{"canonical":true}', markdown: '# user edit' }, 'json');
    expect(await captured.text()).toBe('{"canonical":true}'); expect(link.download).toBe('minutes-revision-4.json');
    downloadMinutes({ revision: 4, json: '{}', markdown: '# user edit' }, 'markdown');
    expect(await captured.text()).toBe('# user edit'); expect(link.download).toBe('minutes-revision-4.md');
    expect(clicked).toBe(2); expect(removed).toBe(2); expect(revoked).toBe(2);
  } finally {
    (globalThis as any).document = originalDocument; URL.createObjectURL = originalCreate; URL.revokeObjectURL = originalRevoke;
  }
});
