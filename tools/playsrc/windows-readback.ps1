# Get-Content attaches provider/drive note properties to its strings. Windows
# PowerShell5's JSON serializer traverses those graphs, even for a tiny log.
function Read-PlainJobText([string]$Path) {
 # A queued task can enter while readback retires the previous task. Permit
 # the live owner's existing write/delete handles; this reader never owns it.
 $stream=[IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,([IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete))
 try {
  if($stream.Length -gt 2097152){throw 'Job launch text exceeds its2MiB bound'}
  $reader=[IO.StreamReader]::new($stream,[Text.Encoding]::UTF8,$true)
  try {return $reader.ReadToEnd()} finally {$reader.Dispose()}
 } finally {$stream.Dispose()}
}
