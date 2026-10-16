import assert from 'node:assert/strict';
import test from 'node:test';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { fileURLToPath } from 'node:url';

const script = fileURLToPath(new URL('../../scripts/tauri-auto.js', import.meta.url));
function run(command, feature, host, extraEnv = {}) {
  // Model the selected host's path semantics, not the machine running Node.
  const hostPath = host.includes('windows') ? path.win32 : path.posix;
  const scriptDir = host.includes('windows') ? 'C:\\repo\\frontend\\scripts' : '/repo/frontend/scripts';
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
    require: (name) => ({ child_process: childProcess, path: hostPath, fs: {
      mkdirSync: (...args) => calls.push(['mkdir', ...args]),
      copyFileSync: (...args) => calls.push(['copy', ...args]),
    }, os: { platform: () => host.includes('windows') ? 'win32' : 'linux' } })[name],
    __dirname: scriptDir,
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
  assert.equal(copy[1], '/repo/target/debug/llama-helper');
  assert.equal(copy[2], '/repo/frontend/src-tauri/binaries/llama-helper-x86_64-unknown-linux-gnu');
  assert.ok(calls.indexOf(copy) < calls.findIndex((c) => c[0] === 'exec' && c[1].startsWith('tauri dev')));
});
test('Windows CUDA release uses GPU feature and .exe target suffix', () => {
  const calls = run('build', 'cuda', 'x86_64-pc-windows-msvc');
  const build = calls.find((c) => c[0] === 'file' && c[1] === 'cargo');
  assert.deepEqual(Array.from(build[2]), ['build', '-p', 'llama-helper', '--release', '--features', 'cuda']);
  const copy = calls.find((c) => c[0] === 'copy');
  assert.equal(copy[1], 'C:\\repo\\target\\release\\llama-helper.exe');
  assert.equal(copy[2], 'C:\\repo\\frontend\\src-tauri\\binaries\\llama-helper-x86_64-pc-windows-msvc.exe');
});
test('unsupported helper GPU feature falls back to CPU and custom target directory works', () => {
  const calls = run('build', 'hipblas', 'x86_64-unknown-linux-gnu', {
    CARGO_TARGET_DIR: '/tmp/helper-target', CARGO_BUILD_TARGET: 'aarch64-unknown-linux-gnu',
  });
  const build = calls.find((c) => c[0] === 'file' && c[1] === 'cargo');
  assert.deepEqual(Array.from(build[2]), ['build', '-p', 'llama-helper', '--release', '--target', 'aarch64-unknown-linux-gnu']);
  assert.equal(calls.find((c) => c[0] === 'copy')[1], '/tmp/helper-target/aarch64-unknown-linux-gnu/release/llama-helper');
});
test('Windows CPU dev stages an exe before starting Tauri', () => {
  const calls = run('dev', 'none', 'x86_64-pc-windows-msvc');
  const build = calls.find((c) => c[0] === 'file' && c[1] === 'cargo');
  assert.deepEqual(Array.from(build[2]), ['build', '-p', 'llama-helper']);
  const copy = calls.find((c) => c[0] === 'copy');
  assert.equal(copy[1], 'C:\\repo\\target\\debug\\llama-helper.exe');
  assert.equal(copy[2], 'C:\\repo\\frontend\\src-tauri\\binaries\\llama-helper-x86_64-pc-windows-msvc.exe');
  assert.ok(calls.indexOf(copy) < calls.findIndex((c) => c[0] === 'exec' && c[1].startsWith('tauri dev')));
});
test('Windows explicit target and custom directory retain Windows path semantics', () => {
  const calls = run('build', 'none', 'x86_64-pc-windows-msvc', {
    CARGO_TARGET_DIR: 'D:\\build cache', CARGO_BUILD_TARGET: 'x86_64-pc-windows-msvc',
  });
  const build = calls.find((c) => c[0] === 'file' && c[1] === 'cargo');
  assert.deepEqual(Array.from(build[2]), ['build', '-p', 'llama-helper', '--release', '--target', 'x86_64-pc-windows-msvc']);
  assert.equal(calls.find((c) => c[0] === 'copy')[1], 'D:\\build cache\\x86_64-pc-windows-msvc\\release\\llama-helper.exe');
});
