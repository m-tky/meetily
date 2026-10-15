'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import { RecordingCandidate, RecordingRecoveryScan, restoreRecording, scanRecordingRecovery } from '@/lib/recording-recovery';

export function RecordingRecoveryPanel({ busy, onRecovered }: {
  busy: boolean;
  onRecovered: (meetingId: string) => Promise<void>;
}) {
  const [scan, setScan] = useState<RecordingRecoveryScan | null>(null);
  const [loading, setLoading] = useState(false);
  const [restoring, setRestoring] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const generation = useRef(0);
  const mounted = useRef(true);
  const pendingRestore = useRef(false);
  const refresh = useCallback(async (cursor: string | null = null) => {
    const current = ++generation.current;
    setLoading(true); setError(null);
    try {
      const next = await scanRecordingRecovery(cursor);
      if (!mounted.current || current !== generation.current) return;
      setScan(old => cursor && old ? {
        candidates: [...old.candidates, ...next.candidates].filter((item, n, items) => items.findIndex(other => other.id === item.id) === n),
        next_cursor: next.next_cursor,
        warnings: [...new Set([...old.warnings, ...next.warnings])],
      } : next);
    } catch (e) {
      if (mounted.current && current === generation.current) setError(String(e));
    } finally {
      if (mounted.current && current === generation.current) setLoading(false);
    }
  }, []);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; generation.current++; };
  }, []);
  useEffect(() => {
    if (!busy) void refresh();
    else { generation.current++; setLoading(false); }
  }, [busy, refresh]);
  const restore = async (candidate: RecordingCandidate) => {
    if (busy || pendingRestore.current || !candidate.can_restore) return;
    pendingRestore.current = true;
    setRestoring(candidate.id); setError(null); setNotice(null);
    let registered = false;
    try {
      const result = await restoreRecording(candidate);
      registered = true;
      if (!mounted.current) return;
      setScan(old => old && { ...old, candidates: old.candidates.filter(item => item.id !== candidate.id) });
      setNotice('Saved data was added to the meeting list. Original files and checkpoints were not changed.');
      await onRecovered(result.meeting_id);
    } catch (e) {
      if (mounted.current) setError(`${registered ? 'Meeting is registered, but opening it failed: ' : ''}${String(e)}`);
    } finally {
      pendingRestore.current = false;
      if (mounted.current) setRestoring(null);
    }
  };
  return <section className="border-b bg-amber-50 px-4 py-2 text-sm" aria-label="Saved recording recovery">
    <div className="flex items-center gap-3">
      <strong>Saved recording recovery</strong>
      <button className="underline disabled:opacity-50" disabled={busy || loading || !!restoring} onClick={() => void refresh()}>
        {loading ? 'Checking…' : 'Check saved recordings'}
      </button>
      {busy && <span>Available after recording and saving finish.</span>}
    </div>
    {error && <p role="alert" className="text-red-700 whitespace-pre-wrap">{error}</p>}
    {notice && <p role="status">{notice}</p>}
    {!!scan?.candidates.length && <>
      <p>These folders are not registered in the meeting list. Add only the data already saved locally; missing speech is not reconstructed. No AI or cloud processing occurs.</p>
      <p>Original audio, transcripts, canonical minutes and editing history are kept. Audio checkpoints are not merged by this action.</p>
      <p className="text-amber-800">Browser-cache recovery is a separate source and may contain additional speech. Choose a source after checking its contents; restoring both separately can create duplicate meetings. No source merging happens here.</p>
      <ul className="max-h-64 overflow-auto">
        {scan.candidates.map(candidate => <li key={candidate.id} className="border-t py-2">
          <strong>{candidate.title}</strong> <span>{candidate.created_at ?? 'Unknown date'} · original status: {candidate.original_status ?? 'unreadable'}</span>
          <p>{candidate.transcript_count} saved segments · audio file: {candidate.audio_present ? 'present (not validated)' : 'missing'} · checkpoints: {candidate.checkpoint_count} · canonical minutes file: {candidate.minutes_present ? 'present' : 'unavailable'}</p>
          <details><summary>Folder and recovery warnings</summary><p>{candidate.folder_name}</p>
            <ul>{candidate.warnings.map((warning, n) => <li key={n}>{warning}</li>)}</ul>
          </details>
          <button className="underline disabled:opacity-50" disabled={busy || loading || !!restoring || !candidate.can_restore} onClick={() => void restore(candidate)}>
            {restoring === candidate.id ? 'Adding…' : 'Add saved data to meeting list'}
          </button>
        </li>)}
      </ul>
    </>}
    {scan?.warnings.map((warning, n) => <p role="status" key={n}>{warning}</p>)}
    {scan?.next_cursor && <button className="underline disabled:opacity-50" disabled={busy || loading || !!restoring} onClick={() => void refresh(scan.next_cursor)}>Check more folders</button>}
  </section>;
}
