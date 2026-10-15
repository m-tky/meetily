#!/usr/bin/env python3
"""Compare isolated parakeet.cpp/parakeet-rs CPU diarization, no audio device access.
Models and compiled binaries must be pre-provisioned outside the app checkout.
"""
import argparse
import array
import ctypes as C
import hashlib
import json
import math
import os
from pathlib import Path
import resource
import struct
import subprocess
import time

from probe_nvidia_diarization import make_fixture, proxy_score, read_wav


def sha256(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as f:
        while block := f.read(1024*1024):
            h.update(block)
    return h.hexdigest()


def write_float_wav(path, audio):
    pcm = array.array('f', audio)
    import sys
    if sys.byteorder != 'little':
        pcm.byteswap()
    data = pcm.tobytes()
    fmt = struct.pack('<HHIIHH', 3, 1, 16000, 64000, 4, 32)
    chunks = b'fmt ' + struct.pack('<I', len(fmt)) + fmt + b'data' + struct.pack('<I', len(data)) + data
    Path(path).write_bytes(b'RIFF'+struct.pack('<I',len(chunks)+4)+b'WAVE'+chunks)


def rasterize(segments, seconds, dt=.01):
    # Native segments may overlap. Retain a set of speakers per frame rather
    # than silently resolving an overlap to one voice.
    rows = [set() for _ in range(round(seconds/dt))]
    for s in segments:
        if (not all(math.isfinite(s[k]) for k in ('start', 'end')) or s['end'] < s['start']
                or type(s['speaker']) is not int or not 0<=s['speaker']<8):
            raise ValueError('Invalid engine segment')
        lo = max(0, math.ceil(s['start']/dt-0.5-1e-6))
        hi = min(len(rows), math.ceil(s['end']/dt-0.5-1e-6))
        for i in range(lo, hi):
            rows[i].add(s['speaker']+1)
    return rows


def summary(events, ready, finish, segments, reference, audio, paced, cpu, rss):
    duration = len(audio)/16000
    rows = rasterize(segments, duration)
    # proxy counts simultaneous/missing prediction as wrong on this no-overlap
    # source fixture; no arbitrary speaker winner.
    predicted = [next(iter(r)) if len(r)==1 else 0 for r in rows]
    first = [None]*len(rows)
    revisions = 0
    previous = [set() for _ in rows]
    for e in events:
        current = rasterize(e['visible_segments'], duration)
        for i, (old, new) in enumerate(zip(previous,current)):
            if new and first[i] is None:
                first[i] = e['wall_seconds']
            if old and old!=new:
                revisions += 1
        previous = current
    # Output-age for first visible speaker label, restricted to frames retaining
    # a final speaker. Not time to confirmed identity; ignores undetected speech.
    ages = sorted(first[i]-i*.01 for i,r in enumerate(rows) if r and first[i] is not None)
    compute = sum(e['push_seconds'] for e in events)+finish['flush_seconds']
    lateness = [max(0,e['wall_seconds']-e['input_seconds']) for e in events]
    return {'seconds':duration,'paced':paced,'cpu_threads':4,'load_seconds':ready['load_seconds'],
            'buffer_latency_seconds':ready['buffer_latency'],
            'compiled_cpu_features':ready.get('compiled_cpu_features'),'compute_seconds':compute,
            'compute_rtf':compute/duration,'wall_seconds':finish['wall_seconds'],
            'flush_seconds':finish['flush_seconds'],'cpu_seconds':cpu,'max_rss_mib':rss,
            'maximum_push_seconds':max(e['push_seconds'] for e in events),
            'maximum_completion_lateness_seconds':max(lateness) if paced else None,
            'final_completion_lateness_seconds':lateness[-1] if paced else None,
            'completion_lateness_15_30_45_60':{str(t):next((max(0,e['wall_seconds']-e['input_seconds']) for e in events if e['input_seconds']>=t),None) for t in (15,30,45,60)} if paced else None,
            'first_speaker_wall_seconds':min((t for t in first if t is not None),default=None),
            'voiced_frame_first_label_age_median_seconds':ages[len(ages)//2] if paced and ages else None,
            'voiced_frame_first_label_age_max_seconds':max(ages) if paced and ages else None,
            'clusters':sorted({s['speaker'] for s in segments}),
            'segment_count':len(segments),'overlap_frames':sum(len(r)>1 for r in rows),
            'revised_nonempty_frame_events':revisions,
            'proxy_score':proxy_score(audio,reference,predicted,.01,8),
            'not_standard_der':True,'asr_concurrency_not_tested':True}


class CppSegment(C.Structure):
    _fields_=[('speaker',C.c_int),('start',C.c_float),('end',C.c_float)]


def require_cpu_simd(features):
    missing=[name for name in ('avx2','fma','f16c') if features.get(name)!=1]
    if missing:
        raise RuntimeError('Required CPU SIMD not compiled: '+', '.join(missing))


def cpp_run(library, model, audio, profile, paced, trace=False, require_simd=False, start_at=None):
    # Must be set before first native load; all accelerator backends were also
    # disabled at build time. No global OS/environment modification.
    os.environ['PARAKEET_DEVICE']='cpu'
    lib=C.CDLL(str(Path(library).resolve()))
    ptr=C.c_void_p
    def bind(name, ret, types):
        f=getattr(lib,'parakeet_capi_'+name);f.restype=ret;f.argtypes=types;return f
    # Upstream exposes a C++ thread setter but no standalone diar C-ABI setter.
    # Linux-only benchmark shim via the exported Itanium C++ symbol, no upstream
    # source patch. Fail rather than silently use a different number of threads.
    set_threads=getattr(lib,'_ZN2pk15set_num_threadsEi')
    set_threads.argtypes=[C.c_int];set_threads.restype=None;set_threads(4)
    features={}
    for name in ('sse3','avx','avx2','avx512','fma','f16c','llamafile'):
        feature=getattr(lib,'ggml_cpu_has_'+name)
        feature.restype=C.c_int;feature.argtypes=[]
        features[name]=feature()
    if require_simd:require_cpu_simd(features)
    load=bind('load',ptr,[C.c_char_p]); load_error=bind('load_error',C.c_char_p,[])
    error=bind('last_error',C.c_char_p,[ptr]);free=bind('free',None,[ptr])
    begin=bind('diarize_stream_begin_latency',ptr,[ptr,C.c_int])
    feed=bind('diarize_stream_feed',C.c_int,[ptr,C.POINTER(C.c_float),C.c_int,C.c_int,C.POINTER(C.POINTER(CppSegment)),C.POINTER(C.c_int)])
    active=bind('diarize_stream_active',C.c_int,[ptr,C.POINTER(C.POINTER(CppSegment)),C.POINTER(C.c_int)])
    free_segs=bind('free_diar_segments',None,[C.POINTER(CppSegment)])
    free_stream=bind('diarize_stream_free',None,[ptr])
    processed=bind('diarize_stream_time',C.c_float,[ptr])
    chunk_samples=bind('diarize_stream_chunk_samples',C.c_int,[ptr])
    t=time.monotonic();ctx=load(str(Path(model).resolve()).encode())
    if not ctx: raise RuntimeError(load_error().decode())
    ready={'load_seconds':time.monotonic()-t,'compiled_cpu_features':features}
    stream=None
    def take(call,*args):
        out=C.POINTER(CppSegment)();n=C.c_int()
        status=call(*args,C.byref(out),C.byref(n))
        try:
            if status: raise RuntimeError(error(ctx).decode())
            return [{'speaker':out[i].speaker,'start':float(out[i].start),'end':float(out[i].end)} for i in range(n.value)]
        finally:
            if out: free_segs(out)
    try:
        stream=begin(ctx,{'low':1,'very-low':2,'ultra':3}[profile])
        if not stream: raise RuntimeError(error(ctx).decode())
        ready['buffer_latency']=chunk_samples(stream)/16000
        if trace: print(json.dumps({'phase':'ready',**ready}),flush=True)
        buffer=(C.c_float*len(audio)).from_buffer(audio)
        if start_at is not None:
            if not paced or time.monotonic() >= start_at:
                raise RuntimeError('Paced synchronized start missed; do not benchmark misaligned runs')
            time.sleep(max(0, start_at-time.monotonic()))
        events=[];segments=[];start=time.monotonic();cpu_start=time.process_time()
        ready['start_monotonic']=start
        for offset in range(0,len(audio),1920):
            n=min(1920,len(audio)-offset); supplied=(offset+n)/16000
            if paced: time.sleep(max(0,start+supplied-time.monotonic()))
            t=time.monotonic()
            closed=take(feed,stream,C.cast(C.byref(buffer,offset*4),C.POINTER(C.c_float)),n,0)
            push_seconds=time.monotonic()-t
            segments.extend(closed)
            visible=segments+take(active,stream)
            if trace and (closed or offset % (1920*9) == 0):
                print(json.dumps({'phase':'progress','input_seconds':supplied,'push_seconds':push_seconds,
                                  'wall_seconds':time.monotonic()-start,'processed_seconds':float(processed(stream))}),flush=True)
            events.append({'phase':'push','input_seconds':supplied,'push_seconds':push_seconds,
                           'processed_seconds':float(processed(stream)),
                           'wall_seconds':time.monotonic()-start,'segments':closed,
                           'visible_segments':visible})
        t=time.monotonic();segments.extend(take(feed,stream,None,0,1))
        finish={'phase':'finish','flush_seconds':time.monotonic()-t,'wall_seconds':time.monotonic()-start,
                'processed_seconds':float(processed(stream))}
        cpu=time.process_time()-cpu_start
        rss=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss/1024
        return ready,events,finish,segments,cpu,rss
    finally:
        if stream:free_stream(stream)
        free(ctx)


def rs_run(binary,model,wav,profile,paced):
    completed=subprocess.run([str(Path(binary).resolve()),str(Path(model).resolve()),str(Path(wav).resolve()),profile,'paced' if paced else 'fast'],capture_output=True,text=True,timeout=600)
    if completed.returncode:
        raise RuntimeError(f'Rust exit{completed.returncode}: {completed.stderr[-8000:]}')
    lines=[json.loads(line) for line in completed.stdout.splitlines()]
    ready=next(e for e in lines if e['phase']=='ready')
    finish=next(e for e in lines if e['phase']=='finish')
    events=[e for e in lines if e['phase']=='push']
    visible=[]
    for e in events:
        visible.extend(e['segments'])
        e['visible_segments']=visible.copy()
    segments=visible+finish['segments']
    usage=resource.getrusage(resource.RUSAGE_CHILDREN)
    return ready,events,finish,segments,usage.ru_utime+usage.ru_stime,usage.ru_maxrss/1024


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--prepare',action='store_true')
    ap.add_argument('--directory',type=Path,required=True)
    ap.add_argument('--engine',choices=['cpp','rs'])
    ap.add_argument('--model',type=Path)
    ap.add_argument('--library',type=Path)
    ap.add_argument('--binary',type=Path)
    ap.add_argument('--fixture',choices=['turns','reverse','repeat','silence','tone'],default='turns')
    ap.add_argument('--profile',choices=['low','very-low','ultra'],default='low')
    ap.add_argument('--pace',action='store_true')
    ap.add_argument('--output',type=Path)
    ap.add_argument('--limit-seconds',type=float,default=0,help='CPP diagnostic prefix only; does not alter fixture')
    ap.add_argument('--trace',action='store_true')
    ap.add_argument('--require-simd',action='store_true',help='CPP fail-fast AVX2/FMA/F16C build guard')
    args=ap.parse_args()
    if args.prepare:
        base=args.directory.parent/'qwen3-asr'
        sources=[base/'diagnostics/current-vad-validation-r2'/f'lexical-converted-{i}.wav' for i in (0,1)]
        args.directory.mkdir(parents=True,exist_ok=True)
        manifest={}
        for name,kind,src,secs in [('turns','turns',sources,60),('reverse','turns',sources[::-1],60),
                                   ('repeat','repeat',[base/'artifacts/fleurs-ja_jp-validation-0-1519.wav'],60),
                                   ('silence','silence',[],12),('tone','tone',[],12)]:
            audio,ref,provenance=make_fixture(kind,src,secs)
            wav=args.directory/(name+'.wav');write_float_wav(wav,audio)
            assert read_wav(wav)==audio
            metadata={'sha256':sha256(wav),'samples':len(audio),'sources':provenance,'reference':ref}
            (args.directory/(name+'.reference.json')).write_text(json.dumps(metadata)+'\n')
            manifest[name]={k:v for k,v in metadata.items() if k!='reference'}
        (args.directory/'fixtures.json').write_text(json.dumps(manifest,indent=2)+'\n')
        print(json.dumps(manifest,indent=2));return
    if not args.engine or not args.model or not args.output:ap.error('--engine/--model/--output required')
    wav=args.directory/(args.fixture+'.wav')
    meta=json.loads((args.directory/(args.fixture+'.reference.json')).read_text())
    if sha256(wav)!=meta['sha256']:raise ValueError('Fixture hash changed')
    audio=read_wav(wav)
    if len(audio)!=meta['samples'] or len(audio)!=len(meta['reference']):raise ValueError('Fixture length changed')
    if args.limit_seconds:
        if args.engine!='cpp' or not 0<args.limit_seconds<=len(audio)/16000:ap.error('Valid cpp prefix limit required')
        audio=audio[:round(args.limit_seconds*16000)]
        meta['reference']=meta['reference'][:len(audio)]
    model_hash=sha256(args.model)
    if args.require_simd and args.engine!='cpp':ap.error('--require-simd only applies to cpp')
    result=cpp_run(args.library,args.model,audio,args.profile,args.pace,args.trace,args.require_simd) if args.engine=='cpp' else rs_run(args.binary,args.model,wav,args.profile,args.pace)
    ready,events,finish,segments,cpu,rss=result
    stats=summary(events,ready,finish,segments,meta['reference'],audio,args.pace,cpu,rss)
    stats.update({'engine':args.engine,'profile':args.profile,'fixture':args.fixture,
                  'audio_sha256':meta['sha256'],'model_sha256':model_hash,
                  'prefix_limit_seconds':args.limit_seconds or None,'input_samples':len(audio)})
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps({'summary':stats,'segments':segments,'events':events,'finish':finish},indent=2)+'\n')
    print(json.dumps(stats,indent=2),flush=True)


if __name__=='__main__':main()
