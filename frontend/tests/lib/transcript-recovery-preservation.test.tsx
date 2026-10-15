import { beforeEach, expect, mock, test } from 'bun:test';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';

const commands: string[] = [];
const saves: unknown[] = [];
let folderPath: string | undefined;
mock.module('@tauri-apps/api/core', () => ({
  invoke: async (command: string) => {
    commands.push(command);
    if (command === 'recover_audio_from_checkpoints') return { status: 'failed', chunk_count: 1, estimated_duration_seconds: 0, message: 'synthetic failure' };
    throw new Error('Unexpected command');
  },
}));
mock.module('@/services/indexedDBService', () => ({
  indexedDBService: {
    getMeetingMetadata: async () => ({ meetingId: 'old', title: 'Synthetic', folderPath }),
    getTranscripts: async () => [{ id: 1, text: 'saved text', timestamp: '00:01', sequenceId: 1, confidence: -1 }],
    markMeetingSaved: async () => {},
    getAllMeetings: async () => [],
  },
}));
mock.module('@/services/storageService', () => ({
  storageService: { saveMeeting: async (_title: string, _transcripts: unknown, folder: unknown) => { saves.push(folder); return { meeting_id: 'indexed-old' }; } },
}));
mock.module('@/lib/summary-language-preferences', () => ({ applyPinnedSummaryLanguageToMeeting: async () => {} }));
const { useTranscriptRecovery } = await import('../../src/hooks/useTranscriptRecovery');
function api() {
  let captured!: ReturnType<typeof useTranscriptRecovery>;
  function Capture() { captured = useTranscriptRecovery(); return null; }
  renderToStaticMarkup(<Capture />);
  return captured;
}
beforeEach(() => { commands.length = 0; saves.length = 0; folderPath = undefined; });
test('historical recovery never guesses the current recording folder', async () => {
  await api().recoverMeeting('old');
  expect(commands).toEqual([]);
  expect(saves).toEqual([null]);
});
test('audio failure keeps folder binding and never deletes checkpoints', async () => {
  folderPath = '/synthetic/old-folder';
  const result = await api().recoverMeeting('old');
  expect(result.audioRecoveryStatus?.status).toBe('failed');
  expect(commands).toEqual(['recover_audio_from_checkpoints']);
  expect(saves).toEqual([folderPath]);
});
