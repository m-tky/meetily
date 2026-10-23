<# Early clean-PATH startup/shutdown check; no audio, models, profiles or cloud. #>
[CmdletBinding()]
param(
 [Parameter(Mandatory=$true)][string]$RuntimeDirectory,
 [Parameter(Mandatory=$true)][string]$ReportPath
)
$ErrorActionPreference='Stop'
$RuntimeDirectory=[IO.Path]::GetFullPath($RuntimeDirectory)
$r=[ordered]@{stage='starting';error=$null;processorCount=[Environment]::ProcessorCount;realAudioUsed=$false;modelLoaded=$false;runtimeFiles=@()}
$p=$null;$stderr=$null
function Publish{[IO.File]::WriteAllText($ReportPath,($r|ConvertTo-Json -Depth 8))}
function Receive{
 $read=$p.StandardOutput.ReadLineAsync()
 if(!$read.Wait(30000)){throw 'Clean runtime response timed out'}
 $line=$read.GetAwaiter().GetResult()
 if(!$line){throw 'Clean runtime closed stdout before its response'}
 return ($line|ConvertFrom-Json)
}
try{
 $r.runtimeFiles=@(Get-ChildItem -LiteralPath $RuntimeDirectory -File|Where-Object {$_.Extension -in @('.exe','.dll')}|ForEach-Object {@{name=$_.Name;sha256=(Get-FileHash $_.FullName).Hash;version=$_.VersionInfo.FileVersion}})
 Publish
 $si=[Diagnostics.ProcessStartInfo]::new((Join-Path $RuntimeDirectory 'qwen-speech-worker.exe'),('"'+(Join-Path $RuntimeDirectory 'model-not-installed')+'" 6 --model Qwen3-ASR-1.7B'))
 $si.UseShellExecute=$false;$si.CreateNoWindow=$true;$si.RedirectStandardInput=$true;$si.RedirectStandardOutput=$true;$si.RedirectStandardError=$true
 $si.StandardOutputEncoding=[Text.Encoding]::UTF8;$si.StandardErrorEncoding=[Text.Encoding]::UTF8
 $si.EnvironmentVariables['Path']=$env:SystemRoot+'\System32;'+$env:SystemRoot
 $si.EnvironmentVariables['ORT_DYLIB_PATH']=Join-Path $RuntimeDirectory 'onnxruntime.dll'
 foreach($key in @('OPENBLAS_NUM_THREADS','OMP_NUM_THREADS','MKL_NUM_THREADS')){$si.EnvironmentVariables[$key]='6'}
 $p=[Diagnostics.Process]::new();$p.StartInfo=$si;[void]$p.Start();$stderr=$p.StandardError.ReadToEndAsync()
 $ready=Receive
 if($ready.event -ne 'ready' -or $ready.model_loaded -or $ready.threads -ne 6){throw 'Clean runtime handshake failed'}
 $p.StandardInput.WriteLine('{"op":"shutdown","id":"clean-startup"}');$p.StandardInput.Flush()
 $bye=Receive
 if($bye.status -ne 'bye' -or !$p.WaitForExit(10000) -or $p.ExitCode -ne 0){throw 'Clean runtime shutdown failed'}
 $r.stage='complete'
}catch{$r.stage='failed';$r.error=$_.Exception.Message;throw}
finally{
 if($p){
  try{if(!$p.HasExited){$p.Kill();[void]$p.WaitForExit(5000)};$r.exitCode=$p.ExitCode;$r.exitHex=('0x{0:X8}' -f [BitConverter]::ToUInt32([BitConverter]::GetBytes([int]$p.ExitCode),0));if($stderr){$r.stderr=$stderr.GetAwaiter().GetResult()}}finally{$p.Dispose()}
 }
 Publish
 if($r.stage -eq 'failed'){
  Write-Host ($r|ConvertTo-Json -Depth 8)
  if(Get-Command dumpbin.exe -ErrorAction SilentlyContinue){foreach($name in @('qwen-speech-worker.exe','libopenblas.dll','onnxruntime.dll')){& dumpbin.exe /dependents (Join-Path $RuntimeDirectory $name)}}
 }
}
