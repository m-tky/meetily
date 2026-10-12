"""Summarize measured outputs only; run after the sequential suites finish."""
import argparse,json,pathlib,statistics,subprocess
p=argparse.ArgumentParser();p.add_argument('root');a=p.parse_args();root=pathlib.Path(a.root).resolve();results=root/'results'
def rows(path):return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
summary={'scope':'10 fixed public reading clips per language, no meeting acceptance; timings exclude load/warm-up','accuracy':[],'latency':[],'paced':[],'pilots':[]}
for engine in ['c','native']:
 for size in ['0.6B','1.7B']:
  path=results/f'{engine}-{size}-t6-accuracy-r1.jsonl'
  report=json.loads(subprocess.check_output(['node',str(pathlib.Path(__file__).parent/'metrics.mjs'),str(root/'artifacts/manifest.json'),str(path)],text=True))
  assert all(g['complete'] for g in report['groups'])
  (results/f'{engine}-{size}-t6-accuracy.report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
  data=rows(path);groups=report['groups']
  # Use scorer audio totals: silence ID may change without contaminating RTF.
  summary['accuracy'].append({'engine':engine,'size':size,'threads':6,'groups':groups,'weighted_rtf':sum(g['elapsed_seconds'] for g in groups)/sum(g['audio_seconds'] for g in groups),'peak_rss_gib':max(r['peak_rss_kib'] for r in data)/1024**2,'silence':report['silence']})
  latency=rows(results/f'{engine}-{size}-t6-latency-r3.jsonl')
  for duration in [2,4,8]:
   selected=[r for r in latency if abs(r['audio_seconds']-duration)<1e-6]
   assert len(selected)==9
   summary['latency'].append({'engine':engine,'size':size,'prefix_seconds':duration,'count':len(selected),'inference_median_seconds':statistics.median(r['elapsed_seconds'] for r in selected),'first_token_median_seconds':statistics.median(r['first_token_seconds'] for r in selected if r.get('first_token_seconds') is not None)})
  for mode in (['paced'] if engine=='c' else ['paced','reconcile']):
   for r in rows(results/f'{engine}-{size}-t6-{mode}-r1.jsonl'):
    summary['paced'].append({**r,'runtime':r['engine'],'engine':engine,'size':size,'mode':mode})
for path in sorted(results.glob('*-pilot-r3*.jsonl')):
 if not path.name.startswith(('c-','native-')):continue
 data=rows(path);assert len(data)==9 and all(r['status']=='ok' for r in data)
 summary['pilots'].append({'file':path.name,'inference_seconds':sum(r['elapsed_seconds'] for r in data),'cpu_seconds':sum(r['cpu_seconds'] for r in data) if all('cpu_seconds' in r for r in data) else None,'weighted_rtf':sum(r['elapsed_seconds'] for r in data)/sum(r['audio_seconds'] for r in data)})
(results/'validated-summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n')
for item in summary['accuracy']:
 g=item['groups'];print(item['engine'],item['size'],'JA CER',round(g[0]['cer']*100,2),'EN WER',round(g[1]['wer']*100,2),'TH CER',round(g[2]['cer']*100,2),'RTF',round(item['weighted_rtf'],3),'RSS GiB',round(item['peak_rss_gib'],2))
