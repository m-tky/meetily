"""Parse only explicitly tagged numeric transport telemetry, never speech/log text."""
import re


def summarize_transport(log):
    timings = {}
    for role in ['Microphone', 'System']:
        rows = re.findall(r'Audio transport handoff: source=' + role + r', chunk=(\d+), samples=(\d+), at_s=([0-9.]+)', log)
        times = [float(t) for _, _, t in rows]
        gaps = sorted((b-a)*1000 for a,b in zip(times, times[1:]))
        receives = re.findall(r'Audio transport receive: source=' + role + r', chunk=(\d+), samples=(\d+), at_s=([0-9.]+), handoff_s=([0-9.]+)', log)
        queue_lags = [(float(at)-float(sent))*1000 for _, _, at, sent in receives]
        timings[role] = {'callbacks': len(rows), 'samples': sum(int(n) for _,n,_ in rows),
            'first_handoff_s': times[0] if times else None,
            'last_handoff_s': times[-1] if times else None,
            'gap_max_ms': max(gaps, default=0),
            'gap_p95_ms': gaps[round((len(gaps)-1)*.95)] if gaps else 0,
            'queue_lag_max_ms': max(queue_lags, default=0)}
    mix_rows = re.findall(r'Audio transport mix: start_sample=(\d+), samples=(\d+), at_s=([0-9.]+), finishing=(true|false), degraded_started=(true|false)', log)
    return {'transport_timing': timings,
        'first_mix_output_s': float(mix_rows[0][2]) if mix_rows else None,
        'degraded_mix_times_s': [float(at) for _,_,at,_,degraded in mix_rows if degraded == 'true'],
        'late_samples_by_source': {role: sum(int(n) for n in re.findall(
            r'Discarded (\d+) late audio samples from an already emitted interval \(source=' + role + r'\)', log)) for role in timings}}
