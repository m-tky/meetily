export type MinutesSettings = { provider: 'ollama' | 'chatgpt_plan'; ollamaModel: string; chatgptModel: string };
export const defaultMinutesSettings: MinutesSettings = { provider: 'ollama', ollamaModel: '', chatgptModel: 'gpt-6-luna' };
export function parseMinutesSettings(value: unknown): MinutesSettings {
  const v = value && typeof value === 'object' ? value as Record<string, unknown> : {};
  return { provider: v.provider === 'chatgpt_plan' ? 'chatgpt_plan' : 'ollama',
    ollamaModel: typeof v.ollamaModel === 'string' ? v.ollamaModel : '',
    chatgptModel: typeof v.chatgptModel === 'string' ? v.chatgptModel : defaultMinutesSettings.chatgptModel };
}

type Storage = { get: (key: string) => Promise<unknown>; set: (key: string, value: unknown) => Promise<unknown>; save: () => Promise<unknown> };
/** Preferences only. Credentials stay in the native SIWC store; enabling updates is never persisted. */
export class MinutesSettingsRepository {
  private queue: Promise<unknown> = Promise.resolve();
  constructor(private storage: Storage) {}
  async load() { return parseMinutesSettings(await this.storage.get('minutes_settings')); }
  update(patch: Partial<MinutesSettings>): Promise<MinutesSettings> {
    const operation = this.queue.then(async () => {
      const previous = await this.load();
      const next = parseMinutesSettings({ ...previous, ...patch });
      try { await this.storage.set('minutes_settings', next); await this.storage.save(); }
      catch (error) { await this.storage.set('minutes_settings', previous); throw error; }
      return next;
    });
    this.queue = operation.catch(() => {});
    return operation;
  }
}
