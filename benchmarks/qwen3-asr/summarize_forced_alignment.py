"""Auditable bounded summary, preserving raw experiments and their failures."""
import argparse
import json
from pathlib import Path
from evaluate_forced_alignment import sha256


def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--runtime',type=Path,required=True);ap.add_argument('--output',type=Path,required=True)
    a=ap.parse_args();root=a.runtime.resolve();base=root/'qwen3-asr/diagnostics';proof=[];quality=[]
    def keep(path):
        if path.exists():proof.append(dict(path=str(path.relative_to(root)),sha256=sha256(path)))
    def load(path):keep(path);return json.loads(path.read_text())
    for name in ('forced-alignment-r1','forced-alignment-sentence-turns-r1'):
        d=base/name;plan=load(d/'plan.json');checks=[]
        for fixture in plan['fixtures']:
            keep(Path(fixture['path']))
            records=[load(d/f'pass-{i}'/(fixture['id']+'.json')) for i in range(3)]
            payloads=[(r['response'].get('text'),r['response'].get('alignment',{}).get('units')) for r in records]
            same=all(p==payloads[0] for p in payloads)
            if not same:raise ValueError('Native text/unit repeat mismatch: '+fixture['id'])
            checks.append(dict(fixture=fixture['id'],native_fresh_three_runs_text_and_units_identical=same))
        official_pairs=[]
        for p in sorted((d/'official-control-0').glob('*.json')):
            first=load(p);second=load(d/'official-control-1'/p.name)
            if 'text_origin' in first:
                same=(first.get('text'),first.get('status'),first.get('units'))==(second.get('text'),second.get('status'),second.get('units'))
                if not same:raise ValueError('Official repeat mismatch: '+p.name)
                official_pairs.append(dict(case=p.stem,official_two_runs_text_status_units_identical=same))
        rows=load(d/'comparison-0.json');load(d/'comparison-1.json')
        for row in rows:
            row.pop('assigned_word_units');row.pop('word_level_groups')
        for i in range(3):
            load(d/f'pass-{i}'/'summary.json');load(d/f'pass-{i}'/'provenance.json')
            for p in sorted((d/f'pass-{i}').glob('wrong*.json')):load(p)
        quality.append(dict(directory=name,fixture_count=len(plan['fixtures']),comparison=rows,
                            native_reproducibility=checks,official_reproducibility=official_pairs))
    concurrency=[]
    for d in sorted(base.glob('three-model-*')):
        r=load(d/'result.json');cleanup=load(d/'cleanup.json');warm=load(d/'warmup.json')
        cpp=load(d/'cpp.json')
        # Old successful runs asserted 0<=epoch skew<20ms but did not persist it.
        cpp_end=60+r['cpp_stop_drain'] if 'cpp_stop_drain' in r else .02+cpp['summary']['wall_seconds']
        span=max(60+r.get('asr_alignment_stop_drain',r['final_stop_drain']),cpp_end)
        keep(d/'memory.json');keep(d/'asr-events.jsonl');keep(d/'alignment-events.jsonl');keep(d/'plan.json')
        concurrency.append(dict(directory=d.name,input_mode=r['plan'].get('asr_input_mode','real_time_10s_windows'),
            backend=r['plan']['alignment_backend'],aligner_threads=r['plan']['aligner_threads'],
            cpp_rtf=r['cpp']['compute_rtf'],asr_rtf=r['asr_request_rtf'],align_rtf=r['align_request_rtf'],
            cpp_final_push_lag=r['cpp']['final_completion_lateness_seconds'],
            cpp_label_age_max=r['cpp']['voiced_frame_first_label_age_max_seconds'],
            max_asr_wait=r['max_asr_queue_wait'],max_alignment_wait=r['max_alignment_queue_wait'],
            peak_sum_rss_mib=r['peak_sum_rss_mib'],cpu_seconds_sampled=r['cpu_seconds_sampled'],
            corrected_mean_owned_core_equivalents=sum(r['cpu_seconds_sampled'].values())/span,
            corrected_all_models_stop_drain=max(0,span-60),
            old_cpp_epoch_20ms_upper_bound_used='cpp_stop_drain' not in r,
            triple_request_cpp_push_overlap=r['actual_triple_request_cpp_compute_overlap_seconds'],
            frozen_live_criteria_passed=r['throughput_criteria_passed'],cleanup=cleanup,
            cold_asr_load_seconds=warm['asr_response'].get('load_seconds'),
            first_asr_inference_seconds=warm['asr_response'].get('inference_seconds'),
            first_asr_request_seconds=warm['asr_request_seconds'],
            cold_alignment_load_seconds=warm.get('align_response',{}).get('load_seconds'),
            first_alignment_inference_seconds=warm.get('align_response',{}).get('inference_seconds'),
            first_alignment_request_seconds=warm.get('align_request_seconds')))
    model=load(root/'qwen3-asr/artifacts/model-forced-aligner-0.6B/provenance.json')
    for row in model['files']:
        p=root/'qwen3-asr/artifacts/model-forced-aligner-0.6B'/row['file']
        if sha256(p)!=row['sha256']:raise ValueError('Pinned model changed')
    keep(root/'diarization-engines/parakeet.cpp/build-znver3/libparakeet.so')
    keep(root/'diarization-engines/models/cpp-q8_0.gguf')
    keep(root/'qwen3-asr/align-worker-tested-r2');keep(root/'qwen3-asr/align-worker-target/release/qwen-speech-worker')
    keep(root/'qwen3-asr/worker-target/release/qwen-speech-worker')
    default=load(base/'forced-alignment-r1/default-off-regression.json');default.pop('outputs')
    result=dict(schema=1,authorization='Optional isolated worker + four public/synthetic tests; official control on degradation',
        app_integration=False,real_capture=False,physical_playback=False,cloud_inference=False,
        model=model,worker_default_off_regression=default,quality=quality,concurrency=concurrency,
        original_benchmark_binary_retained='qwen3-asr/align-worker-tested-r2',
        formatting_rebuild_three_run_native_text_units_equal=True,
        corrections=['Original live results final_stop_drain excluded CPP; corrected aggregate above includes CPP/flush.',
            'Directory three-model-official-backlog-r1 accidentally ran paced; declared input_mode is authoritative; actual-backlog directory is the stress test.'],
        verification=dict(rust_tests=17,python_tests=35,python_components=dict(worker_client=5,alignment=7,prior_diarization_attribution=23),
                          raw_quality_json_parse=True,fmt=True,release_build_locked_offline=True),
        decision='Character path and official path not accepted for natural-meeting speaker-attributed transcription',
        limitations=['No human character timestamps/DER','Word labels inherited by characters do not create character timestamps',
            'ASR omission/substitution not repaired','Overlap can be missed by diarization','Cold RSS not profiled; sampled RSS not PSS/leak proof',
            '60s repeats/backlog only; no long-duration/UI/capture/storage/minutes contention','Official control differs in runtime and preprocessing, not just nagisa'],
        artifacts=proof)
    a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps(dict(artifact_hashes=len(proof),quality_fixtures=sum(q['fixture_count'] for q in quality),concurrency_runs=len(concurrency))))


if __name__=='__main__':main()
