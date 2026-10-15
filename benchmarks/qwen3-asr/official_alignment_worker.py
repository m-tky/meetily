"""Bounded opt-in official alignment subprocess for public three-model benchmarks.
No ASR operations; uses unmodified qwen-asr0.0.6/nagisa and local CPU weights.
"""
import argparse
import json
import os
import sys
import time
os.environ.update(HF_HUB_OFFLINE='1',TRANSFORMERS_OFFLINE='1',HF_HUB_DISABLE_TELEMETRY='1',DO_NOT_TRACK='1')


def emit(v):print(json.dumps(v,ensure_ascii=False),flush=True)


def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('unused_asr_model');ap.add_argument('threads',type=int)
    ap.add_argument('--aligner-dir',required=True);a=ap.parse_args()
    if not 1<=a.threads<=12:raise ValueError('Invalid threads')
    import torch
    torch.set_num_threads(a.threads);torch.set_num_interop_threads(1)
    from qwen_asr import Qwen3ForcedAligner
    import soundfile as sf
    emit(dict(event='ready',protocol=1,alignment_enabled=True,alignment_unit='official_nagisa_word',threads=a.threads))
    model=None;seen=set()
    while True:
        line=sys.stdin.buffer.readline(4097)
        if not line:return
        if len(line)>4096 or not line.endswith(b'\n'):raise ValueError('Oversize/truncated request')
        r=None
        try:
            r=json.loads(line);identifier=r['id']
            if not isinstance(identifier,str) or not 0<len(identifier)<=64 or any(not(c.isascii() and (c.isalnum() or c in '-_.')) for c in identifier):raise ValueError('Invalid ID')
            if identifier in seen or len(seen)>=1024:raise ValueError('Duplicate/limit')
            seen.add(identifier)
            if r.get('op')=='ping':emit(dict(id=identifier,status='pong'));continue
            if r.get('op')=='shutdown':emit(dict(id=identifier,status='bye'));return
            if set(r)!={'id','op','wav','text','language'} or r['op']!='align' or r['language']!='Japanese':raise ValueError('Invalid operation')
            text=r['text']
            if not isinstance(text,str) or not text.strip() or not 0<len(text)<=512:raise ValueError('Invalid text length')
            info=sf.info(r['wav'])
            if info.samplerate!=16000 or info.channels!=1 or not 0<info.frames<=480000 or info.subtype not in ('FLOAT','PCM_16'):raise ValueError('Invalid WAV')
            samples,rate=sf.read(r['wav'],dtype='float32')
            import numpy as np
            if not np.isfinite(samples).all() or np.max(np.abs(samples))>1:raise ValueError('Invalid samples')
            load_seconds=0.0
            if model is None:
                tick=time.monotonic();model=Qwen3ForcedAligner.from_pretrained(a.aligner_dir,dtype=torch.float32,device_map='cpu',attn_implementation='eager')
                load_seconds=time.monotonic()-tick
            tick=time.monotonic();aligned=model.align(audio=(samples,rate),text=text,language='Japanese')[0]
            units=[dict(text=x.text,start_seconds=x.start_time,end_seconds=x.end_time,confirmed=False) for x in aligned]
            emit(dict(id=identifier,status='aligned',text=text,units=units,unit='official_nagisa_word',
                      load_seconds=load_seconds,inference_seconds=time.monotonic()-tick,confirmed=False,recognition_verified=False))
        except Exception as error:
            emit(dict(id=r.get('id') if isinstance(locals().get('r'),dict) else None,status='error',code='alignment_failed',error=type(error).__name__,confirmed=False))


if __name__=='__main__':main()
