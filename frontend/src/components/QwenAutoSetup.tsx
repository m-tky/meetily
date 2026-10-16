import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Button } from './ui/button';
import { prepareAndSelectQwen, setupActive, type QwenSettings, type QwenSetupInfo, type QwenSetupStatus } from '@/lib/qwen-setup';

const gib = (bytes: number) => (bytes / 1024 ** 3).toFixed(1);
export function QwenAutoSetup({ disabled, onPrepared, onBusyChange }: { disabled: boolean; onPrepared: (settings: QwenSettings, shouldApply: () => boolean) => Promise<void>; onBusyChange: (busy: boolean) => void }) {
  const [info, setInfo] = useState<QwenSetupInfo | null>(null);
  const [status, setStatus] = useState<QwenSetupStatus | null>(null);
  const [consent, setConsent] = useState(false);
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState('');
  const [selected, setSelected] = useState(false);
  const active = useRef(false);
  const cancelRequested = useRef(false);
  useEffect(() => {
    active.current = true;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const value = await invoke<QwenSetupStatus | null>('qwen_setup_status');
        if (active.current) setStatus(value);
      } catch (e) { if (active.current) setError(String(e)); }
      if (active.current) timer = setTimeout(poll, 1000);
    };
    invoke<QwenSetupInfo>('qwen_setup_info').then(value => { if (active.current) setInfo(value); }).catch(e => { if (active.current) setError(String(e)); });
    void poll();
    return () => { active.current = false; clearTimeout(timer); };
  }, []);
  const running = setupActive(status);
  useEffect(() => { onBusyChange(starting || running); }, [starting, running, onBusyChange]);
  useEffect(() => () => onBusyChange(false), [onBusyChange]);
  if (!info?.available) return null;
  const locked = disabled || starting || running;
  const start = async () => {
    if (locked || !consent) return;
    cancelRequested.current = false;
    setStarting(true); setError(''); setSelected(false);
    try {
      await prepareAndSelectQwen({ consent, invoke, onStatus: s => { if (active.current) setStatus(s); },
        onPrepared: async settings => { await onPrepared(settings, () => active.current && !cancelRequested.current); if (active.current && !cancelRequested.current) setSelected(true); },
        isActive: () => active.current && !cancelRequested.current, wait: () => new Promise(resolve => setTimeout(resolve, 750)) });
    } catch (e) { if (active.current) setError(String(e)); }
    finally { if (active.current) { setStarting(false); setConsent(false); } }
  };
  const usePrepared = async () => {
    if (!status?.settings || locked) return;
    cancelRequested.current = false;
    setStarting(true); setError('');
    try { await onPrepared(status.settings, () => active.current && !cancelRequested.current); if (active.current) setSelected(true); }
    catch (e) { if (active.current) setError(String(e)); }
    finally { if (active.current) setStarting(false); }
  };
  return <section className="border rounded p-4 space-y-3" aria-label="Automatic Qwen setup">
    <h3 className="font-medium">Set up Qwen locally (Windows)</h3>
    <p className="text-sm">Download Qwen3-ASR 1.7B from Hugging Face (~{gib(info.download_bytes)} GiB), verify its checksums, and prepare its local INT8 cache. Allow {gib(info.minimum_free_bytes)} GiB free disk space, several GB of available RAM, and several minutes of CPU processing.</p>
    <p className="text-xs break-all">Install to: {info.model_dir}</p>
    <p className="text-sm">Verified files are reused. Different existing files are never overwritten. This selects Qwen after validation; no recording, cloud inference, Parakeet or summary-model download is started.</p>
    <label className="flex items-start gap-2 text-sm">
      <input type="checkbox" checked={consent} disabled={locked} onChange={e => setConsent(e.target.checked)} />
      I agree to download the public Apache-2.0 model, prepare it locally, and use Qwen for transcription.
    </label>
    <Button disabled={locked || !consent} onClick={start}>Agree and set up Qwen</Button>
    {(running || starting) && <div role="status" aria-live="polite" className="text-sm">
      {status?.phase || 'Starting'} {status?.file}
      {status && status.total > 0 && <progress className="block w-full" value={status.downloaded} max={status.total} aria-label="Model files verified or downloaded" />}
      <p>Initialization and worker validation continue after the file progress reaches 100%. You may leave this screen; return here to review setup. Closing the app stops setup.</p>
      {running && status && <Button variant="outline" onClick={async () => {
        cancelRequested.current = true;
        try { await invoke('qwen_setup_cancel', { id: status.id }); } catch (e) { setError(String(e)); }
      }}>Cancel setup</Button>}
    </div>}
    {status?.phase === 'ready' && !selected && <Button variant="outline" disabled={locked} onClick={usePrepared}>Use prepared Qwen</Button>}
    {selected && <p role="status" className="text-sm">Qwen is ready and selected.</p>}
    {(error || status?.error) && <p role="alert" className="text-sm text-red-600">{error || status?.error}</p>}
    {['failed', 'cancelled'].includes(status?.phase || '') && <p className="text-sm">Retry with consent above. Verified complete files are kept; an interrupted file is downloaded again.</p>}
  </section>;
}
