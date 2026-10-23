<# Explicit local preparation of the pinned public model. No audio, credentials,
   app settings or cloud inference. No network unless -Download is supplied. #>
[CmdletBinding()]
param(
 [string]$ModelDirectory='',
 [ValidateSet('1.7B','0.6B')][string]$ModelSize='1.7B',
 [string]$RuntimeDirectory=$PSScriptRoot,
 [switch]$Download
)
$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
if(!$ModelDirectory){$ModelDirectory=Join-Path $env:LOCALAPPDATA ('local.meetily.desktop\models\Qwen3-ASR-'+$ModelSize)}
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$OutputEncoding=[Console]::OutputEncoding
$ModelDirectory=[IO.Path]::GetFullPath($ModelDirectory)
$manifestName=if($ModelSize -eq '0.6B'){'qwen-model-0.6b.json'}else{'qwen-model.json'}
$revision=if($ModelSize -eq '0.6B'){'5eb144179a02acc5e5ba31e748d22b0cf3e303b0'}else{'7278e1e70fe206f11671096ffdd38061171dd6e5'}
$manifest=[IO.File]::ReadAllText((Join-Path $RuntimeDirectory $manifestName))|ConvertFrom-Json
if($manifest.id -ne ('Qwen/Qwen3-ASR-'+$ModelSize) -or $manifest.revision -ne $revision){throw 'Unexpected model manifest'}
$initializer=Join-Path $RuntimeDirectory 'qwen-model-init.exe'
if(!(Test-Path -LiteralPath $initializer -PathType Leaf)){throw 'Bundled model initializer is missing'}
[void][IO.Directory]::CreateDirectory($ModelDirectory)
if((Get-Item -LiteralPath $ModelDirectory).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Model directory must not be a link/reparse point'}
function Get-ModelSha256([string]$path){
 # Buffer OS reads for large shards, including high-latency/network filesystems.
 $stream=$null;$sha=[Security.Cryptography.SHA256]::Create()
 try{
  $stream=[IO.FileStream]::new($path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read,1048576,[IO.FileOptions]::SequentialScan)
  return [BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-','').ToLowerInvariant()
 }finally{if($stream){$stream.Dispose()};$sha.Dispose()}
}
# FileShare.None prevents two preparations from downloading/quantizing together.
$lease=$null
try{
 $lease=[IO.File]::Open((Join-Path $ModelDirectory '.meetily-preparation.lock'),[IO.FileMode]::OpenOrCreate,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None)
 foreach($entry in $manifest.files){
  if([IO.Path]::GetFileName($entry.file) -cne $entry.file -or $entry.file -match '[:\\/]'){throw 'Invalid model filename'}
  $path=Join-Path $ModelDirectory $entry.file
  if(Test-Path -LiteralPath $path){
   $item=Get-Item -LiteralPath $path
   if($item.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Linked model file rejected'}
   if($item.Length -ne $entry.bytes -or (Get-ModelSha256 $path) -ne $entry.sha256){throw ('Existing model file differs; not overwritten: '+$entry.file)}
   continue
  }
  if(!$Download){throw ('Missing '+$entry.file+'. Supply -Download to explicitly fetch the selected public model.')}
  $temporary=Join-Path $ModelDirectory ('.download-'+[Guid]::NewGuid().ToString('N'))
  try{
   Write-Host ('Downloading public model file: '+$entry.file)
   $url='https://huggingface.co/'+$manifest.id+'/resolve/'+$manifest.revision+'/'+$entry.file
   $old=$ErrorActionPreference
   try{$ErrorActionPreference='Continue';& curl.exe --fail --location --silent --show-error --retry 3 --connect-timeout 30 --max-time 1800 --proto '=https' --proto-redir '=https' --output $temporary $url;$downloadExit=$LASTEXITCODE}finally{$ErrorActionPreference=$old}
   if($downloadExit -ne 0){throw ('Public model download failed: '+$entry.file)}
   if((Get-Item -LiteralPath $temporary).Length -ne $entry.bytes -or (Get-ModelSha256 $temporary) -ne $entry.sha256){throw ('Downloaded model checksum mismatch: '+$entry.file)}
   Move-Item -LiteralPath $temporary -Destination $path
  }finally{if(Test-Path -LiteralPath $temporary){Remove-Item -LiteralPath $temporary}}
 }
 $env:OPENBLAS_NUM_THREADS='6';$env:OMP_NUM_THREADS='6';$env:MKL_NUM_THREADS='6'
 $env:HF_HUB_DISABLE_TELEMETRY='1';$env:DO_NOT_TRACK='1'
 $old=$ErrorActionPreference
 try{$ErrorActionPreference='Continue';& $initializer $ModelDirectory;$code=$LASTEXITCODE}finally{$ErrorActionPreference=$old}
 if($code -ne 0){throw 'Local INT8 cache initialization failed'}
 $cache=Join-Path $ModelDirectory 'qwen-asr-int8.sidecar'
 if(!(Test-Path -LiteralPath $cache -PathType Leaf) -or (Get-Item -LiteralPath $cache).Length -lt 64){throw 'Local INT8 cache was not created'}
 Write-Host ('Prepared local model: '+$ModelDirectory)
 Write-Host 'Select Qwen in Settings / Transcription, then Validate and select Qwen. No provider was switched automatically.'
}finally{if($lease){$lease.Dispose()}}
