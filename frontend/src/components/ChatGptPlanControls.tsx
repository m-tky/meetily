'use client';

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

type Account = { client_id: string; label: string; connected: boolean; plan_enabled: boolean };
type AuthStatus = { active: string | null; accounts: Account[] };
type Model = { slug: string; display_name: string };
type Fixture = { elapsed_ms: number; passed: boolean; items: { text: string; status: string }[] };

/** Only public account metadata crosses IPC; tokens stay in the native protected store. */
export function ChatGptPlanControls({ onSelect }: { onSelect?: (account: string | null, model: string) => void }) {
  const [status, setStatus] = useState<AuthStatus>({ active: null, accounts: [] });
  const [catalog, setCatalog] = useState<{ account: string; models: Model[] } | null>(null);
  const [model, setModel] = useState('gpt-6-luna');
  const [catalogEpoch, setCatalogEpoch] = useState(0);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('公式Sign in with ChatGPT。アプリ専用にログインし、既存Codexの認証は使いません。');
  const [fixture, setFixture] = useState<Fixture | null>(null);
  const active = status.accounts.find((a) => a.client_id === status.active);
  const ready = !busy && !!active?.connected && !!active.plan_enabled
    && catalog?.account === active.client_id && catalog.models.some((m) => m.slug === model);

  async function refresh() { setStatus(await invoke<AuthStatus>('chatgpt_status')); setCatalogEpoch((value) => value + 1); }
  useEffect(() => { void refresh().catch((e) => setMessage(String(e))); }, []);
  useEffect(() => {
    let cancelled = false;
    setCatalog(null);
    if (active?.connected && active.plan_enabled) {
      void invoke<Model[]>('chatgpt_list_models', { clientId: active.client_id }).then((models) => {
        if (cancelled) return;
        setCatalog({ account: active.client_id, models });
        setMessage(models.some((m) => m.slug === 'gpt-6-luna')
          ? 'Lunaがアカウントの一覧にあります。実際の利用可否は推論の完了で確認します。'
          : 'gpt-6-lunaがこのアカウントの一覧にありません。自動で別モデルへ切り替えません。');
      }).catch((e) => { if (!cancelled) setMessage(String(e)); });
    }
    return () => { cancelled = true; };
  }, [active?.client_id, active?.connected, active?.plan_enabled, catalogEpoch]);
  useEffect(() => { onSelect?.(ready ? status.active : null, ready ? model : ''); }, [ready, status.active, model, onSelect]);

  async function signIn(clientId: string | null, consent = false) {
    setBusy(true); setMessage('システムブラウザでログインと同意を完了してください（最大5分）。');
    try { setStatus(await invoke<AuthStatus>('chatgpt_sign_in', { clientId, consent })); }
    catch (e) { setMessage(String(e)); await refresh().catch(() => undefined); }
    finally { setBusy(false); }
  }
  async function selectAccount(clientId: string) {
    setBusy(true); setFixture(null);
    try { setStatus(await invoke<AuthStatus>('chatgpt_select_account', { clientId })); }
    catch (e) { setMessage(String(e)); }
    finally { setBusy(false); }
  }
  async function signOut() {
    setBusy(true); setFixture(null);
    try { setMessage(await invoke<string>('chatgpt_sign_out')); await refresh(); }
    catch (e) { setMessage(String(e)); }
    finally { setBusy(false); }
  }
  async function testFixture() {
    if (!ready || !status.active) return;
    setBusy(true); setMessage('日本語の人工文だけを送信して、決定の撤回・未解決論点を検証中。実際の録音は読みません。');
    try {
      const result = await invoke<Fixture>('api_test_live_minutes_fixture', { account: status.active, model });
      setFixture(result);
      setMessage(`${result.passed ? '意味fixture成功' : '意味fixture失敗（実会議には未採用）'} / ${(result.elapsed_ms / 1000).toFixed(2)}秒`);
    } catch (e) { setMessage(String(e)); }
    finally { setBusy(false); }
  }

  return <section className="border rounded-lg p-3 text-left bg-white text-gray-900">
    <h3 className="text-sm font-semibold">Using ChatGPT plan（公式・preview）</h3>
    <p className="text-xs text-gray-600 mt-1">クラウドを選ぶと文字起こし・既存議事録をOpenAIへ送信し、ChatGPTプランの利用枠を使います。音声はローカルに留めます。</p>
    {status.accounts.length > 0 && <select aria-label="ChatGPT account" className="border rounded px-2 py-1 mt-2 max-w-full text-sm"
      value={status.active ?? ''} disabled={busy} onChange={(e) => void selectAccount(e.target.value)}>
      <option value="" disabled>アカウントを選択</option>
      {status.accounts.map((a) => <option key={a.client_id} value={a.client_id}>{a.label}{!a.connected ? '（未接続）' : !a.plan_enabled ? '（プラン利用同意なし）' : ''}</option>)}
    </select>}
    <div className="flex flex-wrap gap-2 mt-2">
      <button className="border rounded px-2 py-1 text-sm" disabled={busy} onClick={() => void signIn(status.active)}>Continue with ChatGPT</button>
      {status.accounts.length > 0 && <button className="border rounded px-2 py-1 text-sm" disabled={busy} onClick={() => void signIn(null)}>別アカウント／workspaceを追加</button>}
      {active?.connected && !active.plan_enabled && <button className="border rounded px-2 py-1 text-sm" disabled={busy} onClick={() => void signIn(status.active, true)}>プラン利用に同意する</button>}
      {active?.connected && <button className="border rounded px-2 py-1 text-sm" disabled={busy} onClick={() => void signOut()}>サインアウト</button>}
      <button className="border rounded px-2 py-1 text-sm" disabled={busy} onClick={() => void refresh().catch((e) => setMessage(String(e)))}>接続状態を再確認</button>
      <button className="text-sm underline" onClick={() => void invoke('chatgpt_open_usage').catch((e) => setMessage(String(e)))}>Manage usage</button>
    </div>
    {catalog?.account === status.active && <div className="flex flex-wrap gap-2 mt-2">
      <select aria-label="ChatGPT plan model" className="border rounded px-2 py-1 text-sm" disabled={busy} value={catalog.models.some((m) => m.slug === model) ? model : ''} onChange={(e) => { setModel(e.target.value); setFixture(null); }}>
        <option value="">アカウントで利用可能なモデルを選択</option>
        {catalog.models.map((m) => <option key={m.slug} value={m.slug}>{m.display_name} ({m.slug})</option>)}
      </select>
      <button className="border rounded px-2 py-1 text-sm" disabled={!ready} onClick={() => void testFixture()}>人工文の議事録テスト（プラン枠を使用）</button>
    </div>}
    <p role="status" className="text-xs mt-2 break-words">{message}</p>
    {fixture && <ul className="text-xs mt-2 list-disc pl-5">{fixture.items.map((item, i) => <li key={i}>[{item.status}] {item.text}</li>)}</ul>}
  </section>;
}
