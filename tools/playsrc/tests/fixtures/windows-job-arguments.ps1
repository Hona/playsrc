param([Parameter(Mandatory=$true)][string]$Bridge)
$ErrorActionPreference='Stop'
$tokens=$null;$errors=$null
$ast=[System.Management.Automation.Language.Parser]::ParseFile($Bridge,[ref]$tokens,[ref]$errors)
if($errors.Count){throw 'Bridge syntax errors'}
$function=$ast.Find({param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'ArgumentArray'},$true)
. ([ScriptBlock]::Create($function.Extent.Text))
$many=ArgumentArray '["test","a.test.ts","b.test.ts"]' 20
$empty=ArgumentArray '[]' 20
$one=ArgumentArray '["a.test.ts"]' 20
$rejected=0
foreach($bad in '[null]','[1]','[["a"]]','"test"') {try {$null=ArgumentArray $bad 20}catch{$rejected++}}
@{many=$many;emptyCount=$empty.Count;one=$one;rejected=$rejected}|ConvertTo-Json -Compress
