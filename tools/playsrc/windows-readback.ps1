# Get-Content attaches provider/drive note properties to its strings. Windows
# PowerShell5's JSON serializer traverses those graphs, even for a tiny log.
function Read-PlainJobText([string]$Path) {
 if((Get-Item -LiteralPath $Path).Length -gt 2097152){throw 'Job launch text exceeds its2MiB bound'}
 return [IO.File]::ReadAllText($Path)
}
