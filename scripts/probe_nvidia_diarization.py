#!/usr/bin/env python3
"""Isolated CPU streaming probe. Reads approved public WAVs, never opens audio devices.
Uses the upstream stable C ABI; no application integration or network inference.
"""
import argparse
import array
import ctypes as C
import hashlib
import itertools
import json
import math
from pathlib import Path
import resource
import struct
import sys
import time


class Config(C.Structure):
    _fields_ = [('size', C.c_size_t), ('model_path', C.c_char_p), ('gpu', C.c_int32),
                ('preset', C.c_char_p)] + [(k, C.c_int32) for k in
                ('chunk_frames', 'right_context_frames', 'left_context_frames',
                 'fifo_frames', 'spkcache_frames', 'update_period_frames')]


class Segment(C.Structure):
    _fields_ = [('start_time', C.c_double), ('end_time', C.c_double), ('speaker', C.c_int32)]


def read_wav(path):
    raw = Path(path).read_bytes()
    if raw[:4] != b'RIFF' or raw[8:12] != b'WAVE':
        raise ValueError('Expected RIFF WAV')
    fmt = payload = None
    pos = 12
    while pos + 8 <= len(raw):
        tag, size = struct.unpack_from('<4sI', raw, pos)
        part = raw[pos + 8:pos + 8 + size]
        if len(part) != size:
            raise ValueError('Truncated WAV chunk')
        if tag == b'fmt ':
            fmt = struct.unpack_from('<HHIIHH', part)
        elif tag == b'data':
            payload = part
        pos += 8 + size + size % 2
    if fmt is None or payload is None:
        raise ValueError('Missing fmt/data')
    kind, channels, rate, _, _, bits = fmt
    if channels != 1 or rate != 16000 or (kind, bits) not in ((1, 16), (3, 32)):
        raise ValueError(f'Expected mono 16k PCM16/float32, got {fmt}')
    samples = array.array('h' if kind == 1 else 'f')
    samples.frombytes(payload)
    if sys.byteorder != 'little':
        samples.byteswap()
    out = array.array('f', (x / 32768 for x in samples)) if kind == 1 else samples
    if not all(math.isfinite(x) for x in out):
        raise ValueError('Non-finite audio')
    return out


