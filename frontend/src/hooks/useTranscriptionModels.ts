import {useState,useCallback,useRef} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {qwenLabel,isQwenModel} from '@/lib/qwen-models';
export interface RawModelInfo{name:string;size_mb:number;status:'Available'|'Missing'|{Downloading:{progress:number}}|{Error:string}}
export interface ModelOption{provider:'qwen';name:string;displayName:string;size_mb:number}
export function useTranscriptionModels(config:{provider?:string;model?:string}|undefined){
  const [availableModels,setModels]=useState<ModelOption[]>([]);
  const [selectedModelKey,setKey]=useState('');
  const [loadingModels,setLoading]=useState(false);
  const userSelected=useRef(false);
  const fetchModels=useCallback(async()=>{
    setLoading(true);
    try{
      const raw=await invoke<RawModelInfo[]>('qwen_get_available_models');
      const models=raw.filter(m=>m.status==='Available'&&isQwenModel(m.name)).map(m=>({provider:'qwen' as const,name:m.name,displayName:qwenLabel(m.name),size_mb:m.size_mb}));
      setModels(models);
      // Never silently switch to the other installed model when selection is unavailable.
      if(!userSelected.current)setKey(models.some(m=>m.name===config?.model)?`qwen:${config!.model}`:'');
    }catch{setModels([]);setKey('');}
    finally{setLoading(false);}
  },[config]);
  return {availableModels,selectedModelKey,setSelectedModelKey:useCallback((key:string)=>{userSelected.current=true;setKey(key);},[]),loadingModels,fetchModels,resetSelection:useCallback(()=>{userSelected.current=false;setKey('');},[])};
}
