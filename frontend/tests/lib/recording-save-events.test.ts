import { beforeEach, expect, mock, test } from 'bun:test';

const handlers = new Map<string, (event: { payload: unknown }) => void>();
mock.module('@tauri-apps/api/event', () => ({
  listen: async (name: string, callback: (event: { payload: unknown }) => void) => {
    handlers.set(name, callback);
    return () => handlers.delete(name);
  },
}));
const { recordingService } = await import('../../src/services/recordingService');
beforeEach(() => handlers.clear());

test('storage warning is forwarded without changing recording lifecycle', async () => {
  const warnings: string[] = [];
  const unlisten = await recordingService.onRecordingSaveWarning(message => warnings.push(message));
  handlers.get('recording-save-warning')?.({ payload: 'disk full; keep checkpoints' });
  expect(warnings).toEqual(['disk full; keep checkpoints']);
  unlisten();
  expect(handlers.has('recording-save-warning')).toBe(false);
});

test('stopped payload preserves storage failure separately from recording stop', async () => {
  const events: unknown[] = [];
  await recordingService.onRecordingStopped(payload => events.push(payload));
  const payload = { message: 'Recording stopped', folder_path: '/synthetic/session',
    save_succeeded: false, save_error: 'metadata commit failed' };
  handlers.get('recording-stopped')?.({ payload });
  expect(events).toEqual([payload]);
});
