import { useState } from 'react';
import { useConfig } from '@/contexts/ConfigContext';
import { QWEN_LANGUAGES } from '@/lib/qwen-models';
export interface Language {code:string;name:string}
interface Props {selectedLanguage:string;onLanguageChange:(language:string)=>void;disabled?:boolean;provider?:'qwen'}
export function LanguageSelection({selectedLanguage,onLanguageChange,disabled=false}:Props){
  const {setSelectedLanguage}=useConfig();const [saving,setSaving]=useState(false);
  return <div className="space-y-2"><label htmlFor="transcription-language">Transcription language</label>
    <select id="transcription-language" className="w-full border rounded p-2" value={QWEN_LANGUAGES.some(l=>l.code===selectedLanguage)?selectedLanguage:''} disabled={disabled||saving} onChange={e=>{
      setSaving(true);try{setSelectedLanguage(e.target.value);onLanguageChange(e.target.value);}finally{setSaving(false);}
    }}><option value="" disabled>Select language</option>{QWEN_LANGUAGES.map(l=><option key={l.code} value={l.code}>{l.name}</option>)}</select>
  </div>;
}
