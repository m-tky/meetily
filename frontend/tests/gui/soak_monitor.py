"""Read-only diagnostics for this harness's owned native/ASR process group.
RSS is per-process resident memory (not unique RAM, a leak proof or a budget gate).
Never enumerate command lines, environments, host devices or credentials.
"""
import json
import os
import pathlib
import statistics
import time


def status_rss(status):
    for line in status.splitlines():
        if line.startswith('VmRSS:'):
            return int(line.split()[1])
    return 0


class SoakMonitor:
    def __init__(self, root, group, native, worker):
        self.root, self.group = root, group
        self.native, self.worker = native, worker
        self.native_executables = {native}
        # Nix makeWrapper is a shell script in bin; /proc/exe names libexec.
        companion = native.parent.parent/'libexec'/native.name
        if native.parent.name=='bin' and companion.is_file():
            self.native_executables.add(companion.resolve())
        self.worker_pids = set()
        self.rows = []
        self.start = time.monotonic()

    def sample(self, folder):
        row = {'seconds':time.monotonic()-self.start, 'native_rss_kib':0,
               'worker_rss_kib':0, 'worker_pids':[], 'saved_segments':0,
               'temporary_inputs':len(list((self.root/'cache/com.meetily.ai/qwen-audio').glob('*')))}
        for entry in pathlib.Path('/proc').iterdir():
            if not entry.name.isdigit(): continue
            try:
                pid = int(entry.name)
                if os.getpgid(pid)!=self.group: continue
                exe = (entry/'exe').resolve()
                if exe not in self.native_executables and exe!=self.worker: continue
                rss = status_rss((entry/'status').read_text())
                if exe==self.worker:
                    row['worker_pids'].append(pid)
                    self.worker_pids.add(pid)
                    row['worker_rss_kib'] += rss
                else:
                    row['native_rss_kib'] += rss
            except (OSError, ProcessLookupError): continue
        path = folder/'transcripts.json'
        try: row['saved_segments'] = len(json.loads(path.read_text())['segments'])
        except FileNotFoundError: pass  # Initial silence hasn't produced a transcript yet.
        self.rows.append(row)
        with (self.root/'soak-samples.jsonl').open('a') as output:
            output.write(json.dumps(row)+'\n')

    def finish(self, seconds, bursts):
        assert self.rows and self.rows[-1]['native_rss_kib']>0
        assert self.worker_pids, 'No real ASR process observed'
        # Initial model loading is excluded from the two plateau windows.
        head = [r for r in self.rows if 60<=r['seconds']<120]
        tail = [r for r in self.rows if r['seconds']>=seconds-60]
        summary = {'elapsed_seconds':seconds, 'bursts':bursts,
                   'samples':len(self.rows), 'worker_pids':sorted(self.worker_pids),
                   'max_pending_temporary_inputs':max(r['temporary_inputs'] for r in self.rows),
                   'last_saved_segments_before_stop':self.rows[-1]['saved_segments'],
                   'rss_units':'KiB, native executable and ASR executable only; excludes WebKit/helper/driver',
                   'not_a_leak_or_long_meeting_acceptance':True}
        for key in ['native_rss_kib', 'worker_rss_kib']:
            summary[key] = {'peak':max(r[key] for r in self.rows),
                            'head_60_120s_median':statistics.median(r[key] for r in head) if head else None,
                            'last_60s_median':statistics.median(r[key] for r in tail)}
        (self.root/'soak-summary.json').write_text(json.dumps(summary,indent=2))
