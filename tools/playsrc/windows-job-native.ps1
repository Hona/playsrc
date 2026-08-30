param([Parameter(Mandatory=$true)][string]$Request)
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
[Console]::OutputEncoding=New-Object System.Text.UTF8Encoding($false)
try {
  # Inherits the scheduled interactive token, not an SSH desktop.
  $parent=(Get-CimInstance Win32_Process -Filter "ProcessId=$PID").ParentProcessId
  Add-Type -Path (Join-Path $PSScriptRoot 'windows-job-native.cs') -ReferencedAssemblies System.Web.Extensions,System.Drawing
  [PlaysrcNativeJob]::Run($Request,[int]$parent)
} catch { [Console]::Error.WriteLine($_.Exception.ToString()); exit 1 }
