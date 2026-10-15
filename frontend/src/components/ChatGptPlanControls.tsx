'use client';
import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

type Account = { client_id: string; label: string; connected: boolean; plan_enabled: boolean };
type AuthStatus = { active: string | null; accounts: Account[] };
type Model = { slug: string; display_name: string };
type Fixture = { elapsed_ms: number; passed: boolean };

/** Only account metadata crosses IPC. Credentials remain in the native store. */
export function ChatGptPlanControls({ initialModel = 'gpt-6-luna', onSelect }: {
  initialModel?: string; onSelect?: (account: string | null, model: string) => void;
}) {
  const [status, setStatus] = useState<AuthStatus>({ active: null, accounts: [] });
  const [catalog, setCatalog] = useState<{ account: string; models: Model[] } | null>(null);
  const [model, setModel] = useState(initialModel);
  const [catalogEpoch, setCatalogEpoch] = useState(0);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const active = status.accounts.find(a => a.client_id === status.active);
  const ready = !busy && !!active?.connected && !!active.plan_enabled
    && catalog?.account === active.client_id && catalog.models.some(m => m.slug === model);
  async function refresh() {
    setStatus(await invoke<AuthStatus>('chatgpt_status'));
    setCatalogEpoch(value => value + 1);
  }
  useEffect(() => { void refresh().catch(e => setMessage(String(e))); }, []);
  useEffect(() => {
    let cancelled = false;
    setCatalog(null);
    if (active?.connected && active.plan_enabled) {
      void invoke<Model[]>('chatgpt_list_models', { clientId: active.client_id }).then(models => {
        if (!cancelled) {
          setCatalog({ account: active.client_id, models });
          setMessage(models.some(m => m.slug === model) ? '' : 'Selected model unavailable.');
        }
      }).catch(e => { if (!cancelled) setMessage(String(e)); });
    }
    return () => { cancelled = true; };
    // Model selection does not refetch the catalog.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active?.client_id, active?.connected, active?.plan_enabled, catalogEpoch]);
  useEffect(() => { if (ready) onSelect?.(status.active, model); }, [ready, status.active, model, onSelect]);
  async function signIn(clientId: string | null, consent = false) {
    setBusy(true); setMessage('Complete sign-in in your browser.');
    try { setStatus(await invoke<AuthStatus>('chatgpt_sign_in', { clientId, consent })); setMessage(''); }
    catch (e) { setMessage(String(e)); await refresh().catch(() => {}); }
    finally { setBusy(false); }
  }
  async function selectAccount(clientId: string) {
    setBusy(true); setMessage('');
    try { setStatus(await invoke<AuthStatus>('chatgpt_select_account', { clientId })); }
    catch (e) { setMessage(String(e)); }
    finally { setBusy(false); }
  }
  async function signOut() {
    setBusy(true);
    try { setMessage(await invoke<string>('chatgpt_sign_out')); await refresh(); }
    catch (e) { setMessage(String(e)); }
    finally { setBusy(false); }
  }
  async function testFixture() {
    if (!ready || !status.active) return;
    setBusy(true); setMessage('Testing…');
    try {
      const result = await invoke<Fixture>('api_test_live_minutes_fixture', { account: status.active, model });
      setMessage(`${result.passed ? 'Test passed' : 'Test failed'} · ${(result.elapsed_ms / 1000).toFixed(2)}s`);
    } catch (e) { setMessage(String(e)); }
    finally { setBusy(false); }
  }
  return <section className="space-y-3">
    <p className="text-xs text-gray-500">ChatGPT plan · Text sent to OpenAI</p>
    {status.accounts.length > 0 && <select aria-label="ChatGPT account" className="border rounded px-2 py-1 max-w-full text-sm"
      value={status.active ?? ''} disabled={busy} onChange={e => void selectAccount(e.target.value)}>
      <option value="" disabled>Select account</option>
      {status.accounts.map(a => <option key={a.client_id} value={a.client_id}>{a.label}{!a.connected ? ' (disconnected)' : !a.plan_enabled ? ' (consent required)' : ''}</option>)}
    </select>}
    <div className="flex flex-wrap gap-2">
      <button className="border rounded px-2 py-1 text-sm" disabled={busy} onClick={() => void signIn(status.active)}>Continue with ChatGPT</button>
      {status.accounts.length > 0 && <button className="border rounded px-2 py-1 text-sm" disabled={busy} onClick={() => void signIn(null)}>Add account</button>}
      {active?.connected && !active.plan_enabled && <button className="border rounded px-2 py-1 text-sm" disabled={busy} onClick={() => void signIn(status.active, true)}>Grant plan access</button>}
      {active?.connected && <button className="border rounded px-2 py-1 text-sm" disabled={busy} onClick={() => void signOut()}>Sign out</button>}
      <button className="border rounded px-2 py-1 text-sm" disabled={busy} onClick={() => void refresh().catch(e => setMessage(String(e)))}>Refresh</button>
      <button className="text-sm underline" onClick={() => void invoke('chatgpt_open_usage').catch(e => setMessage(String(e)))}>Usage</button>
    </div>
    {catalog?.account === status.active && <div className="flex flex-wrap gap-2">
      <select aria-label="ChatGPT plan model" className="border rounded px-2 py-1 text-sm" disabled={busy}
        value={catalog.models.some(m => m.slug === model) ? model : ''} onChange={e => { setModel(e.target.value); setMessage(''); }}>
        <option value="">Select model</option>
        {catalog.models.map(m => <option key={m.slug} value={m.slug}>{m.display_name} ({m.slug})</option>)}
      </select>
      <button className="border rounded px-2 py-1 text-sm" disabled={!ready} onClick={() => void testFixture()}>Test connection</button>
    </div>}
    {message && <p role="status" className="text-xs break-words">{message}</p>}
  </section>;
}
