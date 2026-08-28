/** Private diagnostic receipts only. No focus/activation or UI mutation. */
export const WINDOWS_OWNED_UI = String.raw`
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
function Read-OwnedUI([long]$windowId,[uint32]$processId) {
 $clock=[Diagnostics.Stopwatch]::StartNew()
 $root=[System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]$windowId)
 if (!$root -or $root.Current.ProcessId -ne $processId) { throw 'Owned UI root identity differs' }
 $walker=[System.Windows.Automation.TreeWalker]::ControlViewWalker
 $queue=New-Object 'System.Collections.Generic.Queue[object]'
 $queue.Enqueue(@{element=$root;depth=0})
 $rows=New-Object 'System.Collections.Generic.List[object]'
 $depthLimited=$false
 while ($queue.Count -gt 0 -and $rows.Count -lt 48 -and $clock.ElapsedMilliseconds -lt 1500) {
  $item=$queue.Dequeue();$element=$item.element;$current=$element.Current
  if ($current.ProcessId -ne $processId) { continue }
  $name=[string]$current.Name;$automation=[string]$current.AutomationId
  if ($name.Length -gt 256) {$name=$name.Substring(0,256)}
  if ($automation.Length -gt 128) {$automation=$automation.Substring(0,128)}
  $rect=$current.BoundingRectangle
  $rows.Add(@{name=$name;automationId=$automation;controlType=$current.ControlType.ProgrammaticName;className=$current.ClassName;enabled=$current.IsEnabled;offscreen=$current.IsOffscreen;depth=$item.depth;runtimeId=$element.GetRuntimeId();bounds=@{x=$rect.X;y=$rect.Y;width=$rect.Width;height=$rect.Height}})
  if ($item.depth -lt 4) {
   $child=$walker.GetFirstChild($element);$count=0
   while ($child -and $count -lt 24 -and $queue.Count -lt 48 -and $clock.ElapsedMilliseconds -lt 1500) {
    $queue.Enqueue(@{element=$child;depth=$item.depth+1});$child=$walker.GetNextSibling($child);$count++
   }
  } else {$depthLimited=$true}
 }
 return @{windowId=$windowId;processId=$processId;milliseconds=$clock.ElapsedMilliseconds;truncated=($queue.Count -gt 0 -or $depthLimited);elements=$rows.ToArray()}
}
`

export function ownedDiagnosticWindow(native: any, bounds: { left: number; top: number; width: number; height: number }, browserPid: number): number {
  const matches = native.windows.filter((window: any) => window.visible && !window.minimized && window.bounds.Left === bounds.left && window.bounds.Top === bounds.top
    && window.bounds.Right - window.bounds.Left === bounds.width && window.bounds.Bottom - window.bounds.Top === bounds.height)
  if (matches.length !== 1 || native.foregroundOwner?.processId !== browserPid
    || native.foregroundAfter !== native.foreground || ![native.foreground, native.foregroundOwner.rootOwnerWindowId].includes(matches[0].id)) {
    throw new Error("Diagnostic window is not the measured browser or its owned foreground UI")
  }
  return matches[0].id
}
