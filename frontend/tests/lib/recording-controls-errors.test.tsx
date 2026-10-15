// Run this hook harness in its own Bun process: it replaces React hooks, not IPC.
import { expect, mock, test } from 'bun:test';
import * as React from 'react';
const effects: (() => void | (() => void))[] = [];
const hookReact = {
  ...React,
  useState: (initial: unknown) => [initial, () => {}],
  useEffect: (effect: () => void | (() => void)) => { effects.push(effect); },
  useCallback: (callback: unknown) => callback,
};
mock.module('react', () => ({ ...hookReact, default: hookReact }));
const handlers = new Map<string, (event: { payload: unknown }) => void>();
const unlistened: string[] = [];
mock.module('@tauri-apps/api/event', () => ({
  listen: async (name: string, handler: (event: { payload: unknown }) => void) => {
    handlers.set(name, handler);
    return () => { handlers.delete(name); unlistened.push(name); };
  },
}));
mock.module('@tauri-apps/api/core', () => ({ invoke: async () => false }));
mock.module('@/contexts/RecordingStateContext', () => ({
  useRecordingState: () => ({ isPaused: false, isStartingRecording: false }),
}));
mock.module('@/lib/analytics', () => ({ default: { trackTranscriptionError: () => {} } }));
const { RecordingControls } = await import('../../src/components/RecordingControls');
const tick = () => new Promise<void>(resolve => setTimeout(resolve, 0));

function mount() {
  effects.length = 0;
  const stop = mock(() => {});
  const error = mock(() => {});
  RecordingControls({ isRecording: true, barHeights: [], onRecordingStop: stop,
    onRecordingStart: () => {}, onTranscriptReceived: () => {}, onTranscriptionError: error,
    isRecordingDisabled: false, isParentProcessing: false });
  const cleanups = effects.map(effect => effect()).filter((value): value is () => void => typeof value === 'function');
  return { stop, error, cleanup: () => cleanups.forEach(fn => fn()) };
}

test('ASR errors notify without pretending native capture stopped', async () => {
  const mounted = mount();
  await tick();
  handlers.get('transcription-error')?.({ payload: { phase: 'active', actionable: false,
    error: 'worker exited', userMessage: 'Transcription failed: worker exited' } });
  handlers.get('transcript-error')?.({ payload: 'legacy ASR failure' });
  expect(mounted.stop).not.toHaveBeenCalled();
  expect(mounted.error).toHaveBeenCalledWith('legacy ASR failure');
  mounted.cleanup();
  expect(handlers.size).toBe(0);
});

test('cleanup before async registration resolves removes late listener', async () => {
  unlistened.length = 0;
  const mounted = mount();
  mounted.cleanup();
  await tick();
  expect(handlers.size).toBe(0);
  expect(unlistened).toEqual(['transcript-error']);
});
