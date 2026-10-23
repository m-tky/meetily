import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync,existsSync} from 'node:fs';
const read=path=>readFileSync(new URL(path,import.meta.url),'utf8');
test('application has no retired engine modules, IPC handlers or build dependencies',()=>{
  const entry=read('../../src-tauri/src/lib.rs'),engine=read('../../src-tauri/src/audio/transcription/engine.rs');
  assert.doesNotMatch(entry,/whisper_engine|parakeet_engine/);
  assert.doesNotMatch(engine,/Whisper|Parakeet|fallback_model/);
  assert.doesNotMatch(read('../../src-tauri/Cargo.toml'),/whisper-rs|esaxx-rs/);
  assert.doesNotMatch(read('../../../Cargo.lock'),/name = "whisper-rs(?:-sys)?"|name = "esaxx-rs"/);
  for(const name of ['whisper_engine','parakeet_engine'])assert.equal(existsSync(new URL(`../../src-tauri/src/${name}`,import.meta.url)),false);
});
test('live, import and retranscription resolve the shared Qwen adapter without original fallback',()=>{
  for(const path of ['../../src-tauri/src/audio/import.rs','../../src-tauri/src/audio/retranscription.rs']){
    const source=read(path);assert.match(source,/BatchSession::open/);assert.match(source,/session\s*\.transcribe/);assert.doesNotMatch(source,/use_parakeet|get_or_init_whisper|get_or_init_parakeet|transcribe_audio_with_confidence/);
  }
  const batch=read('../../src-tauri/src/audio/transcription/batch.rs');
  assert.match(batch,/QwenProvider::new/);assert.match(batch,/qwen_model::validate/);assert.match(batch,/compare_exchange/);assert.match(batch,/tokio::select!/);
  assert.match(read('../../src-tauri/src/audio/transcription/engine.rs'),/initialize_model/);
});
test('both setup manifests are independently pinned; no arbitrary download sources',()=>{
  const big=JSON.parse(read('../../../nix/qwen-model.json')),small=JSON.parse(read('../../../nix/qwen-model-0.6b.json'));
  assert.equal(big.id,'Qwen/Qwen3-ASR-1.7B');assert.equal(small.id,'Qwen/Qwen3-ASR-0.6B');
  assert.equal(small.revision,'5eb144179a02acc5e5ba31e748d22b0cf3e303b0');
  assert.equal(small.files.find(f=>f.file==='model.safetensors').sha256,'79d6cbd4c98c7bbffe9db2edac07f56cd6637d0d5944b27f6c2b8353840323ea');
  assert.ok(small.files.every(f=>f.bytes>0&&/^[a-f0-9]{64}$/.test(f.sha256)));
});
