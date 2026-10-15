// Isolated Bun process: deterministic React hook harness, no DOM/audio hardware.
import { expect, mock, test } from 'bun:test';
import * as React from 'react';
let states: unknown[] = [];
let refs: { current: unknown }[] = [];
let stateIndex = 0, refIndex = 0;
let effects: (() => unknown)[] = [];
const hookReact = { ...React,
  useState(initial: unknown) {
    const index = stateIndex++;
    if (!(index in states)) states[index] = initial;
    return [states[index], (next: unknown) => { states[index] = typeof next === 'function' ? (next as Function)(states[index]) : next; }];
  },
  useRef(initial: unknown) { const index = refIndex++; return refs[index] ??= { current: initial }; },
  useCallback: (callback: unknown) => callback,
  useId: () => 'fixture-picker',
  useEffect: (effect: () => unknown) => { effects.push(effect); },
};
mock.module('react', () => ({ ...hookReact, default: hookReact }));
const invoked: string[] = [];
let inventory: () => Promise<unknown>;
mock.module('@tauri-apps/api/core', () => ({ invoke: async (name: string) => {
  invoked.push(name); return inventory();
} }));
const { DeviceSelection } = await import('../../src/components/DeviceSelection');
const devices = [{ name: 'virtual_mic', device_type: 'Input' },
  { name: 'pulse-monitor:virtual_speaker.monitor', device_type: 'Output' }];
function reset() {
  states = []; refs = []; invoked.length = 0;
  inventory = async () => devices;
}
function render(props: Parameters<typeof DeviceSelection>[0]) {
  stateIndex = 0; refIndex = 0; effects = [];
  return DeviceSelection(props);
}
function elements(node: unknown): React.ReactElement<any>[] {
  if (Array.isArray(node)) return node.flatMap(elements);
  if (!React.isValidElement(node)) return [];
  return [node, ...elements((node.props as any).children)];
}
const tick = () => new Promise(resolve => setTimeout(resolve, 0));
const choice = { micDevice: 'virtual_mic (input)', systemDevice: 'pulse-monitor:virtual_speaker.monitor (output)' };

test('shows both English pickers and enumerates without starting any capture', async () => {
  reset(); const changed = mock(() => {}), available = mock(() => {});
  const props = { selectedDevices: choice, onDeviceChange: changed, compact: true, onAvailabilityChange: available };
  render(props); effects.forEach(e => e()); await tick();
  const tree = render(props); effects[1]();
  const selects = elements(tree).filter(e => e.type === 'select');
  expect(selects.map(e => e.props['aria-label'])).toEqual(['Microphone', 'PC audio']);
  expect(selects.map(e => e.props.value)).toEqual([choice.micDevice, choice.systemDevice]);
  expect(invoked).toEqual(['get_audio_devices']);
  expect(changed).not.toHaveBeenCalled();
  expect(available).toHaveBeenLastCalledWith(true, choice);
  selects[0].props.onChange({ target: { value: '__default__' } });
  expect(changed).toHaveBeenCalledWith({ ...choice, micDevice: null });
});

test('missing saved device remains visible, blocks Start, and never auto-selects another', async () => {
  reset(); const changed = mock(() => {}), available = mock(() => {});
  const selected = { ...choice, micDevice: 'missing (input)' };
  const props = { selectedDevices: selected, onDeviceChange: changed, compact: true, onAvailabilityChange: available };
  render(props); effects.forEach(e => e()); await tick();
  const tree = render(props); effects[1]();
  const mic = elements(tree).find(e => e.type === 'select' && e.props['aria-label'] === 'Microphone')!;
  expect(mic.props.value).toBe('missing (input)');
  expect(elements(tree).some(e => e.type === 'option' && e.props.value === 'missing (input)' && e.props.disabled)).toBe(true);
  expect(changed).not.toHaveBeenCalled();
  expect(available).toHaveBeenLastCalledWith(false, selected);
});

test('disabled pickers and refresh cannot change a live recording selection', async () => {
  reset(); const changed = mock(() => {});
  const props = { selectedDevices: choice, onDeviceChange: changed, compact: true, disabled: true };
  render(props); effects.forEach(e => e()); await tick();
  const tree = render(props);
  for (const select of elements(tree).filter(e => e.type === 'select')) {
    expect(select.props.disabled).toBe(true);
    select.props.onChange({ target: { value: '__default__' } });
  }
  expect(changed).not.toHaveBeenCalled();
  expect(elements(tree).find(e => e.type === 'button')!.props.disabled).toBe(true);
});

test('speaker-only inventory permits Start and accepts raw saved device names', async () => {
  reset(); inventory = async () => [devices[1]]; const available = mock(() => {});
  const props = { selectedDevices: { micDevice: null, systemDevice: devices[1].name },
    onDeviceChange: () => {}, onAvailabilityChange: available, compact: true };
  render(props); effects.forEach(e => e()); await tick();
  const tree = render(props); effects[1]();
  expect(available).toHaveBeenLastCalledWith(true, props.selectedDevices);
  expect(elements(tree).filter(e => e.type === 'select')[1].props.value).toBe(choice.systemDevice);
});

test('native Pulse microphone has an explicit label but retains its exact capture identity', async () => {
  reset(); inventory = async () => [...devices, { name: 'pulse-input:fixture_native_mic', device_type: 'Input' }];
  const changed = mock(() => {});
  const props = { selectedDevices: choice, onDeviceChange: changed, compact: true };
  render(props); effects.forEach(e => e()); await tick();
  const tree = render(props);
  const option = elements(tree).find(e => e.type === 'option' && e.props.value === 'pulse-input:fixture_native_mic (input)')!;
  expect(option.props.children).toBe('fixture_native_mic');
  expect(changed).not.toHaveBeenCalled();
  const mic = elements(tree).find(e => e.type === 'select' && e.props['aria-label'] === 'Microphone')!;
  mic.props.onChange({ target: { value: option.props.value } });
  expect(changed).toHaveBeenCalledWith({ ...choice, micDevice: 'pulse-input:fixture_native_mic (input)' });
});

test('inventory error blocks Start; refresh recovers without changing selected devices', async () => {
  reset(); inventory = async () => { throw new Error('synthetic enumeration error'); };
  const available = mock(() => {}), changed = mock(() => {});
  const props = { selectedDevices: choice, onDeviceChange: changed, compact: true, onAvailabilityChange: available };
  render(props); effects.forEach(e => e()); await tick();
  let tree = render(props); effects[1]();
  expect(available).toHaveBeenLastCalledWith(false, choice);
  expect(elements(tree).filter(e => e.type === 'select').every(e => e.props.disabled)).toBe(true);
  inventory = async () => devices;
  elements(tree).find(e => e.type === 'button')!.props.onClick(); await tick();
  tree = render(props); effects[1]();
  expect(available).toHaveBeenLastCalledWith(true, choice);
  expect(changed).not.toHaveBeenCalled();
});
