'use client';

import { useEffect, useRef, useState } from 'react';
import Link from 'next/link';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { LiveMinutesRunner, type MinutesSession, type MinutesStop } from '@/lib/live-minutes-runner';
import { useRecordingState } from '@/contexts/RecordingStateContext';
import { useMinutesSettings } from '@/contexts/MinutesSettingsContext';

import { MinutesDocument } from './MinutesDocument';
import { acceptMinutesSnapshot, type MinutesSnapshot as Snapshot } from '@/lib/minutes-document';

/** Experimental opt-in worker: sequential polling, never on the audio path. */
export function LiveMinutesPanel() {
  const { isRecording } = useRecordingState();
  const [enabled, setEnabled] = useState(false);
  const { settings, loaded, error: settingsError } = useMinutesSettings();
  const { provider } = settings;
  const [account, setAccount] = useState<string | null>(null);
  const selectedModel = provider === 'ollama' ? settings.ollamaModel : settings.chatgptModel;
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [message, setMessage] = useState('');
  const [listening, setListening] = useState(false);
  const [canFinalize, setCanFinalize] = useState(false);
  const [documentSession, setDocumentSession] = useState<string | null>(null);
  const viewEpoch = useRef(0);
  const runner = useRef<LiveMinutesRunner<Snapshot> | null>(null);
  if (!runner.current) runner.current = new LiveMinutesRunner<Snapshot>({
    session: async () => {
      const epoch = viewEpoch.current;
      const info = await invoke<MinutesSession | null>('api_get_live_minutes_session');
      if (epoch === viewEpoch.current) setDocumentSession(info?.id ?? null);
      return info;
    },
    update: args => invoke<Snapshot>('api_update_live_minutes', args),
    result: (value, final) => {
      setSnapshot(current => acceptMinutesSnapshot(current, value));
      setMessage(`${final ? 'Final draft' : 'Draft'} · revision ${value.revision}`);
    },
    message: setMessage, enabled: setEnabled,
  });

  useEffect(() => {
    let active = true;
    runner.current?.cancel(); setAccount(null);
    if (loaded && provider === 'chatgpt_plan') {
      void (async () => {
        const status = await invoke<{ active: string | null; accounts: { client_id: string; connected: boolean; plan_enabled: boolean }[] }>('chatgpt_status');
        const current = status.accounts.find(a => a.client_id === status.active && a.connected && a.plan_enabled);
        if (!current) throw new Error('Connect ChatGPT in Settings.');
        const models = await invoke<{ slug: string }[]>('chatgpt_list_models', { clientId: current.client_id });
        if (!models.some(m => m.slug === settings.chatgptModel)) throw new Error('Select an available model in Settings.');
        if (active) { setAccount(current.client_id); setMessage(''); }
      })().catch(e => { if (active) setMessage(String(e)); });
    }
    return () => { active = false; runner.current?.cancel(); };
  }, [loaded, provider, settings.chatgptModel, settings.ollamaModel]);

  useEffect(() => {
    // Reset consent only on a NEW recording, never on the Stop transition.
    if (isRecording) { viewEpoch.current++; runner.current?.cancel(); setSnapshot(null); setCanFinalize(false); setDocumentSession(null); }
  }, [isRecording]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<MinutesStop>('recording-stopped', event => {
      setCanFinalize(event.payload.transcription_drained === true && !!event.payload.minutes_session_id);
      setDocumentSession(event.payload.minutes_session_id ?? null);
      runner.current?.recordingStopped(event.payload);
    })
      .then(fn => { if (disposed) fn(); else { unlisten = fn; setListening(true); } })
      .catch(e => { if (!disposed) setMessage(`Could not subscribe to recording status: ${String(e)}`); });
    const epoch = viewEpoch.current;
    void invoke<MinutesSession | null>('api_get_live_minutes_session').then(info => {
      if (!disposed && epoch === viewEpoch.current) { setCanFinalize(!!info?.ended && info.transcription_drained); setDocumentSession(info?.id ?? null); }
    }).catch(() => {});
    return () => { disposed = true; unlisten?.(); runner.current?.cancel(); };
  }, []);

  return (
    <details className="border-b border-border px-5 py-3 shrink-0" open={enabled}>
      <summary className="cursor-pointer text-sm font-medium">Meeting minutes</summary>
      <div className="flex gap-3 items-center mt-3 text-sm">
        <span>{provider === 'chatgpt_plan' ? 'ChatGPT' : 'Ollama'}{selectedModel && ` · ${selectedModel}`}</span>
        <Link href="/settings?tab=summaryModels" className="underline">Settings</Link>
      </div>
      <div className="flex items-center gap-3 mt-3">
        <button className="border rounded px-3 py-1 text-sm" disabled={!enabled && (!loaded || !listening || !isRecording || !selectedModel.trim() || (provider === 'chatgpt_plan' && !account))}
          onClick={() => {
            if (enabled) { runner.current?.cancel(); setMessage('Updates stopped.'); }
            else void runner.current?.start({ model: selectedModel.trim(), provider, account });
          }}>{enabled ? 'Stop updates' : 'Start updates'}</button>
        {!isRecording && canFinalize && <button className="border rounded px-3 py-1 text-sm"
          disabled={!loaded || enabled || !selectedModel.trim() || (provider === 'chatgpt_plan' && !account)}
          onClick={() => { void runner.current?.start({ model: selectedModel.trim(), provider, account }, true); }}>
          Update final draft
        </button>}
      </div>
      {settingsError && <p role="alert" className="text-xs text-red-600">{settingsError}</p>}
      {message && <p role="status" className="text-xs mt-1">{message}</p>}
      {documentSession && <MinutesDocument key={documentSession} snapshot={snapshot}
        target={{ kind: 'session', id: documentSession }}
        onSnapshot={value => setSnapshot(current => acceptMinutesSnapshot(current, value))} />}
    </details>
  );
}
