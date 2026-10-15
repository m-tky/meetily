import { beforeEach, expect, mock, test } from 'bun:test';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';

const calls: { command: string; args: unknown }[] = [];
mock.module('@tauri-apps/api/core', () => ({
  invoke: async (command: string, args: unknown) => {
    calls.push({ command, args });
    return command === 'api_scan_recording_recovery'
      ? { candidates: [], next_cursor: null, warnings: [] }
      : { meeting_id: 'meeting-recovery-synthetic', already_registered: false, warnings: [] };
  },
}));
const { scanRecordingRecovery, restoreRecording } = await import('../../src/lib/recording-recovery');
const { RecordingRecoveryPanel } = await import('../../src/components/RecordingRecoveryPanel');
const candidate = {
  id: 'recovery-synthetic', fingerprint: 'reviewed-version', folder_name: 'synthetic folder',
  title: 'Synthetic meeting', created_at: null, original_status: 'recording',
  transcript_count: 1, audio_present: false, checkpoint_count: 1, minutes_present: true,
  can_restore: true, warnings: ['Saved speech may be incomplete'],
};
beforeEach(() => { calls.length = 0; });

test('discovery never auto-restores and carries pagination only', async () => {
  await scanRecordingRecovery('previous-folder');
  expect(calls).toEqual([{ command: 'api_scan_recording_recovery', args: { cursor: 'previous-folder' } }]);
});
test('explicit restore sends opaque id and reviewed fingerprint, not filesystem paths', async () => {
  await restoreRecording(candidate);
  expect(calls).toEqual([{ command: 'api_restore_recording', args: { candidateId: candidate.id, fingerprint: candidate.fingerprint } }]);
});
test('unreadable candidate cannot be submitted', async () => {
  await expect(restoreRecording({ ...candidate, can_restore: false })).rejects.toThrow('No valid saved metadata');
  expect(calls).toEqual([]);
});
test('busy SSR exposes no recovery mutation and disables the check control', () => {
  const html = renderToStaticMarkup(<RecordingRecoveryPanel busy onRecovered={async () => {}} />);
  expect(html).toContain('disabled');
  expect(html).toContain('Available after recording and saving finish');
  expect(calls).toEqual([]);
});
