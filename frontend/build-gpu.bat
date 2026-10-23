@echo off
rem Compatibility launcher. Qwen ASR runs in the local worker, not app GPU features.
cd /d "%~dp0"
if not "%~1"=="" (
  echo Retired ASR GPU flags are not supported. Use tauri-auto.js build.
  exit /b 2
)
node scripts\tauri-auto.js build
exit /b %ERRORLEVEL%
