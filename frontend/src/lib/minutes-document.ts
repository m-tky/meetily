export type MinutesItem = {
  id: string; kind: 'point' | 'decision' | 'todo' | 'question';
  text: string; status: 'active' | 'superseded' | 'withdrawn';
  sources: string[]; owner: string | null; due: string | null;
};
export type MinutesSnapshot = { revision: number; updated_at: string; items: MinutesItem[];
  user_edits?: { base_revision: number; edited_at: string; before: MinutesItem; after: MinutesItem }[] };
export type MinutesTarget = { kind: 'session' | 'meeting'; id: string };
export type MinutesExport = { revision: number; json: string; markdown: string };
export type ItemDraft = { itemId: string; baseRevision: number; text: string; status: MinutesItem['status']; owner: string; due: string };
export function acceptMinutesSnapshot(current: MinutesSnapshot | null, incoming: MinutesSnapshot): MinutesSnapshot {
  return current && current.revision > incoming.revision ? current : incoming;
}
export function beginMinutesEdit(snapshot: MinutesSnapshot, id: string): ItemDraft {
  const item = snapshot.items.find(i => i.id === id);
  if (!item) throw new Error('Minutes item not found');
  return { itemId: id, baseRevision: snapshot.revision, text: item.text, status: item.status, owner: item.owner ?? '', due: item.due ?? '' };
}
export function editArguments(target: MinutesTarget, draft: ItemDraft) {
  return { target, expectedRevision: draft.baseRevision, itemId: draft.itemId,
    patch: { text: draft.text, status: draft.status, owner: draft.owner || null, due: draft.due || null } };
}
export function downloadMinutes(output: MinutesExport, format: 'json' | 'markdown') {
  const blob = new Blob([output[format]], { type: format === 'json' ? 'application/json;charset=utf-8' : 'text/markdown;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url; link.download = `minutes-revision-${output.revision}.${format === 'json' ? 'json' : 'md'}`;
  document.body.appendChild(link);
  try { link.click(); } finally { link.remove(); URL.revokeObjectURL(url); }
}
