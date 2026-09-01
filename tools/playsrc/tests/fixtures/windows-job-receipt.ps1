param([Parameter(Mandatory=$true)][string]$Source,[Parameter(Mandatory=$true)][string]$Directory)
$ErrorActionPreference='Stop'
Add-Type -Path $Source -ReferencedAssemblies System.Web.Extensions,System.Drawing
$save=[PlaysrcNativeJob].GetMethod('Save',[Reflection.BindingFlags]'NonPublic,Static')
$receipt=New-Object PlaysrcNativeJob+Receipt
$receipt.helperPeakPrivateBytes=77905920
$file=Join-Path $Directory 'receipt.json'
$published=$save.Invoke($null,@($file.PSObject.BaseObject,$receipt.PSObject.BaseObject))
$receipt.helperPeakPrivateBytes=77942784
$retained=[IO.File]::ReadAllText($file)
if($published -isnot [string] -or $published -cne $retained){throw 'Published receipt is not the exact retained serialization'}
if(($published|ConvertFrom-Json).helperPeakPrivateBytes -ne 77905920){throw 'Published receipt changed after later sampler activity'}
@{sameBytes=$true;retainedPeak=($retained|ConvertFrom-Json).helperPeakPrivateBytes;livePeak=$receipt.helperPeakPrivateBytes}|ConvertTo-Json -Compress
