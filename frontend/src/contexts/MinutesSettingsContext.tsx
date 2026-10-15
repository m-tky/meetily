'use client';
import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from 'react';
import { Store } from '@tauri-apps/plugin-store';
import { defaultMinutesSettings, MinutesSettingsRepository, type MinutesSettings } from '@/lib/minutes-settings';

type State = { settings: MinutesSettings; loaded: boolean; error: string; saving: boolean;
  update: (patch: Partial<MinutesSettings>) => Promise<void> };
const Context = createContext<State | null>(null);
export function MinutesSettingsProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState(defaultMinutesSettings);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState('');
  const [pending, setPending] = useState(0);
  const repository = useRef<Promise<MinutesSettingsRepository> | null>(null);
  useEffect(() => {
    let active = true;
    repository.current = Store.load('preferences.json').then(store => new MinutesSettingsRepository(store));
    void repository.current.then(r => r.load()).then(value => {
      if (active) { setSettings(value); setLoaded(true); }
    }).catch(() => { if (active) setError('Could not load minutes settings.'); });
    return () => { active = false; };
  }, []);
  const update = useCallback(async (patch: Partial<MinutesSettings>) => {
    if (!repository.current || !loaded) return;
    setPending(n => n + 1); setError('');
    try { setSettings(await (await repository.current).update(patch)); }
    catch { setError('Could not save minutes settings.'); }
    finally { setPending(n => n - 1); }
  }, [loaded]);
  return <Context.Provider value={{ settings, loaded, error, saving: pending > 0, update }}>{children}</Context.Provider>;
}
export function useMinutesSettings() {
  const value = useContext(Context);
  if (!value) throw new Error('MinutesSettingsProvider is required');
  return value;
}
