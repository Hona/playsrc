/** Private diagnostic receipts only. No focus/activation or UI mutation. */
export const WINDOWS_OWNED_UI = String.raw`
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
function Read-UIBounds($rect) {
 if ($rect.IsEmpty) {return $null}
 foreach ($value in @($rect.X,$rect.Y,$rect.Width,$rect.Height)) {
  if ([double]::IsNaN($value) -or [double]::IsInfinity($value)) {return $null}
 }
 return @{x=$rect.X;y=$rect.Y;width=$rect.Width;height=$rect.Height}
}
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
  $rows.Add(@{name=$name;automationId=$automation;controlType=$current.ControlType.ProgrammaticName;className=$current.ClassName;enabled=$current.IsEnabled;offscreen=$current.IsOffscreen;depth=$item.depth;runtimeId=$element.GetRuntimeId();bounds=(Read-UIBounds $rect)})
  if ($item.depth -lt 8) {
   $child=$walker.GetFirstChild($element);$count=0
   while ($child -and $count -lt 24 -and $queue.Count -lt 48 -and $clock.ElapsedMilliseconds -lt 1500) {
    $queue.Enqueue(@{element=$child;depth=$item.depth+1});$child=$walker.GetNextSibling($child);$count++
   }
  } else {$depthLimited=$true}
 }
 return @{windowId=$windowId;processId=$processId;milliseconds=$clock.ElapsedMilliseconds;truncated=($queue.Count -gt 0 -or $depthLimited);elements=$rows.ToArray()}
}
`

/** Explicit normal-control action, separate from the read-only reader. Only
 * the observed loopback permission in the owned ephemeral browser is allowed. */
export const WINDOWS_LOCAL_PERMISSION = String.raw`
function Allow-OwnedLocalPermission($ui,[string]$origin,[uint32]$processId,[long]$windowId) {
 if ($origin -notmatch '^http://127\.0\.0\.1:[0-9]+$') {throw 'Permission origin is not the owned loopback application'}
 $expected=$origin+' wants to: Access other apps and services on this device'
 $prompt=@($ui.elements | Where-Object {$_.className -eq 'RootView' -and $_.controlType -eq 'ControlType.Window' -and $_.name -eq $expected})
 $buttons=@($ui.elements | Where-Object {$_.controlType -eq 'ControlType.Button' -and $_.name -eq 'Allow' -and $_.enabled -and !$_.offscreen -and $_.bounds.width -gt 0 -and $_.bounds.height -gt 0})
 if ($prompt.Count -ne 1 -or $buttons.Count -ne 1 -or [StartupWindow]::GetForegroundWindow().ToInt64() -ne $windowId) {throw 'Observed local permission control differs'}
 $root=[System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]$windowId)
 if ($root.Current.ProcessId -ne $processId) {throw 'Permission process changed'}
 $name=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty,'Allow')
 $type=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty,[System.Windows.Automation.ControlType]::Button)
 $condition=New-Object System.Windows.Automation.AndCondition($name,$type)
 $matches=$root.FindAll([System.Windows.Automation.TreeScope]::Descendants,$condition)
 if ($matches.Count -ne 1) {throw 'Permission control is ambiguous'}
 $button=$matches[0]
 if (($button.GetRuntimeId() -join ':') -ne ($buttons[0].runtimeId -join ':') -or !$button.Current.IsEnabled -or $button.Current.IsOffscreen) {throw 'Permission control changed'}
 $pattern=$button.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
 $start=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds();$pattern.Invoke()
 return @{action='normal-visible-Allow';origin=$origin;windowId=$windowId;processId=$processId;control=$buttons[0];startedEpoch=$start;endedEpoch=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()}
}
`

export function assertOwnedEphemeralBrowser(arguments_: readonly string[]): void {
  const profiles = arguments_.filter(value => value.startsWith("--user-data-dir="))
  if (profiles.length !== 1 || !/[\\/]playwright_chromiumdev_profile-[a-zA-Z0-9_-]+$/u.test(profiles[0]!)
    || !arguments_.includes("--enable-automation")) throw new Error("Permission action requires the owned ephemeral automation profile")
}

export function ownedDiagnosticWindow(native: any, bounds: { left: number; top: number; width: number; height: number }, browserPid: number): number {
  const matches = native.windows.filter((window: any) => window.visible && !window.minimized && window.bounds.Left === bounds.left && window.bounds.Top === bounds.top
    && window.bounds.Right - window.bounds.Left === bounds.width && window.bounds.Bottom - window.bounds.Top === bounds.height)
  if (matches.length !== 1 || native.foregroundOwner?.processId !== browserPid
    || native.foregroundAfter !== native.foreground || ![native.foreground, native.foregroundOwner.rootOwnerWindowId].includes(matches[0].id)) {
    throw new Error("Diagnostic window is not the measured browser or its owned foreground UI")
  }
  return matches[0].id
}
