import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import ts from 'typescript';
const source = path => readFileSync(new URL(path, import.meta.url), 'utf8');
const compiled = ts.transpileModule(source('../../src/lib/qwen-setup.ts'), { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
const { prepareAndSelectQwen } = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);
const settings = { worker_path: 'C:\\app\\qwen\\worker.exe', model_dir: 'C:\\model', ort_library: 'C:\\app\\qwen\\onnxruntime.dll' };
function harness(phases, overrides = {}) {
  const calls = [], applied = [], seen = [];
  return { calls, applied, seen, options: {
    consent: true, isActive: () => true, wait: async () => {},
    invoke: async (cmd, args) => { calls.push([cmd, args]); return cmd === 'qwen_setup_start' ? 'run' : phases.shift(); },
    onStatus: s => seen.push(s.phase), onPrepared: async s => applied.push(s), ...overrides,
  } };
}
const status = phase => ({ id: 'run', phase, settings: phase === 'ready' ? settings : null, error: null });
test('no consent means no IPC, downloads or selection', async () => {
  const h = harness([], { consent: false });
  await assert.rejects(prepareAndSelectQwen(h.options), /Consent/);
  assert.deepEqual(h.calls, []); assert.deepEqual(h.applied, []);
});
test('explicit start selects only after initialization and validation report ready', async () => {
  const h = harness(['checking', 'downloading', 'initializing', 'validating', 'ready'].map(status));
  await prepareAndSelectQwen(h.options);
  assert.deepEqual(h.calls[0], ['qwen_setup_start', { consent: true }]);
  assert.deepEqual(h.seen, ['checking', 'downloading', 'initializing', 'validating', 'ready']);
  assert.deepEqual(h.applied, [settings]);
});
for (const phase of ['failed', 'cancelled']) test(`${phase} never selects Qwen`, async () => {
  const h = harness([status(phase)]);
  await assert.rejects(prepareAndSelectQwen(h.options), new RegExp(phase));
  assert.deepEqual(h.applied, []);
});
test('unmounted UI does not select; native job can be reviewed later', async () => {
  const h = harness([status('ready')], { isActive: () => false });
  await prepareAndSelectQwen(h.options);
  assert.deepEqual(h.applied, []); assert.equal(h.calls.length, 1);
});
test('another job cannot trigger selection for this consent', async () => {
  const h = harness([{ ...status('ready'), id: 'other' }]);
  await assert.rejects(prepareAndSelectQwen(h.options), /Setup changed/);
  assert.deepEqual(h.applied, []);
});
test('selection errors propagate; failed onboarding is retryable', async () => {
  const h = harness([status('ready')], { onPrepared: async () => { throw new Error('store failure'); } });
  await assert.rejects(prepareAndSelectQwen(h.options), /store failure/);
});
test('Qwen-only onboarding does not configure summary/defaults or claim other downloads', () => {
  const native = source('../../src-tauri/src/onboarding.rs').split('pub async fn complete_qwen_onboarding')[1].split('#[cfg(test)]')[0];
  assert.match(native, /qwen_init/); assert.match(native, /selected.provider != "qwen"/);
  assert.doesNotMatch(native, /save_model_config|save_transcript_config|downloaded"|model_status\./);
  const context = source('../../src/contexts/OnboardingContext.tsx').split('const completeQwenOnboarding')[1].split('// Start background downloads')[0];
  assert.match(context, /complete_qwen_onboarding/);
  assert.doesNotMatch(context, /download_model|requestSummary|setParakeet|setSummaryModel/);
});
test('candidate worker is validated before saving local paths or provider selection', () => {
  const ui = source('../../src/components/QwenModelSettings.tsx');
  assert.ok(ui.indexOf("invoke('qwen_validate_settings'") < ui.indexOf("invoke('qwen_save_settings'"));
  assert.ok(ui.indexOf("invoke('qwen_init')") < ui.indexOf("invoke('api_save_transcript_config'"));
});
test('Linux uses the same explicit Qwen-only entry point without mandatory summary models', async () => {
  const linux = { worker_path: '/app/libexec/qwen-speech-worker', model_dir: '/data/models/Qwen3-ASR-1.7B', ort_library: '/app/libexec/libonnxruntime.so' };
  const h = harness([{ ...status('ready'), settings: linux }]);
  await prepareAndSelectQwen(h.options);
  assert.deepEqual(h.applied, [linux]);
  const native = source('../../src-tauri/src/onboarding.rs').split('pub async fn complete_qwen_onboarding')[1].split('#[cfg(test)]')[0];
  assert.match(native, /qwen_is_supported\(\)/);
  assert.doesNotMatch(native, /cfg!\(windows\)/);
  const welcome = source('../../src/components/onboarding/steps/WelcomeStep.tsx');
  assert.match(welcome, /QwenModelSettings automaticOnly/);
  assert.match(welcome, /No local summary model is required/);
  assert.doesNotMatch(source('../../src/components/QwenAutoSetup.tsx'), /locally \(Windows\)/);
});
test('Linux package includes the initializer and runtime beside the worker', () => {
  const pkg = source('../../../nix/package.nix');
  assert.match(pkg, /initializer.*bin\/qwen-model-init \$out\/libexec\/qwen-model-init/);
  assert.match(pkg.split('postFixup =')[1], /lib\/libonnxruntime.so \$out\/libexec\/libonnxruntime.so/);
  assert.doesNotMatch(pkg.split('postFixup =')[0], /ln -s .*\$out\/libexec\/libonnxruntime.so/);
  assert.match(pkg, /ctypes.CDLL/);
  assert.match(pkg, /SSL_CERT_FILE/);
  const provider = source('../../src-tauri/src/audio/transcription/qwen_provider.rs');
  assert.match(provider, /cfg\(target_os = "linux"\)[\s\S]*current_exe\(\)/);
});
test('native setup accepts neither remote URLs nor executable paths from IPC', () => {
  const native = source('../../src-tauri/src/audio/transcription/qwen_setup.rs');
  assert.match(native, /qwen_setup_start<R: Runtime>\(\s*app: tauri::AppHandle<R>,\s*consent: bool,?\s*\)/);
  assert.match(native, /https_only\(true\)/); assert.match(native, /persist_noclobber/);
  assert.match(native, /try_lock_exclusive/); assert.match(native, /JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE/);
});