def make_fixture(kind, sources, seconds):
    target = round(seconds * 16000)
    audio, reference, provenance = array.array('f'), [], []
    if kind in ('silence', 'tone'):
        audio = array.array('f', (0.0 if kind == 'silence' else
                            0.05 * math.sin(2 * math.pi * 440 * i / 16000)
                            for i in range(target)))
        return audio, [0] * target, []
    decoded = [read_wav(p) for p in sources]
    for p in sources:
        provenance.append({'path': str(p), 'sha256': hashlib.sha256(Path(p).read_bytes()).hexdigest()})
    if not decoded or any(not x for x in decoded):
        raise ValueError('Nonempty public sources required')
    turn = 0
    while len(audio) < target:
        idx = 0 if kind == 'repeat' else (turn // 4) % len(decoded)
        clip = decoded[idx]
        n = min(len(clip), target - len(audio))
        audio.extend(clip[:n])
        reference.extend([idx + 1] * n)
        gap = min(8000, target - len(audio))
        audio.extend([0.0] * gap)
        reference.extend([0] * gap)
        turn += 1
    return audio, reference, provenance


def labels(probs, speakers):
    result = []
    for i in range(0, len(probs), speakers):
        row = probs[i:i + speakers]
        best = max(range(speakers), key=row.__getitem__)
        result.append(best + 1 if row[best] >= 0.5 else 0)
    return result


def proxy_score(audio, reference, predicted, dt, speakers):
    # Known source identity only on 10/80ms bins whose RMS exceeds .005.
    # NOT human-annotated DER: concatenated repeated words, no overlap/collar.
    truth = []
    stride = round(dt * 16000)
    for i in range(len(predicted)):
        chunk = audio[i * stride:(i + 1) * stride]
        ref = reference[i * stride:(i + 1) * stride]
        truth.append(max(set(ref), key=ref.count) if ref and chunk and
                     math.sqrt(sum(x*x for x in chunk) / len(chunk)) > .005 else 0)
    ids = sorted(set(truth) - {0})
    if not ids:
        return {'voiced_frames': 0, 'false_speaker_frames': sum(x != 0 for x in predicted)}
    best_correct, best_map = -1, None
    for permutation in itertools.permutations(range(1, speakers + 1), len(ids)):
        mapping = dict(zip(ids, permutation))
        correct = sum(t != 0 and p == mapping[t] for t, p in zip(truth, predicted))
        if correct > best_correct:
            best_correct, best_map = correct, mapping
    total = sum(t != 0 for t in truth)
    return {'voiced_frames': total, 'source_identity_accuracy_proxy': best_correct / total,
            'best_reference_to_cluster': best_map, 'not_standard_der': True}


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--library', type=Path, required=True)
    ap.add_argument('--model', type=Path, required=True)
    ap.add_argument('--source', type=Path, action='append', default=[])
    ap.add_argument('--fixture', choices=['repeat', 'turns', 'silence', 'tone'], default='repeat')
    ap.add_argument('--seconds', type=float, default=60)
    ap.add_argument('--chunk-frames', type=int, default=0)
    ap.add_argument('--cache-frames', type=int, default=0)
    ap.add_argument('--fifo-frames', type=int, default=0)
    ap.add_argument('--pace', action='store_true')
    ap.add_argument('--output', type=Path, required=True)
    args = ap.parse_args()
    if not 0 < args.seconds <= 600 or args.chunk_frames < 0:
        ap.error('Bounded 0<seconds<=600 and nonnegative geometry required')
    audio, reference, sources = make_fixture(args.fixture, args.source, args.seconds)
    lib = C.CDLL(str(args.library.resolve()))
    ptr = C.c_void_p
    def bind(name, ret, types):
        fn = getattr(lib, 'nemo_speech_' + name)
        fn.restype, fn.argtypes = ret, types
        return fn
    last_error = bind('asr_last_error', C.c_char_p, [])
    create = bind('diar_create', C.c_int, [C.POINTER(Config), C.POINTER(ptr)])
    destroy = bind('diar_destroy', None, [ptr])
    stream_open = bind('diar_stream_open', C.c_int, [ptr, C.POINTER(ptr)])
    close = bind('diar_stream_close', None, [ptr])
    push = bind('diar_stream_push_f32', C.c_int, [ptr, C.POINTER(C.c_float), C.c_size_t, C.c_int32])
    finish = bind('diar_stream_finish', C.c_int, [ptr])
    count = bind('diar_frame_count', C.c_int64, [ptr])
    get_probs = bind('diar_frame_probs', C.c_int, [ptr, C.POINTER(C.c_float), C.c_size_t])
    n_speakers = bind('diar_num_speakers', C.c_int32, [ptr])
    cadence = bind('diar_seconds_per_frame', C.c_double, [ptr])
    get_segments = bind('diar_segments', C.c_int, [ptr, ptr, C.POINTER(Segment), C.c_size_t, C.POINTER(C.c_size_t)])
    def check(status):
        if status:
            raise RuntimeError(f'C ABI status={status}: {last_error().decode()}')
    cfg = Config()
    cfg.size, cfg.model_path, cfg.gpu = C.sizeof(cfg), str(args.model.resolve()).encode(), -1
    cfg.preset, cfg.chunk_frames, cfg.left_context_frames = b'v3-streaming', args.chunk_frames, -1
    cfg.spkcache_frames, cfg.fifo_frames = args.cache_frames, args.fifo_frames
    model, stream = ptr(), ptr()
    load_start = time.monotonic()
    check(create(C.byref(cfg), C.byref(model)))
    load_seconds = time.monotonic() - load_start
    speakers, dt = n_speakers(model), cadence(model)
    buffer = (C.c_float * len(audio)).from_buffer(audio)
    events, previous, revisions = [], [], []
    first_appearance, last_change = [], []
    first_output = first_speaker = None
    compute_sum = 0.0
    try:
        check(stream_open(model, C.byref(stream)))
        start, cpu_start = time.monotonic(), time.process_time()
        def snapshot(input_seconds, elapsed, phase):
            nonlocal previous, first_output, first_speaker
            frames = count(stream)
            probs = (C.c_float * (frames * speakers))()
            check(get_probs(stream, probs, len(probs)))
            current = labels(probs, speakers)
            if frames and first_output is None:
                first_output = {'input_seconds': input_seconds, 'wall_seconds': elapsed}
            if any(current) and first_speaker is None:
                first_speaker = {'input_seconds': input_seconds, 'wall_seconds': elapsed}
            changed = [i for i, (a, b) in enumerate(zip(previous, current)) if a != b]
            for i in changed:
                last_change[i] = elapsed
            for i in range(len(previous), len(current)):
                first_appearance.append(elapsed)
                last_change.append(elapsed)
            if changed:
                revisions.append({'input_seconds': input_seconds, 'phase': phase, 'frames': len(changed),
                                  'oldest_revised_audio_seconds': min(changed) * dt})
            previous = current
            return frames, len(changed)
        step = 1920  # 120ms, matching current app delivery window; no app capture used.
        old_count = 0
        for offset in range(0, len(audio), step):
            n = min(step, len(audio) - offset)
            input_seconds = (offset + n) / 16000
            if args.pace:
                time.sleep(max(0, start + input_seconds - time.monotonic()))
            t = time.monotonic()
            check(push(stream, C.cast(C.byref(buffer, offset * 4), C.POINTER(C.c_float)), n, 16000))
            push_seconds = time.monotonic() - t
            compute_sum += push_seconds
            new_count = count(stream)
            elapsed = time.monotonic() - start
            changed = 0
            if new_count != old_count:
                _, changed = snapshot(input_seconds, elapsed, 'push')
            # includes result retrieval time, so simulated queue lateness is conservative.
            done = time.monotonic() - start
            events.append({'input_seconds': input_seconds, 'push_seconds': push_seconds,
                           'wall_seconds': done, 'frames': new_count, 'revised_frames': changed,
                           'completion_lateness_seconds': max(0, done - input_seconds) if args.pace else None,
                           'output_tail_seconds': input_seconds - new_count * dt})
            old_count = new_count
        t = time.monotonic()
        check(finish(stream))
        flush_seconds = time.monotonic() - t
        snapshot(len(audio)/16000, time.monotonic() - start, 'finish')
        total_seconds, cpu_seconds = time.monotonic()-start, time.process_time()-cpu_start
        size = C.c_size_t()
        check(get_segments(stream, None, None, 0, C.byref(size)))
        segments = (Segment * size.value)()
        if size.value:
            check(get_segments(stream, None, segments, size.value, C.byref(size)))
        inference = [e['push_seconds'] for e, prev in zip(events, [None] + events[:-1])
                     if e['frames'] > (prev['frames'] if prev else 0)]
        voiced_ages = sorted(first_appearance[i] - i*dt
                             for i, label in enumerate(previous) if label)
        summary = {'fixture': args.fixture, 'seconds': len(audio)/16000, 'paced': args.pace,
                   'cpu_only': True, 'chunk_frames': args.chunk_frames or 13,
                   'cache_frames': args.cache_frames or 264, 'fifo_frames': args.fifo_frames or 80,
                   'upstream_default_cpu_threads': 4,
                   'push_ms': 120, 'output_cadence_seconds': dt, 'speaker_capacity': speakers,
                   'model_sha256': hashlib.sha256(args.model.read_bytes()).hexdigest(),
                   'sources': sources, 'load_seconds': load_seconds,
                   'push_compute_seconds': compute_sum, 'flush_seconds': flush_seconds,
                   'compute_rtf': (compute_sum+flush_seconds)/(len(audio)/16000),
                   'wall_seconds': total_seconds, 'cpu_seconds': cpu_seconds,
                   'max_rss_mib': resource.getrusage(resource.RUSAGE_SELF).ru_maxrss/1024,
                   'inference_calls': len(inference),
                   'inference_push_max_seconds': max(inference, default=0),
                   'inference_push_mean_seconds': sum(inference)/len(inference) if inference else 0,
                   'completion_lateness_max_seconds': max((e['completion_lateness_seconds'] or 0 for e in events), default=0),
                   'first_output': first_output, 'first_speaker': first_speaker,
                   'voiced_frame_output_age_median_seconds': voiced_ages[len(voiced_ages)//2] if args.pace and voiced_ages else None,
                   'voiced_frame_output_age_max_seconds': max(voiced_ages) if args.pace and voiced_ages else None,
                   'frame_count': len(previous), 'labeled_audio_seconds': len(previous)*dt,
                   'revised_frame_events': sum(r['frames'] for r in revisions),
                   'segments': [{'start': s.start_time, 'end': s.end_time, 'speaker': s.speaker} for s in segments],
                   'proxy_score': proxy_score(audio, reference, previous, dt, speakers),
                   'provisional_labels_not_identity': True,
                   'asr_concurrency_not_tested': True}
        report = {'summary': summary, 'events': events, 'revisions': revisions,
                  'final_labels': previous, 'frame_first_output_wall_seconds': first_appearance,
                  'frame_last_change_wall_seconds': last_change}
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2)+'\n')
        print(json.dumps(summary, indent=2), flush=True)
    finally:
        if stream:
            close(stream)
        destroy(model)


if __name__ == '__main__':
    main()
