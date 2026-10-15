import { invoke } from '@tauri-apps/api/core';

export interface RecordingCandidate {
  id: string;
  fingerprint: string;
  folder_name: string;
  title: string;
  created_at: string | null;
  original_status: string | null;
  transcript_count: number;
  audio_present: boolean;
  checkpoint_count: number;
  minutes_present: boolean;
  can_restore: boolean;
  warnings: string[];
}
export interface RecordingRecoveryScan {
  candidates: RecordingCandidate[];
  next_cursor: string | null;
  warnings: string[];
}
export interface RestoredRecording {
  meeting_id: string;
  already_registered: boolean;
  warnings: string[];
}
export function scanRecordingRecovery(cursor: string | null = null) {
  return invoke<RecordingRecoveryScan>('api_scan_recording_recovery', { cursor });
}
export function restoreRecording(candidate: RecordingCandidate) {
  if (!candidate.can_restore) return Promise.reject(new Error('No valid saved metadata/transcripts; original files are preserved.'));
  return invoke<RestoredRecording>('api_restore_recording', {
    candidateId: candidate.id,
    fingerprint: candidate.fingerprint,
  });
}
