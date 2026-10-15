import type { ModelConfig } from '@/services/configService';
import type { MinutesSettings } from './minutes-settings';

export function mayAutoGenerateSummary(provider: ModelConfig['provider']): boolean {
  return provider !== 'chatgpt_plan';
}

/** ChatGPT minutes and post-meeting summaries share one explicit selection. */
export function resolveSummaryModel(legacy: ModelConfig, minutes: MinutesSettings): ModelConfig {
  if (minutes.provider !== 'chatgpt_plan') return legacy;
  return { provider: 'chatgpt_plan', model: minutes.chatgptModel, whisperModel: legacy.whisperModel };
}
