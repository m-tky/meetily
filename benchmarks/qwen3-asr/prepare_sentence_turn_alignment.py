"""Additional intact Japanese sentence turns, declared male/female FLEURS sources.
Keep the initial stress-plan untouched; no truncation or ASR reference hints.
"""
import argparse
import array
import json
from pathlib import Path
import subprocess
import urllib.request
from evaluate_forced_alignment import read_wav,sha256,write_float_wav,RATE


def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--runtime',type=Path,required=True)
    ap.add_argument('--directory',type=Path,required=True);ap.add_argument('--metadata',type=Path,required=True)
    a=ap.parse_args();root=a.runtime.resolve();out=a.directory.resolve();out.mkdir(parents=True,exist_ok=False)
    meta=json.loads(a.metadata.read_text());row=next(x for x in meta['rows'] if x['row_idx']==34)['row']
    if row['id']!=1612 or row['gender']!=1 or row['num_samples']!=130560:raise ValueError('Frozen female source metadata changed')
    url=row['audio'][0]['src']
    if not url.startswith('https://datasets-server.huggingface.co/cached-assets/google/fleurs/'):raise ValueError('Unexpected source host')
    source=out/'female-source.wav'
    with urllib.request.urlopen(url,timeout=60) as r:raw=r.read(10*1024**2+1)
    if len(raw)>10*1024**2:raise ValueError('Source exceeds bound')
    source.write_bytes(raw)
    female_path=out/'female16.wav'
    subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-i',str(source),'-ar','16000','-ac','1','-c:a','pcm_f32le',str(female_path)],check=True)
    female=read_wav(female_path)
    if len(female)!=row['num_samples']:raise ValueError('Declared source sample count mismatch')
    art=root/'qwen3-asr/artifacts';manifest=json.loads((art/'manifest.json').read_text())
    male_row=next(x for x in manifest['rows'] if x['id']=='fleurs-ja_jp-validation-0-1519')
    male_path=art/male_row['wav']
    if sha256(male_path)!=male_row['sha256']:raise ValueError('Existing male source changed')
    male=read_wav(male_path);fixtures=[]
    for name,duration,order in [('sentence-turns-22',22,[0,1]),('sentence-turns-22-reverse',22,[1,0]),('sentence-turns-30',30,[1,0,1])]:
        audio=array.array('f',[0]*(RATE//2));placements=[]
        for who in order:
            clip=(male,female)[who];text=(male_row['reference'],row['transcription'])[who]
            start=len(audio)/RATE;audio.extend(clip);end=len(audio)/RATE
            placements.append(dict(start=start,end=end,speaker=who,text=text));audio.extend([0]*(RATE//2))
        if len(audio)>duration*RATE:raise ValueError('Whole-source fixture exceeds bound')
        audio.extend([0]*(duration*RATE-len(audio)));p=out/(name+'.wav');write_float_wav(p,audio)
        fixtures.append(dict(id=name,path=str(p),sha256=sha256(p),samples=len(audio),reference=' '.join(p['text'] for p in placements),
            placements=placements,suite='speaker_turns',unique_reference_order=True,human_character_timestamps=False,
            natural_conversation=False,intact_sentence_reading=True))
    plan=dict(scope='Additional sentence-turn validation, initial failures retained',fixtures=fixtures,
        sources=[dict(path=str(male_path),sha256=sha256(male_path),dataset_id=1519,gender=0),
                 dict(path=str(female_path),sha256=sha256(female_path),source_sha256=sha256(source),dataset_id=1612,row_idx=34,gender=1)],
        license='CC BY4.0 / Conneau et al. FLEURS2022',metadata_sha256=sha256(a.metadata),
        gender_metadata_basis='Different declared genders; not independently verified speaker identity',
        human_character_timestamps=False,natural_meeting_acceptance=False,
        reference_text_not_given_to_asr=True,wrong_text_controls=['山','会議は中止です。'])
    # Reference runner expects these shared controls; silence is a synthetic file only.
    write_float_wav(out/'silence-5.wav',array.array('f',[0]*80000))
    (out/'plan.json').write_text(json.dumps(plan,ensure_ascii=False,indent=2)+'\n')


if __name__=='__main__':main()
