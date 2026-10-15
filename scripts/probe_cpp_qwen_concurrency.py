#!/usr/bin/env python3
"""Bounded public-file CPU contention test; no app, device, network or DB access.
Run cpp/asr/both separately (fresh processes), using identical 60s PCM.
ASR requests are serial 10s full-input windows, released at audio EOF; no drops.
Cold model load is recorded but excluded from the warmed paced comparison.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
import time

from probe_diarization_engines import cpp_run, read_wav, sha256, summary, write_float_wav

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO / 'benchmarks/qwen3-asr'))
from worker_client import Worker


def schedule(seconds=60, window=10):
    if seconds != 60 or window != 10:
        raise ValueError('Frozen first-pass schedule is 60s / six 10s windows')
    return [(i*window, (i+1)*window) for i in range(seconds//window)]


def request_metrics(events, duration):
    if not events:
        raise ValueError('Missing ASR events')
    return {
        'requests': len(events),
        'total_request_seconds': sum(e['request_seconds'] for e in events),
        'request_rtf': sum(e['request_seconds'] for e in events)/duration,
        'maximum_request_seconds': max(e['request_seconds'] for e in events),
        'maximum_queue_wait_seconds': max(e['queue_wait_seconds'] for e in events),
        'final_queue_wait_seconds': events[-1]['queue_wait_seconds'],
        'stop_drain_seconds': max(0, events[-1]['completed_seconds']-duration),
        'asr_calls': sum(e['response'].get('asr_calls', 0) for e in events),
        'nonempty_responses': sum(bool(e['response'].get('text')) for e in events),
        'errors': sum(e['response'].get('status')=='error' for e in events),
    }


def proc_rss(pid):
    try:
        return int(Path(f'/proc/{pid}/statm').read_text().split()[1])*os.sysconf('SC_PAGE_SIZE')/1024**2
    except (FileNotFoundError, ProcessLookupError):
        return 0


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--runtime', type=Path, required=True)
    ap.add_argument('--output-dir', type=Path, required=True)
    ap.add_argument('--mode', choices=['cpp','asr','both'], required=True)
    ap.add_argument('--fixture', choices=['repeat','turns'], default='repeat')
    ap.add_argument('--keep-fixture-lanes', action='store_true',
                    help='Retain public synthetic mic/PC lanes; turns fixture only, not actual capture')
    ap.add_argument('--cpp-child', action='store_true', help=argparse.SUPPRESS)
    ap.add_argument('--start-at', type=float, default=0, help=argparse.SUPPRESS)
    args=ap.parse_args()
    root=args.runtime.resolve(); out=args.output_dir.resolve()
    if out==REPO or REPO in out.parents:
        ap.error('Raw outputs must stay outside the app checkout')
    diar=root/'diarization-engines'; qwen=root/'qwen3-asr'
    library=diar/'parakeet.cpp/build-znver3/libparakeet.so'
    model=diar/'models/cpp-q8_0.gguf'
    wav=diar/(args.fixture+'.wav')
    meta=json.loads((diar/(args.fixture+'.reference.json')).read_text())
    if args.keep_fixture_lanes and args.fixture!='turns':
        ap.error('Only public two-source turns has a synthetic lane assignment')
    if sha256(wav)!=meta['sha256']:
        raise ValueError('Pinned public fixture changed')
    audio=read_wav(wav)
    if len(audio)!=960000 or len(meta['reference'])!=len(audio):
        raise ValueError('Frozen 60s fixture required')
    if args.cpp_child:
        result=cpp_run(library,model,audio,'low',True,False,True,args.start_at)
        ready,events,finish,segments,cpu,rss=result
        stats=summary(events,ready,finish,segments,meta['reference'],audio,True,cpu,rss)
        stats['asr_concurrency_not_tested']=args.mode!='both'
        (out/'cpp.json').write_text(json.dumps(dict(summary=stats,ready=ready,events=events,
                                                   finish=finish,segments=segments),indent=2)+'\n')
        return
    out.mkdir(parents=True,exist_ok=False)
    windows=schedule()
    model_dir=qwen/'artifacts/model-1.7B'
    provenance=json.loads((model_dir/'provenance.json').read_text())
    for item in provenance['files']:
        if sha256(model_dir/item['file'])!=item['sha256']:
            raise ValueError('Pinned Qwen source model changed')
    model_hashes={p.name:sha256(p) for p in sorted(model_dir.iterdir()) if p.is_file()}
    lanes=None
    if args.keep_fixture_lanes:
        import array
        mic=array.array('f',(x if label==1 else 0 for x,label in zip(audio,meta['reference'])))
        pc=array.array('f',(x if label==2 else 0 for x,label in zip(audio,meta['reference'])))
        combined=array.array('f',(a+b for a,b in zip(mic,pc)))
        if combined.tobytes()!=audio.tobytes():raise ValueError('Synthetic lane reconstruction not bit exact')
        lanes={}
        for name,values in [('mic',mic),('pc',pc)]:
            p=out/('lane-'+name+'.wav');write_float_wav(p,values)
            if read_wav(p).tobytes()!=values.tobytes():raise ValueError('Retained lane WAV changed PCM')
            lanes[name]=dict(path=str(p),sha256=sha256(p),samples=len(values),sample_rate=16000,start_sample=0)
    plan=dict(qwen_model_hashes=model_hashes,synthetic_source_lanes=lanes,
              synthetic_lane_reconstruction_bit_exact=True if lanes else None,
              mode=args.mode,duration_seconds=60,window_seconds=10,cpp_threads=4,qwen_threads=6,
              cpp_profile='low',fixture=args.fixture,audio_sha256=sha256(wav),
              cpp_library_sha256=sha256(library),cpp_model_sha256=sha256(model),
              qwen_binary_sha256=sha256(qwen/'worker-target/release/qwen-speech-worker'),
              qwen_model_warmup_before_measurement=args.mode in ('asr','both'),
              cpp_model_load_before_epoch=True,cpp_inference_not_prewarmed=True,
              asr_serial_full_input=True,no_sample_drop=True,
              application_integration=False,real_capture=False,physical_playback=False,
              cloud=False,quality_acceptance=False,
              criteria=dict(cpp_rtf_below=1,cpp_final_push_lag_below=.12,
                            asr_rtf_below=1,asr_queue_wait_below=1,asr_stop_drain_below=10),
              limitations=['fixed repeated public clip, not natural meeting',
                           'fixed10s requests, not app VAD segmentation',
                           '60s only, excludes cold-start load from paced measurement'])
    (out/'plan.json').write_text(json.dumps(plan,indent=2)+'\n')
    paths=[]
    for i,(lo,hi) in enumerate(windows):
        p=out/f'window-{i}.wav';write_float_wav(p,audio[lo*16000:hi*16000]);paths.append(p)
    worker=None; child=None; log=None; stop=threading.Event(); samples=[]; sampler=None
    events=[]; ready=None; warm=None; failure=None
    def sample():
        while not stop.is_set():
            pids={'supervisor':os.getpid()}
            if worker and worker.process:pids['qwen']=worker.process.pid
            if child:pids['cpp']=child.pid
            samples.append(dict(monotonic=time.monotonic(),rss_mib={k:proc_rss(v) for k,v in pids.items()}))
            stop.wait(.1)
    try:
        if args.mode in ('asr','both'):
            worker=Worker(qwen/'worker-target/release/qwen-speech-worker',qwen/'artifacts/model-1.7B',threads=6,timeout=90)
            ready=worker.start()
            if ready.get('threads')!=6 or 'float32' not in ready.get('audio_formats',[]):
                raise ValueError('Current float32 worker required')
            t=time.monotonic()
            response=worker.request(dict(op='transcribe',id='warmup',wav=str(paths[0]),language='Japanese'))
            warm=dict(request_seconds=time.monotonic()-t,response=response)
            (out/'qwen-warmup.json').write_text(json.dumps(dict(ready=ready,warmup=warm),ensure_ascii=False,indent=2)+'\n')
            if response.get('asr_calls')!=1 or not response.get('text') or response.get('confirmed') is not False:
                raise RuntimeError('Speech/model warmup did not perform real unconfirmed ASR')
        origin=time.monotonic()+2
        if args.mode in ('cpp','both'):
            log=(out/'cpp.log').open('w')
            env=os.environ.copy();env.update(OMP_NUM_THREADS='4',OPENBLAS_NUM_THREADS='4')
            child=subprocess.Popen([sys.executable,str(Path(__file__).resolve()),'--runtime',str(root),
                '--output-dir',str(out),'--mode',args.mode,'--fixture',args.fixture,
                '--cpp-child','--start-at',str(origin)],
                stdout=log,stderr=log,env=env)
        sampler=threading.Thread(target=sample,daemon=True);sampler.start()
        if worker:
            with (out/'asr-events.jsonl').open('w') as raw:
                for i,((lo,hi),p) in enumerate(zip(windows,paths)):
                    time.sleep(max(0,origin+hi-time.monotonic()))
                    started=time.monotonic()
                    response=worker.request(dict(op='transcribe',id=f'paced-{i}',wav=str(p),language='Japanese'))
                    end=time.monotonic()
                    event=dict(index=i,audio_start=lo,audio_end=hi,input_samples=(hi-lo)*16000,
                               started_seconds=started-origin,completed_seconds=end-origin,
                               queue_wait_seconds=max(0,started-origin-hi),request_seconds=end-started,
                               response=response)
                    events.append(event);raw.write(json.dumps(event,ensure_ascii=False)+'\n');raw.flush()
                    print(json.dumps(dict(mode=args.mode,request=i,request_seconds=end-started,
                                          queue_wait_seconds=event['queue_wait_seconds'],status=response.get('status'))),flush=True)
                    if response.get('status')=='error' or response.get('asr_calls')!=1 or response.get('confirmed') is not False:
                        raise RuntimeError('ASR policy/error failure; raw response preserved')
        if child:
            if child.wait(timeout=90)!=0:raise RuntimeError('CPP child failed; see owned cpp.log')
        if {p.name:sha256(p) for p in sorted(model_dir.iterdir()) if p.is_file()}!=model_hashes:
            raise RuntimeError('Qwen model/cache changed during run')
        result=dict(plan=plan,asr=request_metrics(events,60) if events else None,
                    cpp=json.loads((out/'cpp.json').read_text())['summary'] if child else None)
        if child:
            cr=json.loads((out/'cpp.json').read_text())
            skew=cr['ready']['start_monotonic']-origin
            result['cpp_start_skew_seconds']=skew
            if not 0<=skew<.02:raise RuntimeError('CPP shared epoch skew exceeds frozen20ms bound')
            intervals=[(cr['ready']['start_monotonic']-origin+e['wall_seconds']-e['push_seconds'],
                        cr['ready']['start_monotonic']-origin+e['wall_seconds']) for e in cr['events']]
            result['asr_request_overlap_cpp_compute_seconds']=sum(max(0,min(b,e['completed_seconds'])-max(a,e['started_seconds']))
                for a,b in intervals for e in events)
        paced=[s for s in samples if s['monotonic']>=origin]
        result['sampled_peak_rss_mib']={k:max((s['rss_mib'].get(k,0) for s in paced),default=0) for k in ('supervisor','cpp','qwen')}
        result['sampled_peak_sum_rss_mib']=max((sum(s['rss_mib'].values()) for s in paced),default=0)
        result['rss_note']='100ms sampled resident sum, may double-count shared pages; not PSS or leak proof'
        cpp=result['cpp'];asr=result['asr']
        result['throughput_criteria_passed']=(not cpp or (cpp['compute_rtf']<1 and cpp['final_completion_lateness_seconds']<.12)) and (not asr or (asr['request_rtf']<1 and asr['maximum_queue_wait_seconds']<1 and asr['stop_drain_seconds']<10 and asr['errors']==0))
        if args.mode=='both' and result['asr_request_overlap_cpp_compute_seconds']<=0:
            raise RuntimeError('No actual compute overlap observed')
        (out/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
        print(json.dumps(result,ensure_ascii=False,indent=2),flush=True)
    except BaseException as e:
        failure=dict(type=type(e).__name__,message=str(e))
        (out/'failure.json').write_text(json.dumps(failure,indent=2)+'\n')
        raise
    finally:
        stop.set()
        if sampler:sampler.join(timeout=2)
        if child and child.poll() is None:child.kill();child.wait(timeout=10)
        if worker:worker.cancel()
        if log:log.close()
        (out/'memory.json').write_text(json.dumps(samples)+'\n')
        for p in paths:p.unlink(missing_ok=True)
        (out/'cleanup.json').write_text(json.dumps(dict(worker_reaped=not worker or worker.process is None,
            cpp_reaped=not child or child.poll() is not None,temporary_windows_removed=all(not p.exists() for p in paths),
            failure=failure))+'\n')


if __name__=='__main__':main()
