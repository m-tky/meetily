// Isolated hook harness: no network, native processes, models or user profiles.
import { expect, mock, test } from 'bun:test';
import * as React from 'react';
let states: any[] = [], refs: any[] = [], effects: (() => any)[] = [];
let stateIndex = 0, refIndex = 0;
const hooks = { ...React,
  useState(initial: any) { const i = stateIndex++; if (!(i in states)) states[i] = initial; return [states[i], (v: any) => { states[i] = typeof v === 'function' ? v(states[i]) : v; }]; },
  useRef(initial: any) { const i = refIndex++; return refs[i] ??= { current: initial }; },
  useEffect(effect: () => any) { effects.push(effect); },
};
mock.module('react', () => ({ ...hooks, default: hooks }));
const calls: string[] = [];
let nativeStatus: any = null;
let available = true;
let settings = { worker_path: 'C:\\qwen\\worker.exe', model_dir: 'C:\\models', ort_library: 'C:\\qwen\\onnxruntime.dll' };
mock.module('@tauri-apps/api/core', () => ({ invoke: async (name: string) => {
  calls.push(name);
  if (name === 'qwen_setup_info') return { available, model_dir: settings.model_dir, download_bytes: 4700000000, minimum_free_bytes: 7000000000 };
  if (name === 'qwen_setup_start') { nativeStatus = { id: 'run', phase: 'ready', settings, error: null }; return 'run'; }
  if (name === 'qwen_setup_status') return nativeStatus;
} }));
const { QwenAutoSetup } = await import('../../src/components/QwenAutoSetup');
function elements(node: any): React.ReactElement<any>[] {
  if (Array.isArray(node)) return node.flatMap(elements);
  return React.isValidElement(node) ? [node, ...elements((node.props as any).children)] : [];
}
const tick = () => new Promise(resolve => setTimeout(resolve, 0));
async function mount() {
  states = []; refs = []; calls.length = 0; available = true;
  const selected = mock(async (_s: any, _shouldApply: () => boolean) => {}), busy = mock((_b: boolean) => {});
  const props = { disabled: false, onPrepared: selected, onBusyChange: busy };
  const render = () => { stateIndex = 0; refIndex = 0; effects = []; return QwenAutoSetup(props); };
  render(); const cleanups = effects.map(e => e()).filter(e => typeof e === 'function'); await tick();
  return { selected, busy, render, close: () => cleanups.forEach(f => f()) };
}
test('mount only reads status; unchecked consent disables setup and does not select', async () => {
  nativeStatus = null; const h = await mount();
  try {
    const all = elements(h.render());
    const start = all.find(e => e.props.children === 'Agree and set up Qwen')!;
    expect(start.props.disabled).toBe(true); await start.props.onClick();
    expect(calls).toEqual(['qwen_setup_info', 'qwen_setup_status']);
    expect(h.selected).not.toHaveBeenCalled();
  } finally { h.close(); }
});
test('explicit checkbox + start prepares then selects exactly once', async () => {
  nativeStatus = null; const h = await mount();
  try {
    elements(h.render()).find(e => e.type === 'input')!.props.onChange({ target: { checked: true } });
    const start = elements(h.render()).find(e => e.props.children === 'Agree and set up Qwen')!;
    expect(start.props.disabled).toBe(false); await start.props.onClick();
    expect(calls.filter(c => c === 'qwen_setup_start')).toHaveLength(1);
    expect(h.selected).toHaveBeenCalledTimes(1); expect(h.selected.mock.calls[0][0]).toEqual(settings);
    expect(h.selected.mock.calls[0][1]()).toBe(true);
    h.close(); expect(h.selected.mock.calls[0][1]()).toBe(false);
  } finally { h.close(); }
});
test('Linux uses the same consent UI and selects prepared Linux paths', async () => {
  const previous = settings;
  settings = { worker_path: '/app/libexec/qwen-speech-worker', model_dir: '/data/models/Qwen3-ASR-1.7B', ort_library: '/app/libexec/libonnxruntime.so' };
  nativeStatus = null; const h = await mount();
  try {
    const all = elements(h.render());
    expect(all.find(e => e.type === 'h3')!.props.children).toBe('Set up Qwen locally');
    expect(all.find(e => e.props.children === 'Agree and set up Qwen')!.props.disabled).toBe(true);
    expect(calls).not.toContain('qwen_setup_start');
    all.find(e => e.type === 'input')!.props.onChange({ target: { checked: true } });
    await elements(h.render()).find(e => e.props.children === 'Agree and set up Qwen')!.props.onClick();
    expect(h.selected).toHaveBeenCalledTimes(1);
    expect(h.selected.mock.calls[0][0]).toEqual(settings);
  } finally { h.close(); settings = previous; }
});
test('remounting ready state requires an explicit Use prepared action', async () => {
  nativeStatus = { id: 'old', phase: 'ready', settings, error: null }; const h = await mount();
  try {
    expect(h.selected).not.toHaveBeenCalled();
    const use = elements(h.render()).find(e => e.props.children === 'Use prepared Qwen')!;
    await use.props.onClick(); expect(h.selected).toHaveBeenCalledTimes(1);
    expect(calls).not.toContain('qwen_setup_start');
  } finally { h.close(); }
});
test('a restored running setup exposes cancellation and locks consent', async () => {
  nativeStatus = { id: 'old', phase: 'initializing', downloaded: 4, total: 4, error: null }; const h = await mount();
  try {
    const all = elements(h.render());
    expect(all.find(e => e.type === 'input')!.props.disabled).toBe(true);
    await all.find(e => e.props.children === 'Cancel setup')!.props.onClick();
    expect(calls).toContain('qwen_setup_cancel'); expect(h.selected).not.toHaveBeenCalled();
  } finally { h.close(); }
});
