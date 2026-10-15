#!/usr/bin/env python3
"""Prepare (never execute) local playback of the user's approved short mic test.
No audio/text contents are emitted, and no recording or playback starts here.
"""
import argparse
import json
import pathlib
import shlex
import shutil
import subprocess

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--mic-profile',required=True)
p.add_argument('--dual-profile',required=True)
a=p.parse_args()
mic=pathlib.Path(a.mic_profile).resolve(strict=True)
dual=pathlib.Path(a.dual_profile).resolve(strict=True)
assert (mic/'.meetily-mic-preview').read_text()=='real-mic-local-preview\n'
assert (dual/'.meetily-dual-preview').read_text()=='real-dual-local-preview\n'
root=mic/'home/Documents/meetily-recordings'
folders=[f for f in root.iterdir() if f.is_dir() and not f.is_symlink()]
assert len(folders)==1, 'Only the approved single short microphone test is supported'
audio=folders[0]/'audio.mp4'
assert audio.is_file() and not audio.is_symlink() and audio.resolve().is_relative_to(root.resolve())
info=json.loads(subprocess.check_output(['ffprobe','-v','error','-show_entries','format=duration','-of','json',str(audio)]))
assert 0<float(info['format']['duration'])<=30, 'Refusing long/private meeting playback'
state=json.loads((dual/'preview-state.json').read_text())
assert state['system_audio']=='EchoCancel_Speaker.monitor'
server=json.loads((dual/'input-routing.json').read_text())['host_server']
script=dual/'play-pc-test.sh'
script.write_text('#!'+shutil.which('bash')+'\nset -euo pipefail\n'
    +shlex.quote(shutil.which('ffmpeg'))+' -v error -i '+shlex.quote(str(audio))
    +' -f f32le -ac 2 -ar 48000 - | env PULSE_SERVER='+shlex.quote(server)+' '
    +shlex.quote(shutil.which('paplay'))+' --raw --format=float32le --channels=2 --rate=48000 --device=EchoCancel_Speaker\n')
script.chmod(0o700)
print('Local replay command prepared; no playback started: '+str(script))
