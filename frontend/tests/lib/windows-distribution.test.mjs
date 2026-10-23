import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const text = path => readFileSync(new URL(path, import.meta.url), 'utf8');
const json = path => JSON.parse(text(path));
const base = json('../../src-tauri/tauri.conf.json');
const windows = json('../../src-tauri/tauri.windows.conf.json');
const release = json('../../src-tauri/tauri.windows-release.conf.json');
function merge(a,b) {
  const out={...a};
  for (const [k,v] of Object.entries(b)) out[k]=v&&typeof v==='object'&&!Array.isArray(v)?merge(a[k]&&typeof a[k]==='object'&&!Array.isArray(a[k])?a[k]:{},v):v;
  return out;
}
const config=merge(merge(base,windows),release);
test('Windows distribution is isolated, unsigned NSIS, with no upstream updater',()=>{
  assert.equal(config.identifier,'local.meetily.desktop');
  assert.notEqual(config.identifier,base.identifier);
  assert.equal(config.productName,'Meetily Local');
  assert.deepEqual(config.bundle.targets,['nsis']);
  assert.equal(config.bundle.createUpdaterArtifacts,false);
  assert.deepEqual(config.plugins.updater.endpoints,[]);
  assert.equal(config.bundle.windows.signCommand,null);
  assert.equal(config.bundle.windows.nsis.installMode,'currentUser');
});
test('installer retains helper/templates and colocates complete worker/runtime resources',()=>{
  assert.deepEqual(config.bundle.externalBin,base.bundle.externalBin);
  assert.equal(config.bundle.resources['templates'],'templates');
  assert.equal(config.bundle.resources['binaries/qwen'],'qwen');
  assert.equal(config.bundle.resources['binaries/onnxruntime'],'.');
  assert.equal(config.bundle.resources['binaries/windows-crt'],'.');
  assert.equal(config.bundle.resources['binaries/windows-notices'],'third-party');
});
test('model remains external, pinned and checksummed, never part of bundle resources',()=>{
  const manifest=json('../../../nix/qwen-model.json');
  assert.equal(manifest.id,'Qwen/Qwen3-ASR-1.7B');
  assert.equal(manifest.revision,'7278e1e70fe206f11671096ffdd38061171dd6e5');
  for(const row of manifest.files){assert.match(row.sha256,/^[a-f0-9]{64}$/);assert.ok(row.bytes>0);assert.ok(!row.file.includes('/'));}
  assert.ok(!Object.keys(config.bundle.resources).some(p=>p.includes('model-1.7B')));
});
test('build pins native tools and includes app-local CRT and OpenMP',()=>{
  const script=text('../../scripts/build-windows-installer.ps1');
  for(const sha of ['94af030060d88cc17e9f00ef1663ebdc1126b35e16bebdfa1e807984b70abd8f','8b04387766efc05c627e26d24797ec0d4ed4c105ec14fa7400aa84a02db22b66','174c616efc0271194488642a72f1a514e01487da4dfe84c49296d66e40ebe0da','e2aaeaa0fdbc397d4794828086424d4aaa2102cef1fb6874f6ffd29c0b88b673']) assert.ok(script.includes(sha));
  assert.ok(script.includes('Microsoft.VC143.CRT'));
  assert.ok(script.includes('Microsoft.VC143.OpenMP'));
  assert.ok(script.includes("'--locked'"));
  assert.ok(script.includes("'--frozen-lockfile'"));
  assert.ok(script.includes("'--no-sign'"));
  assert.ok(script.includes("$installers[0].Name.Replace(' ','.')"));
  const assets=script.indexOf("Run 'pnpm.cmd' @('build')");
  assert.ok(assets>=0 && assets<script.indexOf("'--example','qwen_protocol_fixture'"));
});
test('Windows packaging gates selected-folder and writable-handle save regressions',()=>{
  const script=text('../../scripts/build-windows-installer.ps1');
  for(const filter of ['save_folder_tests','audio::recording_saver','audio::checkpoint_audio','audio::incremental_saver','audio::import','audio::audio_processing::resample_integrity_tests','audio::decoder']) {
    assert.ok(script.includes(`'${filter}'`),`Missing Windows native regression: ${filter}`);
  }
  assert.ok(script.includes("$env:Path=(Join-Path $ffmpegRoot 'bin')+';'+$env:Path"));
  assert.ok(script.includes("'--lib',$filter,'--','--test-threads=1'"));
  assert.ok(script.indexOf("'audio::recording_saver'")<script.indexOf("'exec','tauri','build'"));
});
test('release automation is Windows-tag-only, fork-only and never latest/stable',()=>{
  const workflow=text('../../../.github/workflows/windows-prerelease.yml');
  assert.ok(workflow.includes("- 'windows-v*'"));
  assert.ok(workflow.includes('workflow_dispatch:'));
  assert.ok(!workflow.includes('pull_request:'));
  assert.ok(workflow.includes("github.event_name == 'push' && startsWith(github.ref, 'refs/tags/windows-v')"));
  assert.ok(workflow.includes("github.repository != 'Zackriya-Solutions/meeting-minutes'"));
  assert.ok(workflow.includes('--verify-tag --prerelease --latest=false'));
  assert.ok(workflow.includes('Refusing to overwrite a stable/draft release'));
  assert.ok(workflow.includes('contents: read'));
});
test('CI action revisions are full commit pins and installation gates publication',()=>{
  const workflow=text('../../../.github/workflows/windows-prerelease.yml');
  const actions=[...workflow.matchAll(/uses:\s*(actions\/[\w-]+)@(\S+)/g)];
  assert.equal(actions.length,5);
  for(const action of actions) assert.match(action[2],/^[a-f0-9]{40}$/);
  assert.ok(workflow.includes('needs: build'));
  assert.ok(workflow.includes("smoke['stage']=='complete' and smoke['keptInstalled'] is False"));
  assert.ok(workflow.includes('test-windows-installer.ps1'));
});
test('clean worker startup fails early and failed CI diagnostics cannot bypass publication gates',()=>{
  const build=text('../../scripts/build-windows-installer.ps1');
  const probe=text('../../scripts/test-windows-qwen-runtime.ps1');
  const workflow=text('../../../.github/workflows/windows-prerelease.yml');
  assert.ok(build.indexOf("'test-windows-qwen-runtime.ps1'")<build.indexOf("'--release','-p','llama-helper'"));
  assert.ok(probe.includes(String.raw`$si.EnvironmentVariables['Path']=$env:SystemRoot+'\System32;'+$env:SystemRoot`));
  assert.ok(probe.includes("$ready.threads -ne 6"));
  const worker=text('../../../benchmarks/qwen3-asr/worker/src/main.rs');
  assert.ok(worker.includes('openblas_set_num_threads(threads as std::ffi::c_int)'));
  assert.ok(worker.includes('if actual != threads as i32'));
  assert.ok(worker.includes('fn explicit_blas_budget_overrides_startup_count()'));
  assert.ok(probe.includes('exitHex') && probe.includes('$r.stderr=') && probe.includes('/dependents'));
  assert.ok(workflow.includes('if: failure()'));
  assert.ok(workflow.includes('name: UNVERIFIED-windows-failure-diagnostics'));
  assert.ok(workflow.includes('needs: build'));
  assert.ok(!workflow.includes('continue-on-error:'));
});
test('explicit Qwen selection persists only after the worker handshake succeeds',()=>{
  const ui=text('../../src/components/QwenModelSettings.tsx');
  const handshake=ui.indexOf("invoke('qwen_validate_settings'");
  const persist=ui.indexOf("invoke('api_save_transcript_config'");
  const select=ui.indexOf('onSelect(model);');
  assert.ok(handshake>=0 && persist>handshake && select>persist);
});
test('extra model initializer stays opt-in, preserving the default Linux worker bin set',()=>{
  const manifest=text('../../../benchmarks/qwen3-asr/worker/Cargo.toml');
  assert.ok(manifest.includes('default-run = "qwen-speech-worker"'));
  assert.ok(manifest.includes('required-features = ["model-preparation"]'));
});
test('model preparation is opt-in, exclusive, rejects mismatches and uses no cloud inference',()=>{
  const script=text('../../scripts/prepare-qwen-model.ps1');
  assert.ok(script.includes('if(!$Download)'));
  assert.ok(script.includes('[IO.FileShare]::None'));
  assert.ok(script.includes('Existing model file differs; not overwritten'));
  assert.ok(script.includes('qwen-model-init.exe'));
  assert.ok(!script.includes('Invoke-RestMethod'));
  assert.ok(!script.includes('api.openai.com'));
});
