'use client';
import React, {createContext,useContext,useState,useEffect,useRef} from 'react';
import {invoke} from '@tauri-apps/api/core';
import type {PermissionStatus,OnboardingPermissions} from '@/types/onboarding';
interface Context {
  currentStep:number; databaseExists:boolean; initializationError:string;
  permissions:OnboardingPermissions; permissionsSkipped:boolean;
  goToStep:(step:number)=>void; goNext:()=>void; goPrevious:()=>void;
  setPermissionStatus:(permission:keyof OnboardingPermissions,status:PermissionStatus)=>void;
  setPermissionsSkipped:(value:boolean)=>void;
  completeOnboarding:()=>Promise<void>; completeQwenOnboarding:()=>Promise<void>;
}
const OnboardingContext=createContext<Context|undefined>(undefined);
export function OnboardingProvider({children}:{children:React.ReactNode}) {
  const [databaseExists,setDatabaseExists]=useState(false);
  const [initializationError,setInitializationError]=useState('');
  const [permissions,setPermissions]=useState<OnboardingPermissions>({microphone:'not_determined',systemAudio:'not_determined',screenRecording:'not_determined'});
  const [permissionsSkipped,setPermissionsSkipped]=useState(false);
  const initialization=useRef<Promise<void>>();
  useEffect(()=>{
    if(initialization.current)return;
    initialization.current=(async()=>{
      if(!await invoke<boolean>('check_first_launch')){setDatabaseExists(true);return;}
      let legacy:string|null=null;
      if(navigator.platform?.toLowerCase().includes('mac')) {
        const path='/usr/local/var/meetily/meeting_minutes.db';
        try{const found=await invoke<{exists:boolean}|null>('check_homebrew_database',{path});if(found?.exists)legacy=path;}catch{}
      }
      if(!legacy){try{legacy=await invoke<string|null>('check_default_legacy_database');}catch{}}
      if(legacy)await invoke('import_and_initialize_database',{legacyDbPath:legacy});
      else await invoke('initialize_fresh_database');
      setDatabaseExists(true);
    })();
    void initialization.current.catch(e=>setInitializationError(String(e)));
  },[]);
  const completeQwenOnboarding=async()=>{
    await initialization.current;
    await invoke('complete_qwen_onboarding');
  };
  return <OnboardingContext.Provider value={{currentStep:1,databaseExists,initializationError,permissions,permissionsSkipped,
    goToStep:()=>{},goNext:()=>{},goPrevious:()=>{},setPermissionsSkipped,
    setPermissionStatus:(permission,status)=>setPermissions(p=>({...p,[permission]:status})),
    completeOnboarding:completeQwenOnboarding,completeQwenOnboarding}}>{children}</OnboardingContext.Provider>;
}
export function useOnboarding(){const value=useContext(OnboardingContext);if(!value)throw new Error('useOnboarding must be used within OnboardingProvider');return value;}
