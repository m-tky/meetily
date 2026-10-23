import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useConfig } from '@/contexts/ConfigContext';
import { Input } from './ui/input';
import { Button } from './ui/button';
import { Label } from './ui/label';
import { QwenAutoSetup } from './QwenAutoSetup';
import { DEFAULT_QWEN_MODEL, QWEN_MODELS, QWEN_LANGUAGES, isQwenModel } from '@/lib/qwen-models';
import type { QwenSettings } from '@/lib/qwen-setup';
const empty = { worker_path: '', model_dir: '', ort_library: '' };
const fields: [keyof QwenSettings,string][] = [ ['worker_path','Local worker executable'],['model_dir','Prepared model directory'],['ort_library','ONNX Runtime shared library'] ];
export function QwenModelSettings({ onSelect, automaticOnly = false }: { onSelect: (model:string) => void | Promise<void>; automaticOnly?: boolean }) {
  const { selectedLanguage, setSelectedLanguage, transcriptModelConfig } = useConfig();
  const [model,setModel] = useState(isQwenModel(transcriptModelConfig.model) ? transcriptModelConfig.model : DEFAULT_QWEN_MODEL);
  const [settings,setSettings] = useState<QwenSettings>(empty);
  const [supported,setSupported] = useState(false);
  const [busy,setBusy] = useState(false);
  const [autoBusy,setAutoBusy] = useState(false);
  const [loading,setLoading] = useState(true);
  const [error,setError] = useState('');
  useEffect(() => { if (isQwenModel(transcriptModelConfig.model)) setModel(transcriptModelConfig.model); }, [transcriptModelConfig.model]);
  useEffect(() => {
    let active=true; setLoading(true); setError(''); setSettings(empty);
    (async()=>{
      try {
        const ok=await invoke<boolean>('qwen_is_supported');
        const saved=await invoke<QwenSettings|null>('qwen_get_settings',{model});
        if(active){setSupported(ok);setSettings(saved || empty);}
      }catch(e){if(active)setError(String(e));}
      finally{if(active)setLoading(false);}
    })();
    return()=>{active=false;};
  },[model]);
  const languageOk=QWEN_LANGUAGES.some(l=>l.code===selectedLanguage);
  const select=async(candidate=settings,shouldApply=()=>true)=>{
    setBusy(true);setError('');
    try{
      await invoke('qwen_validate_settings',{settings:candidate,model});
      if(!shouldApply())throw new Error('Selection cancelled; prepared files were kept.');
      await invoke('set_language_preference',{language:selectedLanguage});
      await invoke('qwen_save_settings',{settings:candidate});
      await invoke('api_save_transcript_config',{provider:'qwen',model,apiKey:null});
      setSettings(candidate);
      await onSelect(model);
    }catch(e){setError(String(e));throw e;}
    finally{setBusy(false);}
  };
  const locked=busy||autoBusy;
  return <div className="space-y-3 mt-4">
    <Label htmlFor="qwen-model">Transcription model</Label>
    <select id="qwen-model" className="block border rounded p-2" value={model} disabled={locked} onChange={e=>setModel(e.target.value)}>
      {QWEN_MODELS.map(m=><option key={m.id} value={m.id}>{m.label}</option>)}
    </select>
    {!automaticOnly && <details><summary>Advanced local paths</summary>{fields.map(([key,label])=><div key={key}>
      <Label htmlFor={`qwen-${key}`}>{label}</Label><Input id={`qwen-${key}`} value={settings[key]} disabled={locked||loading} onChange={e=>setSettings(s=>({...s,[key]:e.target.value}))}/>
    </div>)}</details>}
    <Label htmlFor="qwen-language">Transcription language</Label>
    <select id="qwen-language" className="block border rounded p-2" value={languageOk?selectedLanguage:''} disabled={locked} onChange={e=>setSelectedLanguage(e.target.value)}>
      <option value="" disabled>Select language</option>{QWEN_LANGUAGES.map(l=><option key={l.code} value={l.code}>{l.name}</option>)}
    </select>
    {!supported && !loading && <p className="text-sm">Qwen requires Linux or Windows x64. No other ASR engine is used.</p>}
    {error && <p role="alert" className="text-sm text-red-600">{error}</p>}
    <QwenAutoSetup key={model} model={model} disabled={busy||loading||!languageOk} onPrepared={select} onBusyChange={setAutoBusy}/>
    <Button disabled={locked||loading||!supported||!languageOk} onClick={()=>{void select().catch(()=>{});}}>{busy?'Checking local worker…':'Use installed model'}</Button>
  </div>;
}
