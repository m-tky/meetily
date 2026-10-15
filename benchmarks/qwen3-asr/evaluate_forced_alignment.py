"""Approved Japanese alignment/attribution failure characterization; PUBLIC/synthetic only.
No human character timestamp ground truth; source-placement checks are proxies.
"""
import argparse
import array
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time

REPO=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(REPO/'scripts'))
from probe_diarization_engines import cpp_run, read_wav, sha256, write_float_wav
from worker_client import Worker

RATE=16000


def prepare(root,out):
    out.mkdir(parents=True,exist_ok=False)
    q=root/'qwen3-asr';art=q/'artifacts'
    fixtures=[]
    sources=[]
    def source(path,text,speaker,license):
        values=read_wav(path)
        sources.append(dict(path=str(path),sha256=sha256(path),text=text,speaker=speaker,license=license))
        return values
    car=source(q/'diagnostics/current-vad-validation-r2/lexical-converted-0.wav','車',0,'CC0 / 葵心 / Lingua Libre')
    book=source(q/'diagnostics/current-vad-validation-r2/lexical-converted-1.wav','本',1,'CC BY 4.0 / Fukujinzuke / Lingua Libre')
    iie=source(q/'robustness/lexical-r1/iie-clean.wav','いいえ',2,'CC BY-SA 4.0 / Zsrtrgh / Lingua Libre')
    fmanifest=json.loads((art/'manifest.json').read_text())
    def add(name,values,reference,placements,**extra):
        values=array.array('f',values);p=out/(name+'.wav');write_float_wav(p,values)
        if read_wav(p).tobytes()!=values.tobytes():raise ValueError('Float WAV roundtrip changed PCM')
        fixtures.append(dict(id=name,path=str(p),sha256=sha256(p),samples=len(values),reference=reference,
                             placements=placements,human_character_timestamps=False,**extra))
    for idx in (0,1):
        row=[r for r in fmanifest['rows'] if r['language']=='Japanese' and r.get('purpose')=='accuracy'][idx]
        p=art/row['wav']
        if sha256(p)!=row['sha256']:raise ValueError('Pinned FLEURS source changed')
        a=source(p,row['reference'],0,'CC BY 4.0 / Conneau et al. FLEURS2022')
        add(f'sentence-{idx}',a,row['reference'],[dict(start=0,end=len(a)/RATE,speaker=0,text=row['reference'])],suite='sentence')
    for duration,reverse in ((10,False),(20,False),(30,False),(20,True)):
        audio=array.array('f',[0]*RATE);placements=[];words=[];turn=0
        # Complete source clips only, no last-word truncation. Four-word speaker runs.
        while True:
            who=((turn//4)%2) ^ int(reverse);clip=(car,book)[who];word=('車','本')[who]
            if len(audio)+len(clip)>duration*RATE:break
            lo=len(audio)/RATE;audio.extend(clip);hi=len(audio)/RATE
            placements.append(dict(start=lo,end=hi,speaker=who,text=word));words.append(word)
            n=min(RATE//2,duration*RATE-len(audio));audio.extend([0]*n);turn+=1
        audio.extend([0]*(duration*RATE-len(audio)))
        add(f'turns-{duration}'+('-reverse' if reverse else ''),audio,''.join(words),placements,suite='speaker_turns',unique_reference_order=True)
    for padding in (0,1):
        a=[0]*(padding*RATE)+list(iie)+[0]*(padding*RATE)
        add('iie-'+('padded' if padding else 'clean'),a,'いいえ',
            [dict(start=padding,end=padding+len(iie)/RATE,speaker=2,text='いいえ')],suite='short_negative_word')
    # Synthetic acknowledgements, explicitly not natural human conversation acceptance.
    for name,text in (('hai','はい'),('un','うん')):
        generated=out/(name+'-espeak-source.wav');converted=out/(name+'-espeak16.wav')
        subprocess.run(['espeak-ng','-v','jpx/ja','-s','150','-w',str(generated),text],check=True)
        subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-i',str(generated),'-ar','16000','-ac','1','-c:a','pcm_s16le',str(converted)],check=True)
        clip=read_wav(converted);a=[0]*RATE+list(clip)+[0]*RATE
        add('ack-'+name,a,text,[dict(start=1,end=1+len(clip)/RATE,speaker=3,text=text)],
            suite='synthetic_acknowledgement',natural_acknowledgement=False,tts='eSpeak-ng jpx/ja speed150')
    lanes=[[0.0]*(10*RATE) for _ in range(2)];placements=[]
    for start in (1,4,7):
        for who,clip,offset in ((0,car,0),(1,book,.15)):
            lo=round((start+offset)*RATE);n=min(len(clip),10*RATE-lo)
            for j in range(n):lanes[who][lo+j]+=.5*clip[j]
            placements.append(dict(start=lo/RATE,end=(lo+n)/RATE,speaker=who,text=('車','本')[who]))
    add('overlap-10',(a+b for a,b in zip(*lanes)),None,placements,suite='overlap',
        unique_reference_order=False,common_source_gain=.5)
    add('silence-5',[0]*(5*RATE),'',[],suite='negative')
    plan=dict(scope='4 requested alignment validations ONLY, no app rewrite',sources=sources,fixtures=fixtures,
              character_alignment='Rust Unicode scalar; official Japanese uses nagisa words',
              forced_aligner_revision='c7cbfc2048c462b0d63a45797104fc9db3ad62b7',
              asr_threads=6,cpp_threads=4,reference_text_not_given_to_asr=True,
              oracle_alignment_control_separate=True,natural_meeting_acceptance=False,
              criteria='Report raw errors, unit/text preservation, invalid/zero times, source-placement proxies; no timing quality acceptance without human timestamps',
              wrong_text_controls=['山','会議は中止です。'])
    (out/'plan.json').write_text(json.dumps(plan,ensure_ascii=False,indent=2)+'\n')


def assign(units,segments):
    # Conservative unique coverage; no arbitrary winner for overlapping speaker activity.
    result=[]
    for unit in units:
        a,b=unit.get('start_seconds'),unit.get('end_seconds');speaker=None;reason='unknown'
        if a is not None and b is not None and b>a and not unit.get('flags'):
            relevant=[s for s in segments if s['start']<b and s['end']>a]
            ids={s['speaker'] for s in relevant}
            if len(ids)==1:
                ranges=sorted((max(a,s['start']),min(b,s['end'])) for s in relevant)
                merged=[]
                for lo,hi in ranges:
                    if merged and lo<=merged[-1][1]:merged[-1]=(merged[-1][0],max(hi,merged[-1][1]))
                    else:merged.append((lo,hi))
                coverage=sum(hi-lo for lo,hi in merged)/(b-a)
                if coverage>=.8:speaker=next(iter(ids));reason='unique_coverage_proxy'
                else:reason='insufficient_coverage'
            elif len(ids)>1:reason='multiple_speakers'
        result.append(dict(unit,speaker=speaker,attribution_reason=reason,confirmed=False))
    return result


def characterize(response):
    alignment=response.get('alignment',response if response.get('status')=='aligned' else {})
    units=alignment.get('units',[])
    text=alignment.get('text','')
    return dict(status=response.get('status'),alignment_status=alignment.get('status'),
                chars=len(units),text_preserved=''.join(u['text'] for u in units)==''.join(c for c in text if not c.isspace()) if units else None,
                zero_duration_units=sum('zero_duration' in u.get('flags',[]) for u in units),
                invalid_time_units=sum(bool(set(u.get('flags',[])) & {'nonfinite_time','out_of_bounds_time','nonmonotonic_time'}) for u in units),
                punctuation_units=sum('non_spoken_symbol' in u.get('flags',[]) for u in units),
                asr_seconds=response.get('inference_seconds'),align_seconds=alignment.get('inference_seconds'))


def edit_distance(a,b):
    row=list(range(len(b)+1))
    for i,x in enumerate(a,1):
        nxt=[i]
        for j,y in enumerate(b,1):nxt.append(min(nxt[-1]+1,row[j]+1,row[j-1]+(x!=y)))
        row=nxt
    return row[-1]


def turn_metrics(fixture,record):
    if fixture['suite']!='speaker_turns':return None
    import itertools
    placements=fixture['placements'];segments=record['diar']['segments']
    clusters=sorted({s['speaker'] for s in segments});candidates=[]
    if len(clusters)>=2:candidates=[dict(zip(pair,(0,1))) for pair in itertools.permutations(clusters,2)]
    elif clusters:candidates=[{clusters[0]:who} for who in (0,1)]
    else:candidates=[{}]
    def score(mapping):
        return sum(max(0,min(s['end'],p['end'])-max(s['start'],p['start']))
            for s in segments for p in placements if mapping.get(s['speaker'])==p['speaker'])
    mapping=max(candidates,key=score)
    truth=[(c,p['speaker']) for p in placements for c in p['text'] if c.isalnum()]
    got=[(c,mapping.get(u['speaker'])) for u in record['assigned_units'] for c in u['text'] if c.isalnum()]
    if not record['assigned_units']:
        got=[(c,None) for c in record['response'].get('text','') if c.isalnum()]
    oracle=(record.get('oracle_alignment') or {}).get('units',[])
    checks=[]
    if len(oracle)==len(placements) and all(u['text']==p['text'] for u,p in zip(oracle,placements)):
        for u,p in zip(oracle,placements):
            a,b=u['start_seconds'],u['end_seconds']
            checks.append(bool(a is not None and b is not None and b>a and p['start']<=(a+b)/2<=p['end']))
    return dict(reference_chars=len(truth),recognized_chars=len(got),
        normalized_alnum_char_errors=edit_distance([x[0] for x in truth],[x[0] for x in got]),
        source_attributed_char_errors_proxy=edit_distance(truth,got),
        cluster_to_source_best_mapping=mapping,
        oracle_word_midpoints_inside_source_clip=sum(checks) if checks else None,
        oracle_word_midpoints_checked=len(checks),
        not_human_character_alignment_or_DER=True,source_clip_bounds_include_silence=True)


def run(root,out,pass_id):
    plan=json.loads((out/'plan.json').read_text());q=root/'qwen3-asr';diar=root/'diarization-engines'
    dest=out/f'pass-{pass_id}';dest.mkdir(exist_ok=False)
    binary=q/'align-worker-target/release/qwen-speech-worker';model=q/'artifacts/model-1.7B';aligner=q/'artifacts/model-forced-aligner-0.6B'
    for folder in (model,aligner):
        provenance=json.loads((folder/'provenance.json').read_text())
        for f in provenance['files']:
            if sha256(folder/f['file'])!=f['sha256']:raise ValueError('Pinned model changed')
    records=[]
    with Worker(binary,model,aligner_dir=aligner,timeout=180) as w:
        for i,f in enumerate(plan['fixtures']):
            p=Path(f['path'])
            if sha256(p)!=f['sha256']:raise ValueError('Fixture changed')
            tick=time.monotonic()
            response=w.request(dict(op='transcribe',id=f'asr-{i}',wav=str(p),language='Japanese',align=True))
            request_seconds=time.monotonic()-tick
            audio=read_wav(p)
            ready,events,finish,segments,cpu,rss=cpp_run(diar/'parakeet.cpp/build-znver3/libparakeet.so',diar/'models/cpp-q8_0.gguf',audio,'low',False,require_simd=True)
            oracle=None
            if f.get('reference'):
                oracle=w.request(dict(op='align',id=f'oracle-{i}',wav=str(p),text=f['reference'],language='Japanese'))
            assigned=assign(response.get('alignment',{}).get('units',[]),segments)
            record=dict(fixture=f['id'],request_seconds=request_seconds,response=response,oracle_alignment=oracle,
                        diar=dict(ready=ready,segments=segments,finish=finish,cpu_seconds=cpu,rss_mib=rss),
                        assigned_units=assigned,groups=groups(assigned))
            if ''.join(g['text'] for g in record['groups'])!=''.join(u['text'] for u in assigned):raise ValueError('Grouping changed text')
            (dest/(f['id']+'.json')).write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n')
            metrics=characterize(response);metrics.update(fixture=f['id'],request_seconds=request_seconds,
                assigned_chars=sum(u['speaker'] is not None for u in assigned),unknown_chars=sum(u['speaker'] is None for u in assigned),
                diar_clusters=sorted({s['speaker'] for s in segments}),oracle=characterize(oracle) if oracle else None)
            metrics['turn_metrics']=turn_metrics(f,record)
            records.append(metrics);print(json.dumps(metrics,ensure_ascii=False),flush=True)
        # Intentionally incorrect transcripts, never substituted into the ASR result.
        for i,text in enumerate(plan['wrong_text_controls']):
            f=next((f for f in plan['fixtures'] if f['id']=='turns-20'),plan['fixtures'][0])
            r=w.request(dict(op='align',id=f'wrong-{i}',wav=f['path'],text=text,language='Japanese'))
            (dest/f'wrong-{i}.json').write_text(json.dumps(r,ensure_ascii=False,indent=2)+'\n')
            records.append(dict(fixture=f'wrong-{i}',known_wrong_text=True,metrics=characterize(r)))
        r=w.request(dict(op='align',id='wrong-silence',wav=str(out/'silence-5.wav'),text='はい',language='Japanese'))
        (dest/'wrong-silence.json').write_text(json.dumps(r,ensure_ascii=False,indent=2)+'\n')
        records.append(dict(fixture='wrong-silence',known_wrong_text=True,metrics=characterize(r)))
    (dest/'summary.json').write_text(json.dumps(records,ensure_ascii=False,indent=2)+'\n')
    (dest/'provenance.json').write_text(json.dumps(dict(binary_sha256=sha256(binary),plan_sha256=sha256(out/'plan.json'),
        aligner_provenance_sha256=sha256(aligner/'provenance.json'),worker_reaped=True,
        records={p.name:sha256(p) for p in dest.iterdir() if p.suffix=='.json'}),indent=2)+'\n')


def groups(units):
    result=[]
    for u in units:
        who=u['speaker']
        if result and result[-1]['speaker']==who:result[-1]['text']+=u['text'];result[-1]['end_seconds']=u['end_seconds']
        else:result.append(dict(speaker=who,text=u['text'],start_seconds=u['start_seconds'],end_seconds=u['end_seconds'],confirmed=False))
    return result


def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--runtime',type=Path,required=True)
    ap.add_argument('--directory',type=Path,required=True);ap.add_argument('--prepare-only',action='store_true');ap.add_argument('--pass-id',type=int,default=0)
    a=ap.parse_args();root=a.runtime.resolve();out=a.directory.resolve()
    if out==REPO or REPO in out.parents:ap.error('Raw evidence must stay outside app checkout')
    if a.prepare_only:prepare(root,out)
    else:run(root,out,a.pass_id)


if __name__=='__main__':main()
