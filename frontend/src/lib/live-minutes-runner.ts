export type MinutesSession = { id: string; ended: boolean; transcription_drained: boolean };
export type MinutesConfig = { model: string; provider: 'ollama' | 'chatgpt_plan'; account: string | null };
export type MinutesStop = { minutes_session_id?: string | null; transcription_drained?: boolean };
type Scheduler = { set: (fn: () => void, ms: number) => unknown; clear: (id: unknown) => void };
type Dependencies<T> = {
  session: () => Promise<MinutesSession | null>;
  update: (config: MinutesConfig & { sessionId: string; finalPass: boolean }) => Promise<T>;
  result: (value: T, final: boolean) => void;
  message: (text: string) => void;
  enabled: (value: boolean) => void;
  scheduler?: Scheduler;
};
/** Explicit consent is frozen per session. Stop queues one final pass after the
 * live request; no overlapping requests, provider fallback or recording calls. */
export class LiveMinutesRunner<T> {
  private generation = 0;
  private active = false;
  private inFlight = false;
  private ending = false;
  private finalPending = false;
  private failures = 0;
  private timer: unknown;
  private context: (MinutesConfig & { sessionId: string }) | null = null;
  private scheduler: Scheduler;
  constructor(private deps: Dependencies<T>) {
    this.scheduler = deps.scheduler ?? { set: (fn, ms) => setTimeout(fn, ms), clear: id => clearTimeout(id as ReturnType<typeof setTimeout>) };
  }
  cancel() {
    this.generation++; this.active = false; this.finalPending = false;
    this.scheduler.clear(this.timer); this.timer = undefined;
    this.deps.enabled(false);
  }
  async start(config: MinutesConfig, finalPass = false) {
    this.cancel(); const generation = this.generation;
    this.active = true; this.ending = finalPass; this.failures = 0;
    this.deps.enabled(true);
    try {
      const session = await this.deps.session();
      if (generation !== this.generation) return;
      if (!session || session.ended !== finalPass || (finalPass && !session.transcription_drained)) {
        throw new Error('No eligible recording session.');
      }
      this.context = { ...config, sessionId: session.id };
      await this.run(generation);
    } catch (e) {
      if (generation === this.generation) { this.deps.message(String(e)); this.cancel(); }
    }
  }
  recordingStopped(event: MinutesStop) {
    if (!this.active || !this.context || event.minutes_session_id !== this.context.sessionId) return;
    this.scheduler.clear(this.timer); this.timer = undefined;
    this.ending = true;
    if (event.transcription_drained !== true) {
      this.deps.message('Final update paused: transcript processing incomplete.');
      this.cancel(); return;
    }
    this.finalPending = true;
    this.deps.message('Final update pending…');
    if (!this.inFlight) void this.run(this.generation);
  }
  private async run(generation: number) {
    if (!this.active || generation !== this.generation || !this.context) return;
    if (this.inFlight) {
      // An old-session request may still be settling; never overlap it.
      this.timer = this.scheduler.set(() => { void this.run(generation); }, 1000); return;
    }
    this.inFlight = true;
    const final = this.ending;
    if (final) this.finalPending = false;
    const context = this.context;
    let delay = 15000;
    this.deps.message(final ? 'Updating final draft…' : 'Updating…');
    try {
      const value = await this.deps.update({ ...context, finalPass: final });
      if (generation !== this.generation || !this.active) return;
      this.failures = 0; this.deps.result(value, final);
      if (final) { this.active = false; this.deps.enabled(false); }
    } catch (error) {
      if (generation !== this.generation || !this.active) return;
      const text = String(error); this.deps.message(text);
      const waiting = text.includes('Waiting for transcript evidence') || text.includes('No current meeting folder') || text.includes('Minutes update already running');
      if (waiting && !final) { /* no model call; keep waiting */ }
      else if (text.startsWith('CHATGPT_TEMPORARY:') || text.includes('timed out') || text.includes('Minutes update already running')) {
        this.failures++; delay = Math.min(15000 * 2 ** this.failures, 300000);
        if (this.failures >= 3) this.cancel();
      } else this.cancel(); // usage/policy/permissions/config: pause, never fallback.
    } finally {
      this.inFlight = false;
      if (generation === this.generation && this.active) {
        if (this.finalPending) void this.run(generation);
        else this.timer = this.scheduler.set(() => { void this.run(generation); }, delay);
      }
    }
  }
}
