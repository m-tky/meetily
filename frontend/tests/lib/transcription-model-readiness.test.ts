import { describe, expect, test } from 'bun:test';

import {
  getProviderCommands,
  hasDownloadingModel,
} from '../../src/lib/transcription-model-readiness';

describe('transcription model readiness', () => {
  test('retired providers have no executable command path', () => {
    expect(getProviderCommands('localWhisper')).toBeNull();
    expect(getProviderCommands('parakeet')).toBeNull();
  });

  test('uses only the Qwen command path', () => {
    expect(getProviderCommands('qwen')).toEqual({
      initialize: 'qwen_init',
      hasAvailableModels: 'qwen_has_available_models',
      getAvailableModels: 'qwen_get_available_models',
    });
  });

  test('rejects unsupported providers without fallback', () => {
    expect(getProviderCommands('deepgram')).toBeNull();
  });

  test('recognizes only active downloads', () => {
    expect(hasDownloadingModel([{ status: 'Available' }])).toBeFalse();
    expect(hasDownloadingModel([{ status: 'Downloading' }])).toBeTrue();
    expect(hasDownloadingModel([{ status: { Downloading: { progress: 0 } } }])).toBeTrue();
    expect(hasDownloadingModel([{ status: { Downloading: { progress: 42 } } }])).toBeTrue();
  });
});
