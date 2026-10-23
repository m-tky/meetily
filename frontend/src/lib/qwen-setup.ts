export type QwenSettings = { worker_path: string; model_dir: string; ort_library: string };
export type QwenSetupInfo = { available: boolean; model_dir: string | null; download_bytes: number; minimum_free_bytes: number };
export type QwenSetupStatus = { id: string; model: string; phase: string; file: string; downloaded: number; total: number; error: string | null; settings: QwenSettings | null };
export const setupActive = (s: QwenSetupStatus | null) => !!s && !['ready', 'failed', 'cancelled'].includes(s.phase);

type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
// Only the explicit start action may auto-select. Mount/poll/reload never does.
export async function prepareAndSelectQwen({ consent, model = 'Qwen3-ASR-1.7B-INT8', invoke, onStatus, onPrepared, isActive, wait }: {
  consent: boolean; model?: string; invoke: Invoke;
  onStatus: (status: QwenSetupStatus) => void;
  onPrepared: (settings: QwenSettings) => Promise<void>;
  isActive: () => boolean; wait: () => Promise<void>;
}) {
  if (!consent) throw new Error('Consent is required before downloading Qwen.');
  const id = await invoke<string>('qwen_setup_start', { consent: true, model });
  while (isActive()) {
    const status = await invoke<QwenSetupStatus | null>('qwen_setup_status');
    if (!status || status.id !== id || status.model !== model) throw new Error('Setup changed. Review its status before selecting Qwen.');
    onStatus(status);
    if (status.phase === 'ready') {
      if (!status.settings) throw new Error('Prepared model settings are missing.');
      if (isActive()) await onPrepared(status.settings);
      return;
    }
    if (!setupActive(status)) throw new Error(status.error || `Setup ${status.phase}.`);
    await wait();
  }
}
