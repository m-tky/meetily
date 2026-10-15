import { transcriptSourceLabel } from '@/lib/transcript-source';

/** A capture origin badge; intentionally never renders a person's identity. */
export function TranscriptInputBadge({ source }: { source?: string | null }) {
  const label = transcriptSourceLabel(source);
  if (!label) return null;
  return (
    <span className="text-xs text-gray-500" title="Audio input source, not speaker identification">
      {label}
    </span>
  );
}
