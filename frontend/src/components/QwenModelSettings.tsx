import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useConfig } from '@/contexts/ConfigContext';
import { Input } from './ui/input';
import { Button } from './ui/button';
import { Label } from './ui/label';

type Settings = { worker_path: string; model_dir: string; ort_library: string };
const fields: [keyof Settings, string][] = [
  ['worker_path', 'Local worker executable (absolute path)'],
  ['model_dir', 'Prepared Qwen3-ASR 1.7B model directory'],
  ['ort_library', 'ONNX Runtime shared library (absolute path)'],
];

export function QwenModelSettings({ onSelect }: { onSelect: () => void }) {
  const { selectedLanguage, setSelectedLanguage } = useConfig();
  const [supported, setSupported] = useState(false);
  const [settings, setSettings] = useState<Settings>({ worker_path: '', model_dir: '', ort_library: '' });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  useEffect(() => {
    let active = true;
    (async () => {
      try {
        const ok = await invoke<boolean>('qwen_is_supported');
        const saved = await invoke<Settings | null>('qwen_get_settings');
        if (active) { setSupported(ok); if (saved) setSettings(saved); }
      } catch (e) { if (active) setError(String(e)); }
    })();
    return () => { active = false; };
  }, []);
  const languageOk = ['ja', 'en', 'th'].includes(selectedLanguage);
  const select = async () => {
    setBusy(true); setError('');
    try {
      await invoke('set_language_preference', { language: selectedLanguage });
      await invoke('qwen_save_settings', { settings });
      await invoke('qwen_init');
      // Persist provider only after local setup / handshake succeeds.
      onSelect();
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  };
  return <div className="space-y-3 mt-4">
    {fields.map(([key, label]) => <div key={key}>
      <Label htmlFor={`qwen-${key}`}>{label}</Label>
      <Input id={`qwen-${key}`} value={settings[key]} disabled={busy} onChange={e => setSettings(prev => ({ ...prev, [key]: e.target.value }))} />
    </div>)}
    <Label htmlFor="qwen-language">Transcription language</Label>
    <select id="qwen-language" className="block border rounded p-2" value={languageOk ? selectedLanguage : ''} disabled={busy} onChange={e => setSelectedLanguage(e.target.value)}>
      <option value="" disabled>Select language</option>
      <option value="ja">Japanese</option><option value="en">English</option><option value="th">Thai</option>
    </select>
    {!supported && <p className="text-sm">Qwen requires Linux and a prepared local worker.</p>}
    {error && <p role="alert" className="text-sm text-red-600">{error}</p>}
    <Button disabled={busy || !supported || !languageOk} onClick={select}>{busy ? 'Checking local worker…' : 'Validate and select Qwen'}</Button>
  </div>;
}
