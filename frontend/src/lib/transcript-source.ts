/** Capture origin, never person/speaker recognition. Legacy mixed audio is unknown. */
export function transcriptSourceLabel(source?: string | null): string | null {
  if (source === 'mic') return 'Mic';
  if (source === 'system') return 'PC';
  return null;
}

export function compareTranscriptTimes(
  a: { audio_start_time?: number; chunk_start_time?: number; sequence_id?: number },
  b: { audio_start_time?: number; chunk_start_time?: number; sequence_id?: number },
): number {
  return ((a.audio_start_time ?? a.chunk_start_time ?? 0) -
    (b.audio_start_time ?? b.chunk_start_time ?? 0)) ||
    ((a.sequence_id ?? 0) - (b.sequence_id ?? 0));
}

export function sameTranscriptEvent(
  a: { sequence_id?: number; text: string; timestamp: string; speaker?: string | null },
  b: { sequence_id?: number; text: string; timestamp: string; speaker?: string | null },
): boolean {
  if (a.sequence_id !== undefined && b.sequence_id !== undefined) {
    return a.sequence_id === b.sequence_id;
  }
  return a.text === b.text && a.timestamp === b.timestamp &&
    (a.speaker ?? null) === (b.speaker ?? null);
}
