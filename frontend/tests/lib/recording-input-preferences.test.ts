import { expect, test } from 'bun:test';
import { createRecordingInputPreferenceWriter } from '../../src/lib/recording-input-preferences';

test('merges device selection without changing recording save settings', async () => {
  let preferences: Record<string, unknown> = { auto_save: true, save_folder: '/synthetic', retained: 7 };
  const committed: unknown[] = [];
  const update = createRecordingInputPreferenceWriter({
    read: async () => preferences,
    write: async value => { preferences = value; },
    commit: value => committed.push(value),
  });
  await update({ micDevice: 'fixture_mic (input)', systemDevice: 'pulse-monitor:fixture_pc.monitor (output)' });
  expect(preferences.auto_save).toBe(true);
  expect(preferences.save_folder).toBe('/synthetic');
  expect(preferences.retained).toBe(7);
  expect(committed).toEqual([{ micDevice: 'fixture_mic (input)', systemDevice: 'pulse-monitor:fixture_pc.monitor (output)' }]);
});

test('rapid selections are serialized and commit only after each successful write', async () => {
  const order: string[] = [];
  let preferences: Record<string, unknown> = { auto_save: false };
  const update = createRecordingInputPreferenceWriter({
    read: async () => { order.push('read'); return preferences; },
    write: async value => { order.push(String(value.preferred_mic_device)); await Promise.resolve(); preferences = value; },
    commit: value => order.push('commit:' + value.micDevice),
  });
  await Promise.all([update({ micDevice: 'first', systemDevice: null }), update({ micDevice: 'second', systemDevice: 'speaker' })]);
  expect(order).toEqual(['read', 'first', 'commit:first', 'read', 'second', 'commit:second']);
  expect(preferences.preferred_mic_device).toBe('second');
  expect(preferences.preferred_system_device).toBe('speaker');
  expect(preferences.auto_save).toBe(false);
});

test('failed write does not replace Start selection and does not poison later writes', async () => {
  let fail = true;
  const committed: unknown[] = [];
  const update = createRecordingInputPreferenceWriter({
    read: async () => ({ auto_save: true }),
    write: async () => { if (fail) throw new Error('synthetic save failure'); },
    commit: value => committed.push(value),
  });
  await expect(update({ micDevice: 'failed', systemDevice: null })).rejects.toThrow();
  expect(committed).toEqual([]);
  fail = false;
  await update({ micDevice: null, systemDevice: null });
  expect(committed).toEqual([{ micDevice: null, systemDevice: null }]);
});

test('snapshots caller choice before asynchronous read completes', async () => {
  const choice = { micDevice: 'chosen', systemDevice: null as string | null };
  let saved: Record<string, unknown> = {};
  const update = createRecordingInputPreferenceWriter({ read: async () => ({}),
    write: async value => { saved = value; }, commit: () => {} });
  const pending = update(choice);
  choice.micDevice = 'mutated';
  await pending;
  expect(saved.preferred_mic_device).toBe('chosen');
});
