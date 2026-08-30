param([Parameter(Mandatory=$true)][string]$Request)
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
[Console]::OutputEncoding=New-Object System.Text.UTF8Encoding($false)
try {
  # No caller-controlled UI switch. Revalidate the invocation AND command using
  # the same classifier as queue admission, including direct -File requests.
  $validated=& bun (Join-Path $PSScriptRoot 'src/local-job.ts') validate-native $Request
  if($LASTEXITCODE){throw 'Invalid native workload request'}
  $plan=$validated|ConvertFrom-Json
  if($plan.interactive -isnot [bool]){throw 'Missing native workload classification'}
  $parent=(Get-CimInstance Win32_Process -Filter "ProcessId=$PID").ParentProcessId
  Add-Type -Path (Join-Path $PSScriptRoot 'windows-job-native.cs') -ReferencedAssemblies System.Web.Extensions,System.Drawing
  [PlaysrcNativeJob]::Run($Request,[int]$parent,$plan.interactive)
} catch { [Console]::Error.WriteLine($_.Exception.ToString()); exit 1 }
