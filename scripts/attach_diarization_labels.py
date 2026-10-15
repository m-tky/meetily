#!/usr/bin/env python3
"""Offline replay of public concurrency traces: tentative labels, never text rewriting.
Prototype contract only: source lanes retained, one mixed diar stream, unchanged
full-context ASR windows. This does NOT cut PCM on diar boundaries or align words.
"""
import argparse
import json
import math
from pathlib import Path

RATE=16000


def annotate(start_sample,end_sample,diar_segments,gate_intervals):
    if not 0<=start_sample<end_sample:
        raise ValueError('Invalid ASR sample range')
    gates=[]
    for g in gate_intervals:
        a,b=g['start_sample']+start_sample,g['end_sample']+start_sample
        if type(a) is not int or type(b) is not int or not start_sample<=a<b<=end_sample:
            raise ValueError('VAD bounds outside original ASR input')
        gates.append((a,b))
    # Merge overlapping VAD gates, without shrinking context supplied to ASR.
    merged=[]
    for a,b in sorted(gates):
        if merged and a<=merged[-1][1]:merged[-1]=(merged[-1][0],max(b,merged[-1][1]))
        else:merged.append((a,b))
    labels=[]
    for s in diar_segments:
        if (not all(math.isfinite(s[k]) for k in ('start','end')) or s['end']<s['start']
            or type(s['speaker']) is not int or not 0<=s['speaker']<8):
            raise ValueError('Invalid engine label')
        a,b=round(s['start']*RATE),round(s['end']*RATE)
        labels.append((a,b,s['speaker']))
    counts={};unknown=0;overlap=0;spans=[]
    for ga,gb in merged:
        cuts={ga,gb}
        for a,b,_ in labels:
            if a<gb and b>ga:cuts.update((max(ga,a),min(gb,b)))
        ordered=sorted(cuts)
        for a,b in zip(ordered,ordered[1:]):
            active=sorted({speaker for lo,hi,speaker in labels if lo<b and hi>a})
            n=b-a
            if not active:unknown+=n
            elif len(active)>1:overlap+=n
            for speaker in active:counts[speaker]=counts.get(speaker,0)+n
            if spans and spans[-1]['end_sample']==a and spans[-1]['speaker_ids']==active:
                spans[-1]['end_sample']=b
            else:spans.append(dict(start_sample=a,end_sample=b,speaker_ids=active))
    ids=sorted(counts)
    # Multiple voices anywhere in the window => no arbitrary winner/word split.
    single=ids[0] if len(ids)==1 and not overlap and not unknown else None
    return dict(start_sample=start_sample,end_sample=end_sample,speaker_ids=ids,
                single_speaker_id=single,voiced_samples=sum(b-a for a,b in merged),
                unknown_voiced_samples=unknown,overlap_voiced_samples=overlap,
                speaker_sample_coverage=counts,label_spans=spans,
                provisional=True,confirmed=False,text_alignment='whole_asr_window_only',
                diar_boundary_rounding='nearest16k sample; no word alignment')


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('run',type=Path)
    args=ap.parse_args();root=args.run.resolve()
    plan=json.loads((root/'plan.json').read_text())
    if not plan.get('synthetic_source_lanes') or not plan.get('synthetic_lane_reconstruction_bit_exact'):
        raise ValueError('Public retained-input-lane test required')
    from probe_diarization_engines import read_wav,sha256
    import array
    lanes=[]
    for name in ('mic','pc'):
        info=plan['synthetic_source_lanes'][name];p=Path(info['path'])
        if sha256(p)!=info['sha256']:raise ValueError('Retained input changed')
        data=read_wav(p)
        if len(data)!=960000:raise ValueError('Retained lane length changed')
        lanes.append(data)
    mixed=array.array('f',(a+b for a,b in zip(*lanes)))
    original=root.parent/(plan['fixture']+'.wav')
    if sha256(original)!=plan['audio_sha256'] or mixed.tobytes()!=read_wav(original).tobytes():
        raise ValueError('Retained source lanes do not reconstruct measured mixed PCM')
    diar=json.loads((root/'cpp.json').read_text())
    events=[json.loads(line) for line in (root/'asr-events.jsonl').read_text().splitlines()]
    labels=[]
    skew=json.loads((root/'result.json').read_text())['cpp_start_skew_seconds']
    for e in events:
        start,end=round(e['audio_start']*RATE),round(e['audio_end']*RATE)
        response=e['response']
        gates=response.get('gate_intervals',[])
        before=[p for p in diar['events'] if skew+p['wall_seconds']<=e['completed_seconds']]
        visible=before[-1]['visible_segments'] if before else []
        if skew+diar['finish']['wall_seconds']<=e['completed_seconds']:visible=diar['segments']
        at_delivery=annotate(start,end,visible,gates)
        final=annotate(start,end,diar['segments'],gates)
        labels.append(dict(request_index=e['index'],raw_text=response['text'],
                           recognition_context=response['recognition_context'],
                           input_samples=e['input_samples'],
                           delivery=at_delivery,final_replay=final,
                           attribution_changed=at_delivery!=final))
    out=root/'attribution.json'
    if out.exists():raise FileExistsError('Do not overwrite attribution evidence')
    out.write_text(json.dumps(dict(source_lanes_retained=True,mixed_samples=len(mixed),
        diarization_streams=1,asr_input_context_unchanged=True,
        offline_trace_replay_not_app_integration=True,labels=labels),ensure_ascii=False,indent=2)+'\n')
    print(json.dumps(dict(requests=len(labels),multi_speaker_windows=sum(len(x['final_replay']['speaker_ids'])>1 for x in labels),
        unknown_voiced_samples=sum(x['final_replay']['unknown_voiced_samples'] for x in labels),
        overlap_voiced_samples=sum(x['final_replay']['overlap_voiced_samples'] for x in labels),
        delivery_attribution_changed=sum(x['attribution_changed'] for x in labels),
        raw_text_preserved=True,source_lanes_retained=True,word_alignment=False)))


if __name__=='__main__':main()
