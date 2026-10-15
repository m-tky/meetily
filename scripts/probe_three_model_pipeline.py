#!/usr/bin/env python3
"""PUBLIC60s pipelined ASR->alignment alongside cpp diarization; bounded queue.
Three independent owned processes; ASR text never fed back or corrected.
"""
import argparse
import json
import os
from pathlib import Path
import queue
import subprocess
import sys
import threading
import time

from probe_cpp_qwen_concurrency import REPO, proc_rss
from probe_diarization_engines import read_wav,sha256,write_float_wav
sys.path.insert(0,str(REPO/'benchmarks/qwen3-asr'))
from worker_client import Worker


def proc_cpu(pid):
    try:
        fields=Path(f'/proc/{pid}/stat').read_text().rsplit(')',1)[1].split()
        return (int(fields[11])+int(fields[12]))/os.sysconf('SC_CLK_TCK')
    except (FileNotFoundError,ProcessLookupError):return 0


def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--runtime',type=Path,required=True)
    ap.add_argument('--output-dir',type=Path,required=True);ap.add_argument('--aligner-threads',type=int,choices=[0,2,6],default=6)
    ap.add_argument('--official-alignment-worker',type=Path)
    ap.add_argument('--offline-backlog',action='store_true',help='All ASR windows ready at epoch; stress, not live input')
    a=ap.parse_args();root=a.runtime.resolve();out=a.output_dir.resolve()
    if a.official_alignment_worker and not a.aligner_threads:ap.error('Official worker requires alignment enabled')
    if out==REPO or REPO in out.parents:ap.error('External runtime only')
    out.mkdir(parents=True,exist_ok=False)
    q=root/'qwen3-asr';diar=root/'diarization-engines'
    binary=q/'align-worker-target/release/qwen-speech-worker';model=q/'artifacts/model-1.7B';aligner=q/'artifacts/model-forced-aligner-0.6B'
    wav=diar/'repeat.wav';meta=json.loads((diar/'repeat.reference.json').read_text())
    if sha256(wav)!=meta['sha256']:raise ValueError('Pinned fixture changed')
    audio=read_wav(wav)
    if len(audio)!=960000:raise ValueError('Frozen60s input required')
    for directory in (model,aligner):
        provenance=json.loads((directory/'provenance.json').read_text())
        for item in provenance['files']:
            if sha256(directory/item['file'])!=item['sha256']:raise ValueError('Model pin changed')
    plan=dict(seconds=60,window_seconds=10,asr_threads=6,cpp_threads=4,aligner_threads=a.aligner_threads,
        alignment_queue_capacity=2,warmup_excluded=True,three_independent_processes=True,
        asr_input_mode='offline_backlog_stress' if a.offline_backlog else 'real_time_10s_windows',cpp_input_paced=True,
        alignment_backend='official_nagisa' if a.official_alignment_worker else 'rust_character',
        official_worker_sha256=sha256(a.official_alignment_worker) if a.official_alignment_worker else None,
        official_adapter_sha256=sha256(REPO/'benchmarks/qwen3-asr/official_alignment_worker.py') if a.official_alignment_worker else None,
        fixture_sha256=sha256(wav),worker_sha256=sha256(binary),
        aligner_provenance_sha256=sha256(aligner/'provenance.json'),
        criteria=dict(all_processing_rtf_below=1,asr_start_wait_below=1,alignment_start_wait_below=1,
                      final_cpp_push_lag_below=.12,stop_drain_below=15),
        peak_owned_rss_limit_mib=14000,no_app_integration=True,no_capture_or_cloud=True)
    (out/'plan.json').write_text(json.dumps(plan,indent=2)+'\n')
    paths=[]
    for i in range(6):
        p=out/f'window-{i}.wav';write_float_wav(p,audio[i*160000:(i+1)*160000]);paths.append(p)
    asr=None;align=None;cpp=None;log=None;sampler=None;align_thread=None;stop=threading.Event()
    jobs=queue.Queue(maxsize=2);memory=[];asr_events=[];align_events=[];errors=[];origin=0;queue_max=0
    def sample():
        while not stop.is_set():
            pids={'supervisor':os.getpid()}
            if asr and asr.process:pids['asr']=asr.process.pid
            if align and align.process:pids['aligner']=align.process.pid
            if cpp:pids['cpp']=cpp.pid
            rss={k:proc_rss(v) for k,v in pids.items()}
            memory.append(dict(monotonic=time.monotonic(),rss_mib=rss,cpu_seconds={k:proc_cpu(v) for k,v in pids.items()}))
            if sum(rss.values())>14000:
                errors.append('owned_RSS_exceeded_frozen14GiB_guard');stop.set()
                if asr:asr.cancel()
                if align:align.cancel()
                if cpp and cpp.poll() is None:cpp.kill()
                return
            stop.wait(.1)
    def align_loop():
        try:
            with (out/'alignment-events.jsonl').open('w') as f:
                while not stop.is_set():
                    try:item=jobs.get(timeout=.1)
                    except queue.Empty:continue
                    if item is None:return
                    i,text,ready=item;started=time.monotonic()
                    response=align.request(dict(op='align',id=f'align-{i}',wav=str(paths[i]),text=text,language='Japanese'))
                    end=time.monotonic()
                    event=dict(index=i,text=text,started_seconds=started-origin,completed_seconds=end-origin,
                               queue_wait_seconds=started-ready,request_seconds=end-started,response=response)
                    align_events.append(event);f.write(json.dumps(event,ensure_ascii=False)+'\n');f.flush()
                    if response.get('status')!='aligned' or response.get('text')!=text or response.get('confirmed') is not False:
                        raise RuntimeError('Alignment protocol/failure; raw preserved')
        except BaseException as error:errors.append(type(error).__name__+': '+str(error))
    try:
        asr=Worker(binary,model,threads=6,timeout=180);asr.start()
        tick=time.monotonic();warm=asr.request(dict(op='transcribe',id='warm',wav=str(paths[0]),language='Japanese'))
        warm_meta=dict(asr_request_seconds=time.monotonic()-tick,asr_response=warm)
        if warm.get('asr_calls')!=1 or not warm.get('text'):raise RuntimeError('ASR warmup missing real inference')
        if a.aligner_threads:
            align=Worker(a.official_alignment_worker or binary,model,threads=a.aligner_threads,timeout=180,aligner_dir=aligner);ready=align.start()
            if not ready.get('alignment_enabled'):raise RuntimeError('Wrong worker capability')
            tick=time.monotonic();r=align.request(dict(op='align',id='warm-align',wav=str(paths[0]),text=warm['text'],language='Japanese'))
            warm_meta.update(align_request_seconds=time.monotonic()-tick,align_response=r)
            if r.get('status')!='aligned':raise RuntimeError('Aligner warmup failed')
        (out/'warmup.json').write_text(json.dumps(warm_meta,ensure_ascii=False,indent=2)+'\n')
        origin=time.monotonic()+2;log=(out/'cpp.log').open('w')
        env=os.environ.copy();env.update(OMP_NUM_THREADS='4',OPENBLAS_NUM_THREADS='4')
        cpp=subprocess.Popen([sys.executable,str(REPO/'scripts/probe_cpp_qwen_concurrency.py'),
            '--runtime',str(root),'--output-dir',str(out),'--mode','both','--fixture','repeat',
            '--cpp-child','--start-at',str(origin)],stdout=log,stderr=log,env=env)
        sampler=threading.Thread(target=sample,daemon=True);sampler.start()
        if align:align_thread=threading.Thread(target=align_loop,daemon=True);align_thread.start()
        with (out/'asr-events.jsonl').open('w') as f:
            for i,p in enumerate(paths):
                ready_seconds=0 if a.offline_backlog else (i+1)*10
                time.sleep(max(0,origin+ready_seconds-time.monotonic()))
                if errors:raise RuntimeError('Background pipeline failed')
                started=time.monotonic();r=asr.request(dict(op='transcribe',id=f'asr-{i}',wav=str(p),language='Japanese'));end=time.monotonic()
                event=dict(index=i,input_seconds=(i+1)*10,started_seconds=started-origin,completed_seconds=end-origin,
                    wall_input_ready_seconds=ready_seconds,
                    queue_wait_seconds=max(0,started-origin-ready_seconds),request_seconds=end-started,response=r)
                asr_events.append(event);f.write(json.dumps(event,ensure_ascii=False)+'\n');f.flush()
                if r.get('asr_calls')!=1 or not r.get('text') or r.get('confirmed') is not False:raise RuntimeError('ASR failed')
                if align:
                    jobs.put((i,r['text'],end),timeout=120);queue_max=max(queue_max,jobs.qsize())
                print(json.dumps(dict(request=i,asr_seconds=end-started,asr_start_wait=event['queue_wait_seconds'],pending_alignment=jobs.qsize())),flush=True)
        if align:
            jobs.put(None,timeout=120);align_thread.join(timeout=180)
            if align_thread.is_alive():raise RuntimeError('Alignment drain timeout')
        if cpp.wait(timeout=120)!=0:raise RuntimeError('CPP failed')
        if errors:raise RuntimeError('Background errors: '+repr(errors))
        stop.set();sampler.join(timeout=2)
        cr=json.loads((out/'cpp.json').read_text());skew=cr['ready']['start_monotonic']-origin
        if not 0<=skew<.02:raise RuntimeError('Shared epoch skew exceeded20ms')
        cpp_intervals=[(skew+e['wall_seconds']-e['push_seconds'],skew+e['wall_seconds']) for e in cr['events']]
        triple=sum(max(0,min(cb,ae['completed_seconds'],le['completed_seconds'])-max(ca,ae['started_seconds'],le['started_seconds']))
                   for ca,cb in cpp_intervals for ae in asr_events for le in align_events)
        paced=[s for s in memory if s['monotonic']>=origin]
        cpu={k:max((s['cpu_seconds'].get(k,0) for s in paced),default=0)-next((s['cpu_seconds'][k] for s in paced if s['cpu_seconds'].get(k,0)>0),0) for k in ('asr','aligner','cpp')}
        result=dict(plan=plan,cpp=cr['summary'],asr_request_rtf=sum(e['request_seconds'] for e in asr_events)/60,
            align_request_rtf=sum(e['request_seconds'] for e in align_events)/60 if align_events else None,
            max_asr_queue_wait=max(e['queue_wait_seconds'] for e in asr_events),
            max_alignment_queue_wait=max((e['queue_wait_seconds'] for e in align_events),default=0),
            alignment_queue_sampled_max=queue_max,asr_stop_drain=max(0,asr_events[-1]['completed_seconds']-60),
            asr_alignment_stop_drain=max(0,max([e['completed_seconds'] for e in asr_events+align_events])-60),
            cpp_stop_drain=max(0,skew+cr['summary']['wall_seconds']-60),
            final_stop_drain=max(0,max([e['completed_seconds'] for e in asr_events+align_events]+[skew+cr['summary']['wall_seconds']])-60),
            actual_triple_request_cpp_compute_overlap_seconds=triple,
            cpu_seconds_sampled=cpu,average_owned_cpu_core_equivalents=sum(cpu.values())/(max([e['completed_seconds'] for e in asr_events+align_events]+[skew+cr['summary']['wall_seconds']])),
            peak_sum_rss_mib=max(sum(s['rss_mib'].values()) for s in paced),
            sampled_peak_rss_mib={k:max((s['rss_mib'].get(k,0) for s in paced),default=0) for k in ('asr','aligner','cpp','supervisor')},
            memory_note='sampled resident sum, not PSS/leak proof',asr_calls=6,alignment_calls=len(align_events))
        result['throughput_criteria_passed']=(result['cpp']['compute_rtf']<1 and result['cpp']['final_completion_lateness_seconds']<.12
            and result['asr_request_rtf']<1 and (result['align_request_rtf'] is None or result['align_request_rtf']<1)
            and result['max_asr_queue_wait']<1 and result['max_alignment_queue_wait']<1 and result['final_stop_drain']<15)
        if a.offline_backlog:result['throughput_criteria_passed']=None  # Frozen live-input criteria do not apply.
        (out/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print(json.dumps(result,indent=2),flush=True)
    except BaseException as error:
        (out/'failure.json').write_text(json.dumps(dict(type=type(error).__name__,message=str(error),background_errors=errors))+'\n');raise
    finally:
        stop.set()
        if asr:asr.cancel()
        if align:align.cancel()
        if cpp and cpp.poll() is None:cpp.kill();cpp.wait(timeout=10)
        if align_thread:align_thread.join(timeout=10)
        if sampler:sampler.join(timeout=2)
        if log:log.close()
        (out/'memory.json').write_text(json.dumps(memory)+'\n')
        for p in paths:p.unlink(missing_ok=True)
        (out/'cleanup.json').write_text(json.dumps(dict(asr_reaped=not asr or asr.process is None,
            aligner_reaped=not align or align.process is None,cpp_reaped=not cpp or cpp.poll() is not None,
            alignment_thread_stopped=not align_thread or not align_thread.is_alive(),temporary_windows_removed=all(not p.exists() for p in paths)))+'\n')


if __name__=='__main__':main()
