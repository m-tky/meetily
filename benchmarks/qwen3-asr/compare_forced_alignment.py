"""Compare identical raw ASR texts; retain unknowns, account for differing units.
Only source-placement proxies; never scores human char timing or natural DER.
"""
import argparse
import json
import math
from pathlib import Path
from evaluate_forced_alignment import assign,turn_metrics,groups


def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--directory',type=Path,required=True);ap.add_argument('--pass-id',type=int,default=0)
    a=ap.parse_args();d=a.directory.resolve();plan=json.loads((d/'plan.json').read_text());rows=[]
    for f in plan['fixtures']:
        native=json.loads((d/f'pass-{a.pass_id}'/(f['id']+'.json')).read_text());p=d/f'official-control-{a.pass_id}'/(f['id']+'-raw.json')
        if not p.exists():continue
        official=json.loads(p.read_text());units=official.get('units',[])
        for u in units:
            x,y=u['start_seconds'],u['end_seconds']
            flags=[]
            if not all(isinstance(t,(int,float)) and math.isfinite(t) for t in (x,y)):flags.append('nonfinite_time')
            elif not 0<=x<=y<=f['samples']/16000:flags.append('out_of_audio_bounds_or_inverted')
            elif x==y:flags.append('zero_duration')
            u['flags']=flags
        assigned=assign(units,native['diar']['segments'])
        multi_source=[]
        placements=f.get('placements',[])
        for i,u in enumerate(assigned):
            if u['speaker'] is None:continue
            for x in placements:
                for y in placements:
                    if x['speaker']!=y['speaker'] and max(x['start'],y['start'],u['start_seconds'])<min(x['end'],y['end'],u['end_seconds']):
                        multi_source.append(i);break
                if i in multi_source:break
        official_record=dict(response=dict(text=official['text']),assigned_units=assigned,diar=native['diar'],oracle_alignment=None)
        raw=official['text'];alignment_chars=''.join(c for u in units for c in u['text'] if c.isalnum())
        row=dict(fixture=f['id'],official_status=official['status'],official_units=len(units),
            raw_asr_text_unchanged=raw==native['response'].get('text'),
            normalized_alnum_text_covered=alignment_chars==''.join(c for c in raw if c.isalnum()),
            official_zero_duration_units=sum(u['start_seconds']==u['end_seconds'] for u in units),
            official_out_of_bounds_units=sum('out_of_audio_bounds_or_inverted' in u['flags'] for u in units),
            official_assigned_alnum_chars=sum(sum(c.isalnum() for c in u['text']) for u in assigned if u['speaker'] is not None),
            official_unknown_alnum_chars=sum(sum(c.isalnum() for c in u['text']) for u in assigned if u['speaker'] is None),
            native_turn_metrics=turn_metrics(f,native),official_turn_metrics=turn_metrics(f,official_record),
            assigned_units_intersecting_known_multi_source_clips=multi_source,
            overlap_audit_is_source_clip_proxy_not_human_voiced_truth=True,
            assigned_word_units=assigned,word_level_groups=groups(assigned),
            official_seconds=official['inference_seconds'],
            units_not_interchangeable=True,not_human_character_timing_or_DER=True)
        rows.append(row)
    path=d/f'comparison-{a.pass_id}.json';path.write_text(json.dumps(rows,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps([{k:v for k,v in r.items() if k not in ('assigned_word_units','word_level_groups')} for r in rows],ensure_ascii=False,indent=2))


if __name__=='__main__':main()
