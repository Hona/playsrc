param([string]$Guard,[string]$Receipt,[string]$Mode,[string]$Repository,[string]$Job,[string]$Task,[string]$InputFile,[string]$Reader)
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
Add-Type -Path $Guard
[PlaysrcReadbackGuard]::Stage=$Mode
[PlaysrcReadbackGuard]::Start($(if($Mode -eq 'deadline'){2000}else{5000}),$(if($Mode -eq 'memory'){134217728}else{536870912}),$Receipt)
switch($Mode) {
 'deadline' {[Threading.Thread]::Sleep(-1)}
 'memory' {$retained=New-Object Collections.ArrayList;for($i=0;$i -lt 16;$i++){$null=$retained.Add((New-Object byte[] 16777216));Start-Sleep -Milliseconds 20};[Threading.Thread]::Sleep(-1)}
 'cim' {$count=@(Get-CimInstance Win32_Process|Where-Object{$_.Name -eq 'bun.exe' -and $_.CommandLine -like "*local-job.ts run $Job*"}).Count;@{stage=$Mode;count=$count}|ConvertTo-Json -Compress}
 'tasks' {$count=@(Get-ScheduledTask -TaskName 'playsrc-local-job-*').Count;@{stage=$Mode;count=$count}|ConvertTo-Json -Compress}
 'result' {$readback=& bun (Join-Path $Repository 'tools/playsrc/src/local-job.ts') result $Job $Task;if($LASTEXITCODE){throw 'Result readback failed'};$result=$readback|ConvertFrom-Json;@{stage=$Mode;resultPresent=($null -ne $result.result);errorLength=$result.launchError.Length}|ConvertTo-Json -Compress}
 'serialize' {$config=Get-Content -Raw (Join-Path $Repository 'playsrc.local.json')|ConvertFrom-Json;$directory=Join-Path $config.sourceCacheDir "local-jobs/$Job";$file=Join-Path $directory ($Task.Replace('playsrc-local-job-','')+'-launch.log');$text=Get-Content -Raw -LiteralPath $file;$status=@{job=$Job;task=@{TaskName=$Task;State=3};running=$false;processes=@();log=$file;launchError=$text;result=$null};$encoded=$status|ConvertTo-Json -Depth 8 -Compress;@{stage=$Mode;inputBytes=(Get-Item -LiteralPath $file).Length;outputCharacters=$encoded.Length}|ConvertTo-Json -Compress}
 'plain' {. $Reader;$value=Read-PlainJobText $InputFile;@{stage=$Mode;launchError=$value;providerProperties=@($value.PSObject.Properties|Where-Object Name -in 'PSPath','PSDrive','PSProvider').Count}|ConvertTo-Json -Depth 8 -Compress}
 default {throw 'Unknown bounded probe mode'}
}
