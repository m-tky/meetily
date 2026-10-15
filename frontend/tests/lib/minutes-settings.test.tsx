import { test, expect, mock, afterEach } from 'bun:test';
import React from 'react';
import { create, act, type ReactTestRenderer } from 'react-test-renderer';
import { MinutesSettingsRepository, parseMinutesSettings } from '../../src/lib/minutes-settings';

const data = new Map<string, unknown>();
const calls: string[] = [];
let available = true;
let recording = false;
let connected = true;
let planEnabled = true;
const storage = { get: async (key: string) => data.get(key), set: async (key: string, value: unknown) => { data.set(key, value); }, save: async () => {} };
mock.module('@tauri-apps/plugin-store', () => ({ Store: { load: async () => storage } }));
mock.module('@tauri-apps/api/core', () => ({ invoke: async (name: string) => {
  calls.push(name);
  if (name === 'chatgpt_status') return { active: 'fixture', accounts: [{ client_id: 'fixture', label: 'Fixture', connected, plan_enabled: planEnabled }] };
  if (name === 'chatgpt_list_models') return available ? [{ slug: 'gpt-6-luna', display_name: 'Luna' }, { slug: 'fixture-alt', display_name: 'Alternate' }] : [];
  if (name === 'api_get_live_minutes_session') return null;
  throw new Error('Unexpected command: ' + name);
} }));
mock.module('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));
mock.module('next/link', () => ({ default: (props: any) => <a {...props} /> }));
mock.module('../../src/contexts/RecordingStateContext', () => ({ useRecordingState: () => ({ isRecording: recording }) }));
const { MinutesSettingsProvider } = await import('../../src/contexts/MinutesSettingsContext');
const { MinutesSettings } = await import('../../src/components/MinutesSettings');
const { LiveMinutesPanel } = await import('../../src/components/LiveMinutesPanel');
let view: ReactTestRenderer | undefined;
afterEach(() => { if (view) act(() => view!.unmount()); view = undefined; data.clear(); calls.length = 0; available = true; recording = false; connected = true; planEnabled = true; });
const tick = async () => { await new Promise(resolve => setTimeout(resolve, 0)); };
async function mount(child: React.ReactNode) { await act(async () => { view = create(<MinutesSettingsProvider>{child}</MinutesSettingsProvider>); await tick(); }); }

test('normalizes preferences without credentials or automatic update consent', () => {
  const value = parseMinutesSettings({ provider: 'chatgpt_plan', chatgptModel: 'fixture-alt', token: 'never-store', enabled: true });
  expect(value).toEqual({ provider: 'chatgpt_plan', chatgptModel: 'fixture-alt', ollamaModel: '' });
  expect(parseMinutesSettings(null).provider).toBe('ollama');
});
test('serializes writes and reloads preferences', async () => {
  const repository = new MinutesSettingsRepository(storage);
  await Promise.all([repository.update({ provider: 'chatgpt_plan' }), repository.update({ chatgptModel: 'fixture-alt' })]);
  expect(await new MinutesSettingsRepository(storage).load()).toEqual({ provider: 'chatgpt_plan', chatgptModel: 'fixture-alt', ollamaModel: '' });
});
test('failed save restores the previous in-memory preference and permits retry', async () => {
  let fail = true;
  const repository = new MinutesSettingsRepository({ ...storage, save: async () => { if (fail) throw new Error('fixture write failure'); } });
  await expect(repository.update({ provider: 'chatgpt_plan' })).rejects.toThrow();
  expect((await repository.load()).provider).toBe('ollama');
  fail = false;
  expect((await repository.update({ provider: 'chatgpt_plan' })).provider).toBe('chatgpt_plan');
});
test('settings contain authentication and persist explicit model selection without inference', async () => {
  data.set('minutes_settings', { provider: 'chatgpt_plan', chatgptModel: 'gpt-6-luna' });
  await mount(<MinutesSettings />);
  expect(view!.root.findByProps({ 'aria-label': 'ChatGPT plan model' }).props.value).toBe('gpt-6-luna');
  await act(async () => { view!.root.findByProps({ 'aria-label': 'ChatGPT plan model' }).props.onChange({ target: { value: 'fixture-alt' } }); await tick(); });
  expect((data.get('minutes_settings') as any).chatgptModel).toBe('fixture-alt');
  expect(calls.every(name => ['chatgpt_status', 'chatgpt_list_models'].includes(name))).toBe(true);
});
test('recording panel restores selected model but has no authentication or model selector and never auto-starts', async () => {
  data.set('minutes_settings', { provider: 'chatgpt_plan', chatgptModel: 'fixture-alt' });
  await mount(<LiveMinutesPanel />);
  expect(view!.root.findAllByType('select')).toHaveLength(0);
  expect(JSON.stringify(view!.toJSON())).toContain('fixture-alt');
  expect(JSON.stringify(view!.toJSON())).not.toContain('Continue with ChatGPT');
  expect(view!.root.findAllByType('button').find(b => b.children.includes('Start updates'))!.props.disabled).toBe(true);
  expect(calls).not.toContain('api_update_live_minutes');
  expect(calls).not.toContain('chatgpt_sign_in');
});
for (const mode of ['disconnected', 'no-consent']) test(`${mode} account blocks updates`, async () => {
  connected = mode !== 'disconnected'; planEnabled = mode !== 'no-consent'; recording = true;
  data.set('minutes_settings', { provider: 'chatgpt_plan', chatgptModel: 'gpt-6-luna' });
  await mount(<LiveMinutesPanel />);
  expect(view!.root.findAllByType('button').find(b => b.children.includes('Start updates'))!.props.disabled).toBe(true);
  expect(calls).not.toContain('chatgpt_list_models');
  expect(calls).not.toContain('api_update_live_minutes');
});

test('unavailable model blocks updates with no fallback or preference overwrite', async () => {
  available = false; recording = true;
  data.set('minutes_settings', { provider: 'chatgpt_plan', chatgptModel: 'fixture-alt' });
  await mount(<LiveMinutesPanel />);
  expect(view!.root.findAllByType('button').find(b => b.children.includes('Start updates'))!.props.disabled).toBe(true);
  expect((data.get('minutes_settings') as any).chatgptModel).toBe('fixture-alt');
  expect(calls).not.toContain('api_update_live_minutes');
});
