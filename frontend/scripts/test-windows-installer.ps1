<# Isolated per-user NSIS/layout/installed-worker smoke, synthetic silence only.
   Refuses existing installs/profiles/running Meetily. Never reads real audio/auth. #>
[CmdletBinding()]
param(
 [Parameter(Mandatory=$true)][string]$Installer,
 [string]$InstallationDirectory=(Join-Path $env:TEMP ('MeetilyInstallerSmoke-'+[Guid]::NewGuid().ToString('N'))),
 [string]$ReportPath='',
 [switch]$KeepInstalled
)
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false);$OutputEncoding=[Console]::OutputEncoding
$Installer=[IO.Path]::GetFullPath($Installer)
if(!$ReportPath){$ReportPath=Join-Path (Split-Path $Installer) 'installer-smoke.json'}
$InstallationDirectory=[IO.Path]::GetFullPath($InstallationDirectory)
if(Test-Path -LiteralPath $InstallationDirectory){throw 'Refusing an existing installation directory'}
if(Test-Path (Join-Path $env:APPDATA 'local.meetily.desktop')){throw 'Refusing an existing Meetily Local application profile'}
if(Get-Process -Name meetily -ErrorAction SilentlyContinue){throw 'Close existing Meetily before the isolated installer smoke'}
foreach($key in @('HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*','HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*')){
 if(Get-ItemProperty $key -ErrorAction SilentlyContinue | Where-Object DisplayName -eq 'Meetily Local'){throw 'An existing Meetily Local installation was found; not replaced'}
}
$sum=[IO.File]::ReadAllText(($Installer+'.sha256')).Split(' ')[0]
if((Get-FileHash -LiteralPath $Installer).Hash.ToLowerInvariant() -ne $sum){throw 'Installer SHA256 mismatch'}
$report=[ordered]@{stage='installing';error=$null;installationDirectory=$InstallationDirectory;modelBundled=$false;realAudioUsed=$false}
function Publish {[IO.File]::WriteAllText($ReportPath,($report|ConvertTo-Json -Depth 15))}
function Start-Owned([string]$file,[string]$arguments){
 $si=[Diagnostics.ProcessStartInfo]::new($file,$arguments);$si.UseShellExecute=$false;$si.CreateNoWindow=$true
 $p=[Diagnostics.Process]::new();$p.StartInfo=$si;[void]$p.Start();return $p
}
function Start-CleanConsole([string]$file,[string]$arguments){
 $si=[Diagnostics.ProcessStartInfo]::new($file,$arguments);$si.UseShellExecute=$false;$si.CreateNoWindow=$true
 $si.RedirectStandardInput=$true;$si.RedirectStandardOutput=$true;$si.RedirectStandardError=$true
 $si.StandardOutputEncoding=[Text.Encoding]::UTF8;$si.StandardErrorEncoding=[Text.Encoding]::UTF8
 $si.EnvironmentVariables['Path']=$env:SystemRoot+'\System32;'+$env:SystemRoot
 $p=[Diagnostics.Process]::new();$p.StartInfo=$si;[void]$p.Start();return $p
}
function Send($process,$message){
 $bytes=[Text.Encoding]::UTF8.GetBytes(($message|ConvertTo-Json -Compress)+"`n")
 $process.StandardInput.BaseStream.Write($bytes,0,$bytes.Length);$process.StandardInput.BaseStream.Flush()
}
function Receive($process){
 $task=$process.StandardOutput.ReadLineAsync()
 if(!$task.Wait(30000)){throw 'Installed worker response timed out'}
 $line=$task.GetAwaiter().GetResult();if(!$line){throw 'Installed worker exited before a response'}
 return ($line|ConvertFrom-Json)
}
$worker=$null;$stderrTask=$null;$temporary=$null
try{
 Publish
 # NSIS requires /D= last and unquoted, including when the path has spaces.
 $setup=Start-Owned $Installer ('/S /D='+$InstallationDirectory)
 try{if(!$setup.WaitForExit(180000)){$setup.Kill();throw 'Owned NSIS installer timed out'};if($setup.ExitCode -ne 0){throw 'NSIS installation failed'}}finally{$setup.Dispose()}
 foreach($name in @('meetily.exe','onnxruntime.dll','onnxruntime_providers_shared.dll','ffmpeg.exe','llama-helper.exe','vcruntime140.dll','vcomp140.dll','templates','third-party\FFmpeg-GPLv3.txt','third-party\FFmpeg-build-and-source.txt','qwen\qwen-speech-worker.exe','qwen\qwen-model-init.exe','qwen\libopenblas.dll','qwen\onnxruntime.dll','qwen\onnxruntime_providers_shared.dll','qwen\prepare-qwen-model.ps1','qwen\qwen-model.json','qwen\qwen-model-0.6b.json','qwen\OpenBLAS-LICENSE.txt','qwen\qwen-asr-LICENSE.txt')){
  if(!(Test-Path -LiteralPath (Join-Path $InstallationDirectory $name))){throw ('Installed resource missing: '+$name)}
 }
 $metadata=[IO.File]::ReadAllText((Join-Path (Split-Path $Installer) 'windows-build.json'))|ConvertFrom-Json
 if((Get-FileHash (Join-Path $InstallationDirectory 'qwen\qwen-speech-worker.exe')).Hash.ToLowerInvariant() -ne $metadata.qwen.workerSha256){throw 'Installed worker differs from staged release'}
 $report.stage='installed-worker';Publish
 $temporary=Join-Path $InstallationDirectory 'synthetic-smoke.wav'
 $stream=[IO.File]::Open($temporary,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
 $w=[IO.BinaryWriter]::new($stream)
 try{
  $w.Write([Text.Encoding]::ASCII.GetBytes('RIFF'));$w.Write([uint32](50+64000));$w.Write([Text.Encoding]::ASCII.GetBytes('WAVEfmt '));$w.Write([uint32]18)
  $w.Write([uint16]3);$w.Write([uint16]1);$w.Write([uint32]16000);$w.Write([uint32]64000);$w.Write([uint16]4);$w.Write([uint16]32);$w.Write([uint16]0)
  $w.Write([Text.Encoding]::ASCII.GetBytes('fact'));$w.Write([uint32]4);$w.Write([uint32]16000);$w.Write([Text.Encoding]::ASCII.GetBytes('data'));$w.Write([uint32]64000);$w.Write([byte[]]::new(64000))
 }finally{$w.Dispose()}
 $runtime=Join-Path $InstallationDirectory 'qwen'
 $report.modelProbes=@()
 foreach($model in @('Qwen3-ASR-1.7B','Qwen3-ASR-0.6B')){
 $si=[Diagnostics.ProcessStartInfo]::new((Join-Path $runtime 'qwen-speech-worker.exe'),('"'+(Join-Path $InstallationDirectory 'model-not-installed')+'" 6 --model '+$model))
 $si.UseShellExecute=$false;$si.CreateNoWindow=$true;$si.RedirectStandardInput=$true;$si.RedirectStandardOutput=$true;$si.RedirectStandardError=$true
 $si.StandardOutputEncoding=[Text.Encoding]::UTF8;$si.StandardErrorEncoding=[Text.Encoding]::UTF8
 $si.EnvironmentVariables['Path']=$env:SystemRoot+'\System32;'+$env:SystemRoot
 $si.EnvironmentVariables['ORT_DYLIB_PATH']=Join-Path $runtime 'onnxruntime.dll'
 $si.EnvironmentVariables['OPENBLAS_NUM_THREADS']='6';$si.EnvironmentVariables['OMP_NUM_THREADS']='6';$si.EnvironmentVariables['MKL_NUM_THREADS']='6'
 $worker=[Diagnostics.Process]::new();$worker.StartInfo=$si;[void]$worker.Start();$stderrTask=$worker.StandardError.ReadToEndAsync()
 $ready=Receive $worker;if($ready.event -ne 'ready' -or $ready.model -ne $model -or $ready.model_loaded -or $ready.threads -ne 6){throw 'Installed worker handshake failed'}
 Send $worker @{op='transcribe';id='installed-silence';wav=$temporary;language='Japanese'}
 $answer=Receive $worker
 if($answer.status -ne 'no_speech' -or $answer.text -ne '' -or $answer.asr_calls -ne 0 -or $answer.model_loaded){throw 'Installed silence contract failed'}
 $worker.Refresh()
 foreach($dll in @('libopenblas.dll','onnxruntime.dll')){
  $module=@($worker.Modules|Where-Object { $_.ModuleName -ieq $dll })
  if($module.Count -ne 1 -or ![String]::Equals($module[0].FileName,(Join-Path $runtime $dll),[StringComparison]::OrdinalIgnoreCase)){throw ('Runtime DLL was not loaded from the installed directory: '+$dll)}
 }
 Send $worker @{op='shutdown';id='installed-bye'};$bye=Receive $worker
 if($bye.status -ne 'bye' -or !$worker.WaitForExit(10000) -or $worker.ExitCode -ne 0){throw 'Installed worker shutdown failed'}
 $report.workerExitCode=$worker.ExitCode;$report.silence=$answer;$report.dllPathsVerified=$true;$report.developerPathRemoved=$true
 $report.modelProbes+=@{model=$model;silence=$answer;exitCode=$worker.ExitCode;dllPathsVerified=$true}
 $report.workerStderr=$stderrTask.GetAwaiter().GetResult()
 $worker.Dispose();$worker=$null;$stderrTask=$null
 Publish
 }
 Remove-Item -LiteralPath $temporary;$temporary=$null
 # Root tools must work without the developer PATH too (no models/inference).
 $probe=Start-CleanConsole (Join-Path $InstallationDirectory 'llama-helper.exe') ''
 $probeError=$probe.StandardError.ReadToEndAsync()
 try{
  Send $probe @{type='ping'};if((Receive $probe).type -ne 'pong'){throw 'Installed helper ping failed'}
  foreach($module in @($probe.Modules|Where-Object {$_.ModuleName -match '^(?:vcruntime140(?:_1)?|msvcp140|vcomp140)\.dll$'})){
   if(![String]::Equals($module.FileName,(Join-Path $InstallationDirectory $module.ModuleName),[StringComparison]::OrdinalIgnoreCase)){throw ('Helper CRT/OpenMP was not app-local: '+$module.ModuleName)}
  }
  Send $probe @{type='shutdown'};if((Receive $probe).type -ne 'goodbye' -or !$probe.WaitForExit(10000) -or $probe.ExitCode -ne 0){throw 'Installed helper shutdown failed'}
  $report.helperPingShutdown=$true;$report.helperCrtPathsVerified=$true
 }finally{try{if(!$probe.HasExited){$probe.Kill();[void]$probe.WaitForExit(5000)}}finally{$probe.Dispose()}}
 $probe=Start-CleanConsole (Join-Path $InstallationDirectory 'ffmpeg.exe') '-version'
 $probeOutput=$probe.StandardOutput.ReadToEndAsync();$probeError=$probe.StandardError.ReadToEndAsync()
 try{
  if(!$probe.WaitForExit(15000) -or $probe.ExitCode -ne 0 -or $probeOutput.GetAwaiter().GetResult() -notmatch '^ffmpeg version 8\.0\.1'){throw 'Installed FFmpeg version probe failed'}
  $report.ffmpegVersionVerified=$true
 }finally{try{if(!$probe.HasExited){$probe.Kill();[void]$probe.WaitForExit(5000)}}finally{$probe.Dispose()}}
 $report.keptInstalled=[bool]$KeepInstalled
 if(!$KeepInstalled){
  $report.stage='uninstalling';Publish
  $uninstallers=@(Get-ChildItem -LiteralPath $InstallationDirectory -Filter 'uninstall*.exe' -File)
  if($uninstallers.Count -ne 1){throw 'Installed uninstaller not found'}
  $uninstall=Start-Owned $uninstallers[0].FullName ('/S _?='+$InstallationDirectory)
  try{if(!$uninstall.WaitForExit(120000)){$uninstall.Kill();throw 'Owned uninstaller timed out'};if($uninstall.ExitCode -ne 0){throw 'Uninstall failed'}}finally{$uninstall.Dispose()}
  if(Test-Path (Join-Path $InstallationDirectory 'qwen\qwen-speech-worker.exe')){throw 'Uninstall retained worker'}
 }
 $report.stage='complete';Publish
}catch{$report.stage='failed';$report.error=$_.Exception.Message;Publish;throw}
finally{
 if($worker){try{if(!$worker.HasExited){$worker.Kill();[void]$worker.WaitForExit(5000)}}catch{};if($stderrTask){try{$report.workerStderr=$stderrTask.GetAwaiter().GetResult();$report.workerExitCode=$worker.ExitCode;$report.workerExitHex=('0x{0:X8}' -f [BitConverter]::ToUInt32([BitConverter]::GetBytes([int]$worker.ExitCode),0));Publish;if($report.stage -eq 'failed'){Write-Host ($report|ConvertTo-Json -Depth 15)}}catch{}};$worker.Dispose()}
 if($temporary -and (Test-Path -LiteralPath $temporary)){Remove-Item -LiteralPath $temporary}
}
