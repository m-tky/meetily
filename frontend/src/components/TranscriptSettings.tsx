import { QwenModelSettings } from './QwenModelSettings';
export interface TranscriptModelProps { provider: 'qwen'; model: string; apiKey?: string | null }
export interface TranscriptSettingsProps {
  transcriptModelConfig: TranscriptModelProps;
  setTranscriptModelConfig: (config: TranscriptModelProps) => void;
  onModelSelect?: () => void;
}
export function TranscriptSettings({ setTranscriptModelConfig, onModelSelect }: TranscriptSettingsProps) {
  return <QwenModelSettings onSelect={model => {
    setTranscriptModelConfig({ provider: 'qwen', model, apiKey: null });
    onModelSelect?.();
  }} />;
}
