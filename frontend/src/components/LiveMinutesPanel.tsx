'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useRecordingState } from '@/contexts/RecordingStateContext';
import { ChatGptPlanControls } from './ChatGptPlanControls';

type Item = {
  id: string; kind: 'point' | 'decision' | 'todo' | 'question';
  text: string; status: 'active' | 'superseded' | 'withdrawn';
  sources: string[]; owner: string | null; due: string | null;
};
type Snapshot = { revision: number; updated_at: string; items: Item[] };
const headings: Record<Item['kind'], string> = {
  point: '要点', decision: '決定事項', todo: 'ToDo', question: '未解決の論点',
};

/** Experimental opt-in worker: sequential polling, never on the audio path. */
export function LiveMinutesPanel() {
  const { isRecording } = useRecordingState();
  const [enabled, setEnabled] = useState(false);
  const [model, setModel] = useState('');
  const [provider, setProvider] = useState<'ollama' | 'chatgpt_plan'>('ollama');
  const [account, setAccount] = useState<string | null>(null);
  const [cloudModel, setCloudModel] = useState('');
  const selectedModel = provider === 'ollama' ? model : cloudModel;
  const onCloudSelect = useCallback((id: string | null, slug: string) => {
    setAccount(id); setCloudModel(slug); setEnabled(false);
  }, []);
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [message, setMessage] = useState('録音開始後、インストール済みのOllamaモデル名を指定してください。');
  const inFlight = useRef(false);
  const session = useRef(0);

  useEffect(() => {
    session.current += 1;
    if (isRecording) setSnapshot(null);
    setEnabled(false);
  }, [isRecording]);

  useEffect(() => {
    if (!enabled || !isRecording || !selectedModel.trim() || (provider === 'chatgpt_plan' && !account)) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    const currentSession = session.current;
    let failures = 0;
    const update = async () => {
      if (inFlight.current) {
        timer = setTimeout(update, 1000);
        return;
      }
      inFlight.current = true;
      setMessage('更新中（録音・文字起こしは継続します）');
      let stop = false;
      let nextDelay = 15000;
      try {
        const result = await invoke<Snapshot>('api_update_live_minutes', { model: selectedModel.trim(), provider, account });
        if (!cancelled && currentSession === session.current) {
          failures = 0;
          setSnapshot(result);
          setMessage(`revision ${result.revision} / ${result.updated_at}`);
        }
      } catch (error) {
        if (!cancelled && currentSession === session.current) {
          const text = String(error); setMessage(text);
          if (text.includes('Waiting for transcript evidence') || text.includes('No current meeting folder')) {
            // Waiting costs no model request and must not stop recording.
          } else if (text.startsWith('CHATGPT_TEMPORARY:') || text.includes('timed out')) {
            failures += 1; nextDelay = Math.min(15000 * 2 ** failures, 300000);
            if (failures >= 3) { stop = true; setEnabled(false); }
          } else { stop = true; setEnabled(false); }
        }
      } finally {
        inFlight.current = false;
        if (!cancelled && !stop) timer = setTimeout(update, nextDelay);
      }
    };
    void update();
    return () => { cancelled = true; clearTimeout(timer); };
  }, [enabled, isRecording, selectedModel, provider, account]);

  return (
    <details className="border-b border-border px-5 py-3 shrink-0" open={enabled}>
      <summary className="cursor-pointer text-sm font-medium">リアルタイム議事録（実験版）</summary>
      <select aria-label="Minutes provider" className="border rounded px-2 py-1 text-sm mt-3" value={provider} disabled={enabled}
        onChange={(e) => { setEnabled(false); setProvider(e.target.value as typeof provider); }}>
        <option value="ollama">ローカル Ollama</option>
        <option value="chatgpt_plan">公式ChatGPT plan（文字起こし・議事録をクラウド送信）</option>
      </select>
      {provider === 'chatgpt_plan' && <div className="mt-3"><ChatGptPlanControls onSelect={onCloudSelect} /></div>}
      <div className="flex items-center gap-3 mt-3">
        {provider === 'ollama' && <input aria-label="Ollama model name" placeholder="インストール済みOllamaモデル名"
          className="border rounded px-2 py-1 bg-background text-sm w-72"
          value={model} disabled={enabled} onChange={(e) => setModel(e.target.value)} />}
        <button className="border rounded px-3 py-1 text-sm" disabled={!isRecording || !selectedModel.trim() || (provider === 'chatgpt_plan' && !account)}
          onClick={() => setEnabled((value) => !value)}>{enabled ? '更新を停止' : '継続更新を開始'}</button>
      </div>
      <p className="text-xs text-muted-foreground mt-2">AI草案・要確認。会議フォルダの live-minutes.json / live-minutes.md に保存します。</p>
      <p role="status" className="text-xs mt-1">{message}</p>
      {snapshot && <div className="max-h-64 overflow-y-auto mt-3 grid md:grid-cols-2 gap-3">
        {(Object.keys(headings) as Item['kind'][]).map((kind) => <section key={kind}>
          <h3 className="text-sm font-medium">{headings[kind]}</h3>
          <ul className="text-sm list-disc pl-5">{snapshot.items.filter((i) => i.kind === kind && i.status === 'active').map((item) =>
            <li key={item.id}>{item.text}{item.owner && ` / 担当: ${item.owner}`}{item.due && ` / 期限: ${item.due}`}
              <span className="block text-xs text-muted-foreground">根拠: {item.sources.join(', ')}</span>
            </li>)}</ul>
        </section>)}
        <section><h3 className="text-sm font-medium">変更・撤回履歴</h3>
          {snapshot.items.filter((i) => i.status !== 'active').map((i) => <p className="text-xs" key={i.id}>
            [{i.status}] {i.text} / 根拠: {i.sources.join(', ')}
          </p>)}
        </section>
      </div>}
    </details>
  );
}
