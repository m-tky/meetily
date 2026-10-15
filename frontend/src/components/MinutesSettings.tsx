'use client';
import { useCallback } from 'react';
import { ChatGptPlanControls } from './ChatGptPlanControls';
import { useMinutesSettings } from '@/contexts/MinutesSettingsContext';

export function MinutesSettings() {
  const { settings, loaded, error, saving, update } = useMinutesSettings();
  const selectModel = useCallback((_account: string | null, model: string) => {
    if (model !== settings.chatgptModel) void update({ chatgptModel: model });
  }, [settings.chatgptModel, update]);
  return <section className="bg-white rounded-lg border border-gray-200 p-6 shadow-sm space-y-4">
    <h3 className="text-lg font-semibold">AI model</h3>
    {error && <p role="alert" className="text-sm text-red-600">{error}</p>}
    {!loaded ? <p role="status">Loading…</p> : <>
      <select aria-label="Minutes provider" className="border rounded px-2 py-1 text-sm" value={settings.provider} disabled={saving}
        onChange={e => void update({ provider: e.target.value as 'ollama' | 'chatgpt_plan' })}>
        <option value="ollama">Ollama</option><option value="chatgpt_plan">ChatGPT plan</option>
      </select>
      {settings.provider === 'chatgpt_plan'
        ? <ChatGptPlanControls initialModel={settings.chatgptModel} onSelect={selectModel} />
        : <input aria-label="Ollama model name" placeholder="Ollama model" className="block border rounded px-2 py-1 text-sm"
            defaultValue={settings.ollamaModel} onBlur={e => void update({ ollamaModel: e.target.value })} />}
    </>}
  </section>;
}
