#!/usr/bin/env python3
"""Read-only local dual-input save audit. Never emit speech, titles or audio.
Only the explicitly approved private dual-input profile is accepted.
Storage checks do not prove accuracy or independently prove both source energies.
"""
import argparse
import array
import json
import math
import pathlib
import sqlite3
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--profile', required=True)
args = parser.parse_args()
root = pathlib.Path(args.profile).resolve(strict=True)
assert (root/'.meetily-dual-preview').read_text() == 'real-dual-local-preview\n'
recordings = root/'home/Documents/meetily-recordings'
folders = sorted((p for p in recordings.iterdir() if p.is_dir() and not p.is_symlink()), key=lambda p:p.stat().st_mtime) if recordings.exists() else []
assert folders, 'No recording folders saved in the approved mic profile'
folder = folders[-1]
assert folder.resolve().is_relative_to(recordings.resolve())
metadata = json.loads((folder/'metadata.json').read_text())
segments = json.loads((folder/'transcripts.json').read_text())['segments']
audio = folder/'audio.mp4'
assert audio.is_file() and not audio.is_symlink()
with sqlite3.connect(f'file:{root}/data/com.meetily.ai/meeting_minutes.sqlite?mode=ro',uri=True) as db:
    meetings = db.execute('SELECT id FROM meetings WHERE folder_path=?',(str(folder),)).fetchall()
    rows = []
    if len(meetings)==1:
        rows = db.execute('SELECT transcript,audio_start_time,audio_end_time,duration FROM transcripts WHERE meeting_id=?',(meetings[0][0],)).fetchall()
expected = [(s['text'],s['audio_start_time'],s['audio_end_time'],s['duration']) for s in segments]
probe = subprocess.run(['ffprobe','-v','error','-select_streams','a:0','-show_entries','stream=sample_rate,channels:format=duration','-of','json',str(audio)],capture_output=True)
assert probe.returncode==0, 'Stored MP4 probe failed (details withheld)'
info = json.loads(probe.stdout)
decoded = subprocess.run(['ffmpeg','-v','warning','-i',str(audio),'-f','f32le','-ac','1','-ar','16000','-'],capture_output=True)
assert decoded.returncode==0, 'Stored MP4 decode failed (details withheld)'
samples=array.array('f');samples.frombytes(decoded.stdout)
import sys
if sys.byteorder!='little': samples.byteswap()
finite = all(math.isfinite(x) for x in samples)
peak = max((abs(x) for x in samples),default=0)
rms = math.sqrt(sum(float(x)*x for x in samples)/len(samples)) if samples and finite else None
devices = metadata.get('devices',{})
report = {
    'recording_folder_count':len(folders),
    'completion_committed':metadata.get('status')=='completed' and bool(metadata.get('completed_at')),
    'selected_microphone_persisted':devices.get('microphone') in ['meetily_mic','meetily_mic (input)','default','default (input)'],
    'selected_system_audio_persisted':devices.get('system_audio') in ['pulse-monitor:EchoCancel_Speaker.monitor','pulse-monitor:EchoCancel_Speaker.monitor (output)'],
    'audio_file_bytes':audio.stat().st_size,
    'audio_duration_seconds':float(info['format']['duration']),
    'decoded_duration_seconds':len(samples)/16000,
    'audio_channels':int(info['streams'][0]['channels']),
    'audio_sample_rate':int(info['streams'][0]['sample_rate']),
    'finite_audio':finite,'nonzero_audio':peak>.001,'peak_amplitude':peak,'rms_amplitude':rms,
    'decode_warnings_absent':not decoded.stderr,
    'saved_segments':len(segments),'all_segments_nonempty':bool(segments) and all(s['text'].strip() for s in segments),
    'one_sqlite_folder_binding':len(meetings)==1,
    # JS/Rust/SQLite JSON round trips may differ at the final binary64 bit.
    # Text remains exact; timestamp tolerance is one nanosecond, not ASR correction.
    'sqlite_matches_raw_text_and_audio_times':len(rows)==len(expected) and all(
        r[0]==e[0] and all(isinstance(x,(int,float)) and isinstance(y,(int,float))
            and math.isclose(x,y,rel_tol=0,abs_tol=1e-9) for x,y in zip(r[1:],e[1:]))
        for r,e in zip(sorted(rows,key=lambda x:(x[0],str(x[1:]))),
                       sorted(expected,key=lambda x:(x[0],str(x[1:]))))),
    'temporary_asr_inputs_removed':not list((root/'cache/com.meetily.ai/qwen-audio').glob('*')),
    'checkpoint_cleanup_complete':not (folder/'.checkpoints').exists(),
    'content_exported':False,
}
checks=['completion_committed','selected_microphone_persisted','selected_system_audio_persisted','finite_audio','nonzero_audio',
        'decode_warnings_absent','all_segments_nonempty','one_sqlite_folder_binding',
        'sqlite_matches_raw_text_and_audio_times','temporary_asr_inputs_removed','checkpoint_cleanup_complete']
report['passed']=all(report[k] for k in checks)
(root/'dual-save-audit.json').write_text(json.dumps(report,indent=2))
print(json.dumps(report,indent=2))
