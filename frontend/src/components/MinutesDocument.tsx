'use client';
import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { acceptMinutesSnapshot, beginMinutesEdit, editArguments, downloadMinutes, type ItemDraft, type MinutesExport, type MinutesSnapshot, type MinutesTarget } from '@/lib/minutes-document';
const headings = { point: 'Key points', decision: 'Decisions', todo: 'ToDo', question: 'Open questions' };

/** Local file operations only. A draft is NOT reset when AI props/revision change. */
export function MinutesDocument({ snapshot, target, onSnapshot }: {
  snapshot: MinutesSnapshot | null; target: MinutesTarget; onSnapshot: (value: MinutesSnapshot) => void;
}) {
  const [draft, setDraft] = useState<ItemDraft | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const mounted = useRef(true);
  const onSnapshotRef = useRef(onSnapshot); onSnapshotRef.current = onSnapshot;
  const snapshotRef = useRef(snapshot); snapshotRef.current = snapshot;
  const accept = (value: MinutesSnapshot) => {
    if (mounted.current && (!snapshotRef.current || value.revision >= snapshotRef.current.revision)) onSnapshotRef.current(value);
  };
  const reload = async () => {
    setBusy(true);
    try {
      const value = await invoke<MinutesSnapshot | null>('api_read_live_minutes', { target });
      if (!mounted.current) return;
      if (value) { accept(value); setMessage(`Loaded revision ${value.revision}.`); }
      else setMessage('No saved minutes.');
    } catch (e) { if (mounted.current) setMessage(String(e)); }
    finally { if (mounted.current) setBusy(false); }
  };
  useEffect(() => {
    mounted.current = true; void reload();
    return () => { mounted.current = false; };
    // Parent keys this component by target identity; no request may cross targets.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(() => {
    if (!draft) return;
    const warn = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = ''; };
    window.addEventListener('beforeunload', warn);
    return () => window.removeEventListener('beforeunload', warn);
  }, [draft]);
  const save = async () => {
    if (!draft) return;
    setBusy(true);
    try {
      const value = await invoke<MinutesSnapshot>('api_edit_live_minutes', editArguments(target, draft));
      if (mounted.current) { accept(value); setDraft(null); setMessage(`Saved revision ${value.revision}.`); }
    } catch (e) { if (mounted.current) setMessage(String(e)); }
    finally { if (mounted.current) setBusy(false); }
  };
  const output = async (format: 'json' | 'markdown') => {
    setBusy(true);
    try {
      const value = await invoke<MinutesExport>('api_export_live_minutes', { target });
      if (mounted.current) { downloadMinutes(value, format); setMessage(`Exported ${format} · revision ${value.revision}.`); }
    } catch (e) { if (mounted.current) setMessage(String(e)); }
    finally { if (mounted.current) setBusy(false); }
  };
  return <div className="space-y-3 py-2">

    <div className="flex gap-2 flex-wrap text-xs">
      <button className="border rounded px-2 py-1" disabled={busy} onClick={() => { void reload(); }}>Reload</button>
      <button className="border rounded px-2 py-1" disabled={busy || !snapshot} onClick={() => { void output('json'); }}>Export JSON</button>
      <button className="border rounded px-2 py-1" disabled={busy || !snapshot} onClick={() => { void output('markdown'); }}>Export Markdown</button>
      {snapshot && <span>revision {snapshot.revision} / {snapshot.updated_at}</span>}
    </div>
    <p role="status" className="text-xs break-words">{message}</p>
    {draft && <fieldset className="border rounded p-2 space-y-2" disabled={busy}>
      <legend className="text-sm">Edit {draft.itemId} · revision {draft.baseRevision}</legend>
      <label className="block text-xs">Text<textarea className="block border rounded w-full p-2" aria-label="Minutes item text" value={draft.text} onChange={e => setDraft({ ...draft, text: e.target.value })} /></label>
      <label className="block text-xs">Status<select className="ml-2 border" value={draft.status} onChange={e => setDraft({ ...draft, status: e.target.value as ItemDraft['status'] })}>
        <option value="active">Active</option><option value="superseded">Superseded</option><option value="withdrawn">Withdrawn</option>
      </select></label>
      <label className="block text-xs">Owner<input className="block border w-full p-1" value={draft.owner} onChange={e => setDraft({ ...draft, owner: e.target.value })} /></label>
      <label className="block text-xs">Due<input className="block border w-full p-1" value={draft.due} onChange={e => setDraft({ ...draft, due: e.target.value })} /></label>

      {snapshot && snapshot.revision !== draft.baseRevision && <p role="alert" className="text-xs text-amber-800">Revision changed. Review before reapplying.
        <button className="border rounded ml-2 px-2" onClick={() => setDraft({ ...draft, baseRevision: snapshot.revision })}>Reapply to latest revision</button>
      </p>}
      <button className="border rounded px-2 py-1 text-xs mr-2" onClick={() => { void save(); }}>Save</button>
      <button className="border rounded px-2 py-1 text-xs" onClick={() => setDraft(null)}>Cancel</button>
    </fieldset>}
    {snapshot && <div className="max-h-72 overflow-y-auto space-y-3">
      {(Object.keys(headings) as (keyof typeof headings)[]).map(kind => <section key={kind}>
        <h3 className="text-sm font-medium">{headings[kind]}</h3>
        <ul className="list-disc pl-5 text-sm">{snapshot.items.filter(i => i.kind === kind && i.status === 'active').map(i => <li key={i.id}>
          <span className="whitespace-pre-wrap">{i.text}</span>{i.owner && ` / Owner: ${i.owner}`}{i.due && ` / Due: ${i.due}`}
          <span className="block text-xs">Sources: {i.sources.join(', ')} {(snapshot.user_edits ?? []).some(e => e.after.id === i.id) && '[Edited · protected]'}</span>
          <button disabled={busy || !!draft} className="border rounded px-2 text-xs" onClick={() => setDraft(beginMinutesEdit(snapshot, i.id))}>Edit</button>
        </li>)}</ul>
      </section>)}
      <section><h3 className="text-sm font-medium">Change history</h3>
        {snapshot.items.filter(i => i.status !== 'active').map(i => <div className="text-xs" key={i.id}>[{i.status}] {i.text} / Sources: {i.sources.join(', ')}
          <button disabled={busy || !!draft} className="border rounded px-2 ml-2" onClick={() => setDraft(beginMinutesEdit(snapshot, i.id))}>Edit</button>
        </div>)}
      </section>
      <details><summary className="text-sm">Edit history ({snapshot.user_edits?.length ?? 0})</summary>
        {(snapshot.user_edits ?? []).map((e, n) => <div key={n} className="text-xs py-1">
          {e.after.id} / {e.edited_at} / base revision {e.base_revision}<br />
          Before [{e.before.status}]: {e.before.text}<br />After[{e.after.status}]: {e.after.text}
        </div>)}
      </details>
    </div>}
  </div>;
}

export function SavedMinutesPanel({ meetingId }: { meetingId: string }) {
  const [snapshot, setSnapshot] = useState<MinutesSnapshot | null>(null);
  return <details className="border-b px-4 py-2 shrink-0">
    <summary className="text-sm cursor-pointer">Saved minutes</summary>
    <MinutesDocument key={meetingId} snapshot={snapshot} target={{ kind: 'meeting', id: meetingId }} onSnapshot={value => setSnapshot(current => acceptMinutesSnapshot(current, value))} />
  </details>;
}
