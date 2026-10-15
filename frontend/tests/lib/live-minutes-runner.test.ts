import { expect, test } from 'bun:test';
import { LiveMinutesRunner, type MinutesSession, type MinutesConfig } from '../../src/lib/live-minutes-runner';
const config: MinutesConfig = { model: 'explicit-model', provider: 'chatgpt_plan', account: 'synthetic-account' };
const deferred = <T,>() => { let resolve!: (v: T) => void; const promise = new Promise<T>(r => { resolve = r; }); return { promise, resolve }; };
async function flush() { for (let i = 0; i < 12; i++) await Promise.resolve(); }
function setup(update?: (args: any) => Promise<string>) {
  let session: MinutesSession = { id: 'meeting-1', ended: false, transcription_drained: false };
  const calls: any[] = []; const results: [string, boolean][] = []; const messages: string[] = []; const enabled: boolean[] = [];
  const timers = new Map<number, { fn: () => void; ms: number }>(); let n = 0;
  const runner = new LiveMinutesRunner<string>({
    session: async () => session,
    update: async args => { calls.push(args); return update ? update(args) : 'snapshot'; },
    result: (value, final) => results.push([value, final]), message: text => messages.push(text), enabled: value => enabled.push(value),
    scheduler: { set: (fn, ms) => { const id = ++n; timers.set(id, { fn, ms }); return id; }, clear: id => { timers.delete(id as number); } },
  });
  return { runner, calls, results, messages, enabled, timers, setSession: (s: MinutesSession) => { session = s; },
    tick: async () => { const entry = timers.entries().next().value; if (entry) { timers.delete(entry[0]); entry[1].fn(); await flush(); } } };
}
test('no opt-in means Stop causes no minutes/model request', () => {
  const s = setup(); s.runner.recordingStopped({ minutes_session_id: 'meeting-1', transcription_drained: true });
  expect(s.calls).toHaveLength(0); expect(s.results).toHaveLength(0);
});
test('Stop queues one final pass after in-flight live update with frozen explicit provider', async () => {
  const live = deferred<string>();
  const s = setup(args => args.finalPass ? Promise.resolve('tail-snapshot') : live.promise);
  const started = s.runner.start(config); await flush();
  expect(s.calls).toHaveLength(1);
  s.runner.recordingStopped({ minutes_session_id: 'meeting-1', transcription_drained: true });
  expect(s.calls).toHaveLength(1);
  live.resolve('live-snapshot'); await started; await flush();
  expect(s.calls).toEqual([
    { ...config, sessionId: 'meeting-1', finalPass: false },
    { ...config, sessionId: 'meeting-1', finalPass: true },
  ]);
  expect(s.results).toEqual([['live-snapshot', false], ['tail-snapshot', true]]);
  expect(s.timers.size).toBe(0); expect(s.enabled.at(-1)).toBe(false);
  s.runner.recordingStopped({ minutes_session_id: 'meeting-1', transcription_drained: true });
  expect(s.calls).toHaveLength(2);
});
test('incomplete transcription tail skips final, suppresses settling live result', async () => {
  const live = deferred<string>(); const s = setup(() => live.promise);
  const started = s.runner.start(config); await flush();
  s.runner.recordingStopped({ minutes_session_id: 'meeting-1', transcription_drained: false });
  live.resolve('stale'); await started;
  expect(s.calls).toHaveLength(1); expect(s.results).toHaveLength(0); expect(s.timers.size).toBe(0);
  expect(s.messages.at(-1)).toContain('processing incomplete');
});
test('cancel/new recording does not display old request or react to stale Stop', async () => {
  const old = deferred<string>(); const s = setup(args => args.sessionId === 'meeting-1' ? old.promise : Promise.resolve('new'));
  const started = s.runner.start(config); await flush(); s.runner.cancel();
  s.setSession({ id: 'meeting-2', ended: false, transcription_drained: false });
  await s.runner.start({ ...config, provider: 'ollama', account: null });
  s.runner.recordingStopped({ minutes_session_id: 'meeting-1', transcription_drained: true });
  old.resolve('old'); await started; await s.tick();
  expect(s.results).toEqual([['new', false]]);
  expect(s.calls[1].sessionId).toBe('meeting-2'); expect(s.calls[1].provider).toBe('ollama');
  s.runner.cancel();
});
test('usage limit pauses without final request or provider fallback', async () => {
  const s = setup(async () => { throw 'CHATGPT_USAGE_LIMIT: synthetic'; });
  await s.runner.start(config);
  s.runner.recordingStopped({ minutes_session_id: 'meeting-1', transcription_drained: true });
  expect(s.calls).toHaveLength(1); expect(s.timers.size).toBe(0); expect(s.enabled.at(-1)).toBe(false);
});
test('temporary retries are bounded with backoff and unchanged provider', async () => {
  const s = setup(async () => { throw 'CHATGPT_TEMPORARY: synthetic'; });
  await s.runner.start(config);
  expect([...s.timers.values()][0].ms).toBe(30000); await s.tick();
  expect([...s.timers.values()][0].ms).toBe(60000); await s.tick();
  expect(s.calls).toHaveLength(3); expect(s.timers.size).toBe(0);
  expect(s.calls.every(c => c.provider === 'chatgpt_plan' && c.model === config.model)).toBe(true);
});
test('manual final retry requires a stopped, drained session', async () => {
  const s = setup(); s.setSession({ id: 'meeting-1', ended: true, transcription_drained: false });
  await s.runner.start(config, true); expect(s.calls).toHaveLength(0);
  s.setSession({ id: 'meeting-1', ended: true, transcription_drained: true });
  await s.runner.start(config, true); expect(s.calls[0].finalPass).toBe(true);
  expect(s.results).toEqual([['snapshot', true]]); expect(s.timers.size).toBe(0);
});
