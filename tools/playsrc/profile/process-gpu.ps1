param([string]$ProcessIds, [ValidateRange(5,10)][int]$Seconds)
$ErrorActionPreference = 'Stop'
if ($ProcessIds -notmatch '^[1-9][0-9]*(,[1-9][0-9]*)*$') { throw 'Invalid GPU process IDs' }
$ids = @($ProcessIds.Split(',') | ForEach-Object { [int]$_ })
$counters = @($ids | ForEach-Object { "\GPU Engine(pid_$($_)_*)\Utilization Percentage" })
$rows = @()
$first = $true
Get-Counter -Counter $counters -SampleInterval 1 -MaxSamples ($Seconds + 2) | ForEach-Object {
  if ($first) { [Console]::Out.WriteLine('READY'); $first = $false }
  foreach ($sample in $_.CounterSamples) {
    if ($sample.InstanceName -match '^pid_([0-9]+)_' -and $ids -contains [int]$Matches[1]) {
      $rows += [pscustomobject]@{
        at = $sample.Timestamp.ToUniversalTime().ToString('o')
        timestamp100ns = [string]$sample.Timestamp100NS
        instance = $sample.InstanceName
        pid = [int]$Matches[1]
        percent = $sample.CookedValue
        status = $sample.Status
      }
    }
  }
}
ConvertTo-Json -InputObject @($rows) -Compress
