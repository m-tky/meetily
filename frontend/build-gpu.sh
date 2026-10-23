#!/usr/bin/env bash
# Compatibility launcher: ASR now uses the Qwen CPU worker, not app GPU features.
set -euo pipefail
cd "$(dirname "$0")"
if [ "$#" -ne 0 ]; then
  echo 'Retired ASR GPU flags are not supported. Use tauri-auto.js build.' >&2
  exit 2
fi
echo 'ASR is Qwen-only. For packaged setup use the Nix package or build-windows-installer.ps1.'
exec node scripts/tauri-auto.js build
