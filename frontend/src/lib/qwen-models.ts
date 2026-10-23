export const DEFAULT_QWEN_MODEL = 'Qwen3-ASR-1.7B-INT8';
export const QWEN_MODELS = [
  { id: DEFAULT_QWEN_MODEL, label: 'Qwen3-ASR 1.7B' },
  { id: 'Qwen3-ASR-0.6B-INT8', label: 'Qwen3-ASR 0.6B' },
] as const;
export const QWEN_LANGUAGES = [
  { code: 'ja', name: 'Japanese' }, { code: 'en', name: 'English' }, { code: 'th', name: 'Thai' },
];
export const isQwenModel = (id: string) => QWEN_MODELS.some(m => m.id === id);
export const qwenLabel = (id: string) => QWEN_MODELS.find(m => m.id === id)?.label || id;
