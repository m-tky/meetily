<# Build the unsigned Windows x64 CPU distribution. No model/audio/auth access.
   Run on an NTFS workspace with Node, pnpm10, Rust/MSVC and VS C++ installed.
   Tool archives are pinned and hash-verified; no machine PATH/locale changes. #>
[CmdletBinding()]
param(
 [string]$CacheDirectory=(Join-Path $env:LOCALAPPDATA 'MeetilyWindowsBuildCache'),
 [string]$NativeTargetDirectory='',
 [string]$WorkerTargetDirectory='',
 [string]$LlvmRoot='',
 [string]$OutputDirectory='',
 [switch]$SkipInstall,
 [switch]$SkipTests
)
$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$OutputEncoding=[Console]::OutputEncoding
if([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT){throw 'Windows x64/MSVC is required'}
$repo=(Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if(!$NativeTargetDirectory){$NativeTargetDirectory=Join-Path $repo 'target'}
if(!$WorkerTargetDirectory){$WorkerTargetDirectory=Join-Path $CacheDirectory 'worker-target'}
if(!$OutputDirectory){$OutputDirectory=Join-Path $repo 'dist\windows'}
foreach($dir in @($CacheDirectory,$OutputDirectory,$NativeTargetDirectory,$WorkerTargetDirectory)){
 [void][IO.Directory]::CreateDirectory($dir)
 if((Get-Item -LiteralPath $dir).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Build directories must not be links/reparse points'}
}
function Run([string]$tool,[string[]]$arguments){
 $old=$ErrorActionPreference
 try{$ErrorActionPreference='Continue';& $tool @arguments;$code=$LASTEXITCODE}finally{$ErrorActionPreference=$old}
 if($code -ne 0){throw "Build command failed: $tool (exit $code)"}
}
function Verify([string]$path,[string]$sha){
 if(!(Test-Path -LiteralPath $path -PathType Leaf) -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $sha){throw ('Checksum mismatch: '+[IO.Path]::GetFileName($path))}
}
function Download([string]$name,[string]$url,[string]$sha){
 $path=Join-Path $CacheDirectory $name
 if(!(Test-Path -LiteralPath $path)){
  $tmp=$path+'.download-'+[Guid]::NewGuid().ToString('N')
  Write-Host ('Downloading verified tool: '+$name)
  try{Run 'curl.exe' @('--fail','--location','--silent','--show-error','--retry','3','--connect-timeout','30','--max-time','900','--proto','=https','--proto-redir','=https','--output',$tmp,$url);Verify $tmp $sha;Move-Item -LiteralPath $tmp -Destination $path}finally{if(Test-Path -LiteralPath $tmp){Remove-Item -LiteralPath $tmp}}
 }
 Verify $path $sha
 return $path
}
function Unzip([string]$archive,[string]$destination){
 if(!(Test-Path -LiteralPath $destination)){Expand-Archive -LiteralPath $archive -DestinationPath $destination}
 if((Get-Item -LiteralPath $destination).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Archive destination is a reparse point'}
}
$vswhere=Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$vs=& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if($LASTEXITCODE -ne 0 -or !$vs){throw 'VS2022 x64 C++ build tools are required'}
$vc=Join-Path $vs 'Common7\Tools\VsDevCmd.bat'
$vars=& cmd.exe /d /s /c "`"$vc`" -no_logo -arch=x64 -host_arch=x64 && set"
if($LASTEXITCODE -ne 0){throw 'VS developer environment failed'}
foreach($line in $vars){if($line -match '^([^=]+)=(.*)$'){[Environment]::SetEnvironmentVariable($matches[1],$matches[2],'Process')}}
# Some VS installations have C++ but not CMake; another installed VS may supply it.
if(!(Get-Command cmake.exe -ErrorAction SilentlyContinue)){
 foreach($installation in @(& $vswhere -products '*' -property installationPath)){
  $cmakeBin=Join-Path $installation 'Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin'
  if(Test-Path (Join-Path $cmakeBin 'cmake.exe')){$env:Path=$cmakeBin+';'+$env:Path;break}
 }
}
if(!(Get-Command cmake.exe -ErrorAction SilentlyContinue)){throw 'CMake is required (VS C++ CMake tools or system CMake)'}
Run 'cmake.exe' @('--version')
if(!$LlvmRoot){
 $installer=Download 'LLVM-18.1.8-win64.exe' 'https://github.com/llvm/llvm-project/releases/download/llvmorg-18.1.8/LLVM-18.1.8-win64.exe' '94af030060d88cc17e9f00ef1663ebdc1126b35e16bebdfa1e807984b70abd8f'
 $LlvmRoot=Join-Path $CacheDirectory 'llvm18'
 if(!(Test-Path (Join-Path $LlvmRoot 'bin\libclang.dll'))){
  $seven=Join-Path $env:ProgramFiles '7-Zip\7z.exe'
  Run $seven @('x',$installer,('-o'+$LlvmRoot),'-y','bin\libclang.dll','bin\clang.exe','lib\clang\18\include\*')
 }
}
$env:LIBCLANG_PATH=Join-Path $LlvmRoot 'bin';$env:CLANG_PATH=Join-Path $env:LIBCLANG_PATH 'clang.exe'
$env:BINDGEN_EXTRA_CLANG_ARGS='-resource-dir="'+((Join-Path $LlvmRoot 'lib\clang\18') -replace '\\','/')+'"'
if(!(Test-Path (Join-Path $env:LIBCLANG_PATH 'libclang.dll'))){throw 'Pinned LLVM18 libclang is missing'}
$env:CL=($env:CL+' /utf-8').Trim();$env:CARGO_BUILD_JOBS='2';$env:CARGO_INCREMENTAL='0'
$env:CARGO_PROFILE_RELEASE_DEBUG='0';$env:CARGO_PROFILE_DEV_DEBUG='0';$env:CARGO_PROFILE_TEST_DEBUG='0'
$env:MEETILY_DISABLE_ANALYTICS='1';$env:NEXT_TELEMETRY_DISABLED='1'
$env:OPENBLAS_NUM_THREADS='6';$env:OMP_NUM_THREADS='6';$env:MKL_NUM_THREADS='6'
$blasZip=Download 'OpenBLAS-0.3.30-x64.zip' 'https://github.com/OpenMathLib/OpenBLAS/releases/download/v0.3.30/OpenBLAS-0.3.30-x64.zip' '8b04387766efc05c627e26d24797ec0d4ed4c105ec14fa7400aa84a02db22b66'
$blas=Join-Path $CacheDirectory 'openblas-0.3.30-lp64';Unzip $blasZip $blas
Verify (Join-Path $blas 'bin\libopenblas.dll') 'e824cf9fc22e5949807ce995a32e413a345fb97e63e6d10cfbf41aa86382193c'
$blasLicense=Download 'OpenBLAS-LICENSE.txt' 'https://raw.githubusercontent.com/OpenMathLib/OpenBLAS/v0.3.30/LICENSE' '190b5a9c8d9723fe958ad33916bd7346d96fab3c5ea90832bb02d854f620fcff'
$env:QWEN_OPENBLAS_LIB_DIR=Join-Path $blas 'lib';$env:Path=(Join-Path $blas 'bin')+';'+$env:Path
$ortZip=Download 'onnxruntime-win-x64-1.22.0.zip' 'https://github.com/microsoft/onnxruntime/releases/download/v1.22.0/onnxruntime-win-x64-1.22.0.zip' '174c616efc0271194488642a72f1a514e01487da4dfe84c49296d66e40ebe0da'
$ortExtract=Join-Path $CacheDirectory 'ort-1.22.0';Unzip $ortZip $ortExtract
$bin=Join-Path $repo 'frontend\src-tauri\binaries';$ort=Join-Path $bin 'onnxruntime';$qwen=Join-Path $bin 'qwen';$notices=Join-Path $bin 'windows-notices';$crtStage=Join-Path $bin 'windows-crt'
foreach($dir in @($ort,$qwen,$notices,$crtStage)){
 [void][IO.Directory]::CreateDirectory($dir)
 if((Get-Item -LiteralPath $dir).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Runtime stage must not be a reparse point'}
}
# App-local redistributable CRTs: do not rely on a machine's VS installation.
$crtCandidates=@(Get-Item -Path (Join-Path $vs 'VC\Redist\MSVC\*\x64\Microsoft.VC143.CRT') -ErrorAction SilentlyContinue | Sort-Object FullName -Descending)
if(!$crtCandidates.Count){throw 'VS2022 x64 CRT redistributable directory is missing'}
$crt=$crtCandidates[0].FullName
foreach($name in @('vcruntime140.dll','msvcp140.dll')){if(!(Test-Path (Join-Path $crt $name))){throw 'Incomplete VC143 CRT redistributables'}}
$openmp=Join-Path (Split-Path $crt) 'Microsoft.VC143.OpenMP'
if(!(Test-Path (Join-Path $openmp 'vcomp140.dll'))){throw 'VC143 OpenMP redistributable is missing'}
foreach($directory in @($crt,$openmp)){
 foreach($dll in Get-ChildItem -LiteralPath $directory -Filter '*.dll' -File){Copy-Item $dll.FullName $crtStage -Force;Copy-Item $dll.FullName $qwen -Force}
}
$ortFiles=@(
 @('lib\onnxruntime.dll','onnxruntime.dll','579b636403983254346a5c1d80bd28f1519cd1e284cd204f8d4ff41f8d711559'),
 @('lib\onnxruntime_providers_shared.dll','onnxruntime_providers_shared.dll','ba00ea1ef846c9b909c7854bc56c51051a20f9773b3e1153dda118d4b85d0b93'),
 @('LICENSE','onnxruntime-LICENSE.txt','c250d6278f0b47a6439fb7592b08b58a55eb9f535aa49a1db63211c3f982b674'))
foreach($entry in $ortFiles){
 $file=Join-Path $ortExtract ('onnxruntime-win-x64-1.22.0\'+$entry[0]);Verify $file $entry[2]
 Copy-Item -LiteralPath $file -Destination (Join-Path $ort $entry[1]) -Force
 Copy-Item -LiteralPath $file -Destination (Join-Path $qwen $entry[1]) -Force
}
$ffmpegZip=Download 'ffmpeg-8.0.1-essentials_build.zip' 'https://github.com/Zackriya-Solutions/ffmpeg-binaries/releases/download/0.0.1/ffmpeg-8.0.1-essentials_build.zip' 'e2aaeaa0fdbc397d4794828086424d4aaa2102cef1fb6874f6ffd29c0b88b673'
$ffmpeg=Join-Path $CacheDirectory 'ffmpeg-8.0.1';Unzip $ffmpegZip $ffmpeg
$ffmpegRoot=Join-Path $ffmpeg 'ffmpeg-8.0.1-essentials_build'
Copy-Item (Join-Path $ffmpegRoot 'bin\ffmpeg.exe') (Join-Path $bin 'ffmpeg-x86_64-pc-windows-msvc.exe') -Force
Copy-Item (Join-Path $ffmpegRoot 'LICENSE') (Join-Path $notices 'FFmpeg-GPLv3.txt') -Force
Copy-Item (Join-Path $ffmpegRoot 'README.txt') (Join-Path $notices 'FFmpeg-build-and-source.txt') -Force
Copy-Item (Join-Path $repo 'LICENSE.md') (Join-Path $notices 'Meetily-LICENSE.md') -Force
Copy-Item (Join-Path $repo 'vendor\qwen-asr\LICENSE') (Join-Path $qwen 'qwen-asr-LICENSE.txt') -Force
Copy-Item $blasLicense (Join-Path $qwen 'OpenBLAS-LICENSE.txt') -Force
Copy-Item (Join-Path $blas 'bin\libopenblas.dll') $qwen -Force
Copy-Item (Join-Path $repo 'nix\qwen-model.json') $qwen -Force
Copy-Item (Join-Path $PSScriptRoot 'prepare-qwen-model.ps1') $qwen -Force
Copy-Item (Join-Path $repo 'docs\meeting-assistant\WINDOWS-INSTALLER.md') (Join-Path $qwen 'SETUP.md') -Force
$env:CARGO_TARGET_DIR=$WorkerTargetDirectory
Push-Location (Join-Path $repo 'benchmarks\qwen3-asr\worker')
try{
 Run 'cargo.exe' @('build','--locked','--release','--features','model-preparation','--bin','qwen-speech-worker','--bin','qwen-model-init')
 if(!$SkipTests){Run 'cargo.exe' @('test','--locked','--release','--features','model-preparation','--lib','--bin','qwen-speech-worker')}
}finally{Pop-Location}
foreach($exe in @('qwen-speech-worker.exe','qwen-model-init.exe')){Copy-Item (Join-Path $WorkerTargetDirectory ('release\'+$exe)) $qwen -Force}
# Fail before the expensive app build if the staged worker cannot start without VS/PATH.
& (Join-Path $PSScriptRoot 'test-windows-qwen-runtime.ps1') -RuntimeDirectory $qwen -ReportPath (Join-Path $OutputDirectory 'qwen-runtime-startup.json')
$env:CARGO_TARGET_DIR=$NativeTargetDirectory
Push-Location $repo
try{Run 'cargo.exe' @('build','--locked','--release','-p','llama-helper')}finally{Pop-Location}
Copy-Item (Join-Path $NativeTargetDirectory 'release\llama-helper.exe') (Join-Path $bin 'llama-helper-x86_64-pc-windows-msvc.exe') -Force
# The overlay affects only this distribution, not the Linux/default identifier.
$env:TAURI_CONFIG=[IO.File]::ReadAllText((Join-Path $repo 'frontend\src-tauri\tauri.windows-release.conf.json'))
Push-Location (Join-Path $repo 'frontend')
try{
 if(!$SkipInstall){Run 'pnpm.cmd' @('install','--frozen-lockfile')}
 Run 'pnpm.cmd' @('exec','tsc','--noEmit')
 Run 'node.exe' @('--test','tests/lib/tauri-sidecar.test.mjs','tests/lib/windows-distribution.test.mjs')
 if(!$SkipTests){
  # A clean checkout needs frontendDist before the custom-protocol fixture build.
  Run 'pnpm.cmd' @('build')
  # Reuse the optimized dependency graph; a second debug graph costs GiBs.
  Run 'cargo.exe' @('build','--locked','--release','-p','meetily','--features','tauri/custom-protocol','--example','qwen_protocol_fixture')
  $env:MEETILY_QWEN_TEST_WORKER=Join-Path $NativeTargetDirectory 'release\examples\qwen_protocol_fixture.exe'
  Run 'cargo.exe' @('test','--locked','--release','-p','meetily','--features','tauri/custom-protocol','--lib','qwen_provider::tests','--','--include-ignored')
 }
 Run 'pnpm.cmd' @('exec','tauri','build','--ci','--no-sign','--config','src-tauri/tauri.windows-release.conf.json','--bundles','nsis','--','--locked')
}finally{Pop-Location}
$version=([IO.File]::ReadAllText((Join-Path $repo 'frontend\src-tauri\tauri.conf.json'))|ConvertFrom-Json).version
$installers=@(Get-ChildItem -LiteralPath (Join-Path $NativeTargetDirectory 'release\bundle\nsis') -Filter ('Meetily Local_'+$version+'_*setup.exe') -File)
if($installers.Count -ne 1){throw 'Expected exactly one current-version NSIS installer'}
$file=Join-Path $OutputDirectory $installers[0].Name;Copy-Item $installers[0].FullName $file -Force
$hash=(Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText(($file+'.sha256'),($hash+'  '+[IO.Path]::GetFileName($file)+"`n"))
$manifest=[ordered]@{version=$version;target='x86_64-pc-windows-msvc';profile='release';identifier='local.meetily.desktop';signed=$false;updater=$false;modelBundled=$false;installer=@{name=[IO.Path]::GetFileName($file);sha256=$hash;bytes=(Get-Item $file).Length};qwen=@{threads=6;precision='decoder-INT8-encoder-prefill-F32';workerSha256=(Get-FileHash (Join-Path $qwen 'qwen-speech-worker.exe')).Hash.ToLowerInvariant()}}
[IO.File]::WriteAllText((Join-Path $OutputDirectory 'windows-build.json'),($manifest|ConvertTo-Json -Depth 8))
Write-Host ('Installer: '+$file)
