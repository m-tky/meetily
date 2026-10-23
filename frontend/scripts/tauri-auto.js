#!/usr/bin/env node
/**
 * Run the Qwen-only desktop app and prepare the local summary sidecar.
 */

const { execSync, execFileSync } = require('child_process');
const path = require('path');
const fs = require('fs');
const os = require('os');

// Get the command (dev or build)
const command = process.argv[2];
if (!command || !['dev', 'build'].includes(command)) {
  console.error('Usage: node tauri-auto.js [dev|build]');
  process.exit(1);
}

const env = { ...process.env };

// Build the tauri command. For `dev`, layer tauri.dev.conf.json on top of
// tauri.conf.json so the dev build gets its own identifier (and AppData
// folder) and won't collide with an installed release build.
let tauriCmd = `tauri ${command}`;
if (command === 'dev') {
  tauriCmd += ' -c src-tauri/tauri.dev.conf.json';
}
console.log(`Running: tauri ${command} (local Qwen ASR worker)`);
console.log('');

// Tauri validates every externalBin even in dev/check builds. The documented
// quick-start previously failed on a clean checkout because this sidecar was absent.
try {
  const root = path.resolve(__dirname, '../..');
  const host = execFileSync('rustc', ['-vV'], { encoding: 'utf8', env })
    .match(/^host: (.+)$/m)?.[1];
  if (!host) throw new Error('Cannot determine Rust host target');
  const target = env.CARGO_BUILD_TARGET || host;
  const release = command === 'build';
  const args = ['build', '-p', 'llama-helper'];
  if (release) args.push('--release');
  if (env.CARGO_BUILD_TARGET) args.push('--target', target);
  // Summary helper acceleration is independent of the retired ASR features.
  if (['cuda', 'metal', 'vulkan'].includes(env.TAURI_GPU_FEATURE)) args.push('--features', env.TAURI_GPU_FEATURE);
  execFileSync('cargo', args, { cwd: root, stdio: 'inherit', env });
  const targetDir = env.CARGO_TARGET_DIR
    ? path.resolve(root, env.CARGO_TARGET_DIR) : path.join(root, 'target');
  const suffix = target.includes('windows') ? '.exe' : '';
  const binary = path.join(targetDir, ...(env.CARGO_BUILD_TARGET ? [target] : []),
    release ? 'release' : 'debug', `llama-helper${suffix}`);
  const destination = path.join(root, 'frontend/src-tauri/binaries', `llama-helper-${target}${suffix}`);
  fs.mkdirSync(path.dirname(destination), { recursive: true });
  fs.copyFileSync(binary, destination);
  console.log(`Staged local LLM sidecar: ${destination}`);
  execSync(tauriCmd, { stdio: 'inherit', env });
} catch (err) {
  process.exit(err.status || 1);
}
