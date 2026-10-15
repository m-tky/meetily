import { test, expect } from 'bun:test';
import { resolveSummaryModel, mayAutoGenerateSummary } from '../../src/lib/summary-model';
import { defaultMinutesSettings } from '../../src/lib/minutes-settings';

const legacy = { provider: 'ollama' as const, model: 'legacy-model', whisperModel: 'unchanged-asr', apiKey: 'synthetic-key', customOpenAIEndpoint: 'http://unused.invalid' };
test('post-meeting summaries use the shared explicit Luna selection, not the legacy Ollama model', () => {
  expect(resolveSummaryModel(legacy, { ...defaultMinutesSettings, provider: 'chatgpt_plan' })).toEqual({ provider: 'chatgpt_plan', model: 'gpt-6-luna', whisperModel: 'unchanged-asr' });
});
test('model changes are shared and never replace an unavailable or blank selection with another provider', () => {
  for (const model of ['fixture-model', '']) expect(resolveSummaryModel(legacy, { ...defaultMinutesSettings, provider: 'chatgpt_plan', chatgptModel: model }).model).toBe(model);
});
test('local selection preserves existing legacy provider behavior', () => {
  expect(resolveSummaryModel(legacy, defaultMinutesSettings)).toBe(legacy);
});
test('choosing a ChatGPT model is not permission to automatically upload after Stop', () => {
  expect(mayAutoGenerateSummary('chatgpt_plan')).toBe(false);
  expect(mayAutoGenerateSummary('ollama')).toBe(true);
});
