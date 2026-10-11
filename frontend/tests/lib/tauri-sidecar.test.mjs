import assert from 'node:assert/strict';
import test from 'node:test';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { fileURLToPath } from 'node:url';

const script = fileURLToPath(new URL('../../scripts/tauri-auto.js', import.meta.url));
function run(command, feature, host, extraEnv = {}) {
  const calls = [];
  const childProcess = {
    execSync: (...args) => { calls.push(['exec', ...args]); return ''; },
    execFileSync: (exe, args, options) => {
      calls.push(['file', exe, args, options]);
      if (exe === 'rustc') return `host: ${host}\n`;
      return '';
    },
  };
  vm.runInNewContext(fs.readFileSync(script, 'utf8'), {
    require: (name) => ({ child_process: childProcess, path, fs: {
      mkdirSync: (...args) => calls.push(['mkdir', ...args]),
      copyFileSync: (...args) => calls.push(['copy', ...args]),
    }, os: { platform: () => host.includes('windows') ? 'win32' : 'linux' } })[name],
    __dirname: path.dirname(script),
    process: { argv: ['node', script, command], env: { TAURI_GPU_FEATURE: feature, ...extraEnv },
      exit: (code) => { throw new Error(`exit ${code}`); } },
    console: { log() {}, error() {} },
  });
  return calls;
}

test('clean Linux dev prepares debug sidecar before Tauri starts', () => {
  const calls = run('dev', 'none', 'x86_64-unknown-linux-gnu');
  const build = calls.find((c) => c[0] === 'file' && c[1] === 'cargo');
  assert.deepEqual(Array.from(build[2]), ['build', '-p', 'llama-helper']);
  const copy = calls.find((c) => c[0] === 'copy');
  assert.ok(copy[1].endsWith('/target/debug/llama-helper'));
  assert.ok(copy[2].endsWith('/binaries/llama-helper-x86_64-unknown-linux-gnu'));
  assert.ok(calls.indexOf(copy) < calls.findIndex((c) => c[0] === 'exec' && c[1].startsWith('tauri dev')));
});
test('Windows CUDA release uses GPU feature and .exe target suffix', () => {
  const calls = run('build', 'cuda', 'x86_64-pc-windows-msvc');
  const build = calls.find((c) => c[0] === 'file' && c[1] === 'cargo');
  assert.deepEqual(Array.from(build[2]), ['build', '-p', 'llama-helper', '--release', '--features', 'cuda']);
  const copy = calls.find((c) => c[0] === 'copy');
  assert.ok(copy[1].endsWith('/target/release/llama-helper.exe'));
  assert.ok(copy[2].endsWith('llama-helper-x86_64-pc-windows-msvc.exe'));
});
test('unsupported helper GPU feature falls back to CPU and custom target directory works', () => {
  const calls = run('build', 'hipblas', 'x86_64-unknown-linux-gnu', {
    CARGO_TARGET_DIR: '/tmp/helper-target', CARGO_BUILD_TARGET: 'aarch64-unknown-linux-gnu',
  });
  const build = calls.find((c) => c[0] === 'file' && c[1] === 'cargo');
  assert.deepEqual(Array.from(build[2]), ['build', '-p', 'llama-helper', '--release', '--target', 'aarch64-unknown-linux-gnu']);
  assert.equal(calls.find((c) => c[0] === 'copy')[1], '/tmp/helper-target/aarch64-unknown-linux-gnu/release/llama-helper');
});
