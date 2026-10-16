import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useConfig } from '@/contexts/ConfigContext';
import { Input } from './ui/input';
import { Button } from './ui/button';
import { Label } from './ui/label';
import { QwenAutoSetup } from './QwenAutoSetup';
import type { QwenSetupInfo } from '@/lib/qwen-setup';

type Settings = { worker_path: string; model_dir: string; ort_library: string };
const fields: [keyof Settings, string][] = [
  ['worker_path', 'Local worker executable (absolute path)'],
  ['model_dir', 'Prepared Qwen3-ASR 1.7B model directory'],
  ['ort_library', 'ONNX Runtime shared library (absolute path)'],
];

export function QwenModelSettings({ onSelect, automaticOnly = false }: { onSelect: () => void | Promise<void>; automaticOnly?: boolean }) {
  const { selectedLanguage, setSelectedLanguage } = useConfig();
  const [supported, setSupported] = useState(false);
  const [settings, setSettings] = useState<Settings>({ worker_path: '', model_dir: '', ort_library: '' });
  const [busy, setBusy] = useState(false);
  const [autoBusy, setAutoBusy] = useState(false);
  const [automaticAvailable, setAutomaticAvailable] = useState(false);
  const [error, setError] = useState('');
  useEffect(() => {
    let active = true;
    (async () => {
      try {
        const ok = await invoke<boolean>('qwen_is_supported');
        const automatic = await invoke<QwenSetupInfo>('qwen_setup_info');
        if (active) { setSupported(ok); setAutomaticAvailable(automatic.available); }
        const saved = await invoke<Settings | null>('qwen_get_settings');
        if (active && saved) setSettings(saved);
      } catch (e) { if (active) setError(String(e)); }
    })();
    return () => { active = false; };
  }, []);
  const languageOk = ['ja', 'en', 'th'].includes(selectedLanguage);
  const select = async (candidate = settings, shouldApply = () => true) => {
    setBusy(true); setError('');
    try {
      await invoke('qwen_validate_settings', { settings: candidate });
      if (!shouldApply()) throw new Error('Qwen selection cancelled; prepared files were kept.');
      await invoke('set_language_preference', { language: selectedLanguage });
      await invoke('qwen_save_settings', { settings: candidate });
      setSettings(candidate);
      await invoke('qwen_init');
      // Settings-page selection must survive restart, just like other local ASRs.
      // Persist only after local setup / handshake succeeds; no silent fallback.
      await invoke('api_save_transcript_config', { provider: 'qwen', model: 'Qwen3-ASR-1.7B-INT8', apiKey: null });
      await onSelect();
    } catch (e) { setError(String(e)); throw e; }
    finally { setBusy(false); }
  };
  if (automaticOnly && !automaticAvailable) return null;
  return <div className="space-y-3 mt-4">
    {!automaticOnly && fields.map(([key, label]) => <div key={key}>
      <Label htmlFor={`qwen-${key}`}>{label}</Label>
      <Input id={`qwen-${key}`} value={settings[key]} disabled={busy || autoBusy} onChange={e => setSettings(prev => ({ ...prev, [key]: e.target.value }))} />
    </div>)}
    <Label htmlFor="qwen-language">Transcription language</Label>
    <select id="qwen-language" className="block border rounded p-2" value={languageOk ? selectedLanguage : ''} disabled={busy || autoBusy} onChange={e => setSelectedLanguage(e.target.value)}>
      <option value="" disabled>Select language</option>
      <option value="ja">Japanese</option><option value="en">English</option><option value="th">Thai</option>
    </select>
    {!supported && <p className="text-sm">Qwen requires Linux or Windows x64 and a prepared local model.</p>}
    {error && <p role="alert" className="text-sm text-red-600">{error}</p>}
    <QwenAutoSetup disabled={busy || !languageOk} onPrepared={select} onBusyChange={setAutoBusy} />
    {!automaticOnly && <Button disabled={busy || autoBusy || !supported || !languageOk} onClick={() => { void select().catch(() => {}); }}>{busy ? 'Checking local worker…' : 'Validate and select Qwen'}</Button>}
  </div>;
}
