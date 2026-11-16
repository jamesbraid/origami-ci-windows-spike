# Drives origami-shell on an interactive Windows desktop: real keyboard and mouse input,
# UI Automation for the controls, a screenshot after each step, and a UIA tree dump.
# Usage: drive.ps1 <exe> <outdir>
param([string]$Exe, [string]$Out)
$ErrorActionPreference = 'Continue'
New-Item -ItemType Directory -Force -Path $Out | Out-Null
Add-Type -AssemblyName System.Windows.Forms, System.Drawing, UIAutomationClient, UIAutomationTypes
Add-Type @'
using System; using System.Runtime.InteropServices;
public class W {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(int f, int dx, int dy, int d, IntPtr e);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int w, int hh, uint f);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, int flags, IntPtr e);
}
'@
$script:n = 0
function Log($m) { $line = "[drive] $m"; Write-Host $line; Add-Content -Path "$Out\drive.log" -Value $line }
function Shot($name) {
  $script:n++
  $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
  $f = "{0}\{1:D2}-{2}.png" -f $Out, $script:n, $name
  $bmp.Save($f); $g.Dispose(); $bmp.Dispose()
  Log "screenshot $f"
}
function Keys($s) { [System.Windows.Forms.SendKeys]::SendWait($s); Start-Sleep -Milliseconds 700 }
function Main-Window {
  $cond = New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::ProcessIdProperty, $script:proc.Id)
  [Windows.Automation.AutomationElement]::RootElement.FindAll([Windows.Automation.TreeScope]::Children, $cond)
}
function Dump-Tree($el, $depth, $sb) {
  if ($depth -gt 14) { return }
  $c = $el.Current
  $pats = ($el.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName -replace 'Identifiers.Pattern', '' }) -join ','
  [void]$sb.AppendLine(("{0}{1} name='{2}' id='{3}' class='{4}' patterns=[{5}] rect={6}" -f (' ' * ($depth * 2)), $c.ControlType.ProgrammaticName.Replace('ControlType.', ''), $c.Name, $c.AutomationId, $c.ClassName, $pats, $c.BoundingRectangle))
  $w = [Windows.Automation.TreeWalker]::RawViewWalker
  $k = $w.GetFirstChild($el)
  while ($k -ne $null) { Dump-Tree $k ($depth + 1) $sb; $k = $w.GetNextSibling($k) }
}
function Dump($name) {
  $sb = New-Object System.Text.StringBuilder
  foreach ($w in (Main-Window)) { Dump-Tree $w 0 $sb }
  $sb.ToString() | Set-Content -Encoding utf8 "$Out\uia-$name.txt"
  $count = ($sb.ToString() -split "`n").Count
  Log "uia dump $name : $count lines"
}
function Find($by, $val, $scope = 'Descendants') {
  $prop = if ($by -eq 'id') { [Windows.Automation.AutomationElement]::AutomationIdProperty } else { [Windows.Automation.AutomationElement]::NameProperty }
  $cond = New-Object Windows.Automation.PropertyCondition($prop, $val)
  foreach ($w in (Main-Window)) {
    $r = $w.FindFirst([Windows.Automation.TreeScope]::Descendants, $cond)
    if ($r -ne $null) { return $r }
  }
  return $null
}
function Act($by, $val) {
  $el = Find $by $val
  if ($el -eq $null) { Log "ACT $by=$val : NOT FOUND"; return $false }
  try {
    $p = $null
    if ($el.TryGetCurrentPattern([Windows.Automation.InvokePattern]::Pattern, [ref]$p)) { $p.Invoke(); Log "ACT $by=$val : invoke"; Start-Sleep -Milliseconds 800; return $true }
    if ($el.TryGetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern, [ref]$p)) { $p.Select(); Log "ACT $by=$val : select"; Start-Sleep -Milliseconds 800; return $true }
    if ($el.TryGetCurrentPattern([Windows.Automation.TogglePattern]::Pattern, [ref]$p)) { $p.Toggle(); Log "ACT $by=$val : toggle"; Start-Sleep -Milliseconds 800; return $true }
    if ($el.TryGetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern, [ref]$p)) { $p.Expand(); Log "ACT $by=$val : expand"; Start-Sleep -Milliseconds 800; return $true }
    Log "ACT $by=$val : no usable pattern"
  } catch { Log "ACT $by=$val : EXC $($_.Exception.Message)" }
  return $false
}
function Click-At($x, $y, $hold = $false) {
  [W]::SetCursorPos($x, $y); Start-Sleep -Milliseconds 150
  [W]::mouse_event(0x2, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 150
  if (-not $hold) { [W]::mouse_event(0x4, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 300 }
}
function Step($name, [scriptblock]$body) { Log "== $name"; try { & $body } catch { Log "STEP $name EXC: $($_.Exception.Message)" } }

# --- launch
$exeDir = Split-Path $Exe
Log "exe size bytes: $((Get-Item $Exe).Length)"
$script:proc = Start-Process -FilePath $Exe -WorkingDirectory $exeDir -PassThru -RedirectStandardOutput "$Out\app-stdout.txt" -RedirectStandardError "$Out\app-stderr.txt"
$deadline = (Get-Date).AddSeconds(60)
while ((Get-Date) -lt $deadline) { if (@(Main-Window).Count -gt 0) { break }; if ($script:proc.HasExited) { break }; Start-Sleep -Milliseconds 500 }
Log "process exited early: $($script:proc.HasExited) exit=$(if ($script:proc.HasExited) { $script:proc.ExitCode })"
Start-Sleep -Seconds 4
$mw = @(Main-Window)
Log "top-level UIA windows: $($mw.Count)"
if ($mw.Count -gt 0) {
  $hwnd = [IntPtr]$mw[0].Current.NativeWindowHandle
  Log "main hwnd=$hwnd dpi=$([W]::GetDpiForWindow($hwnd)) rect=$($mw[0].Current.BoundingRectangle)"
  [W]::SetWindowPos($hwnd, [IntPtr]::Zero, 20, 10, 984, 700, 0x40) | Out-Null
  [W]::SetForegroundWindow($hwnd) | Out-Null
  Start-Sleep -Seconds 2
}
Shot 'initial'
Dump 'initial'

Step 'accelerators, not grabbed' {
  Keys '^n'; Keys '^r'; Keys '^w'
  Keys '^,'; Shot 'ctrl-comma-settings'
  Act 'id' 'library' | Out-Null
  Act 'name' 'Library' | Out-Null
}
Step 'file picker via Ctrl+O' {
  Keys '^o'; Start-Sleep -Seconds 2; Shot 'file-picker-open'
  $dlg = [Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Children, (New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::ClassNameProperty, '#32770')))
  Log "common file dialog found: $($dlg -ne $null)"
  Keys 'C:\Windows\win.ini{ENTER}'; Start-Sleep -Seconds 2
  Shot 'after-file-picker'
}
Step 'settings page and form' {
  Act 'name' 'Settings' | Out-Null
  Start-Sleep -Seconds 1
  Shot 'settings'
  $tb = Find 'id' 'setting-name'
  if ($tb) { $p = $null; if ($tb.TryGetCurrentPattern([Windows.Automation.ValuePattern]::Pattern, [ref]$p)) { $p.SetValue('Octane2 (renamed)'); Log 'textbox value set via UIA' } }
  Act 'id' 'setting-grab' | Out-Null
  Act 'id' 'setting-grab' | Out-Null
  Act 'id' 'card-1' | Out-Null
  Start-Sleep -Milliseconds 500
  Shot 'settings-edited'
  Dump 'settings'
}
Step 'dark theme' {
  Act 'name' 'Dark' | Out-Null
  Start-Sleep -Seconds 1
  Shot 'theme-dark'
  Dump 'settings-dark'
}
Step 'assistant dialog' {
  Keys '^+n'; Start-Sleep -Seconds 1; Shot 'assistant-step1'
  Dump 'assistant'
  Act 'name' 'Next' | Out-Null; Start-Sleep -Seconds 1; Shot 'assistant-step2'
  Act 'name' 'Next' | Out-Null; Start-Sleep -Seconds 1; Shot 'assistant-step3'
  Act 'name' 'Create' | Out-Null; Start-Sleep -Seconds 1; Shot 'assistant-done'
}
Step 'light theme and menus' {
  Act 'name' 'Light' | Out-Null
  Start-Sleep -Seconds 1
  Act 'name' 'File' | Out-Null; Start-Sleep -Seconds 1; Shot 'menu-file-open'; Dump 'menu-open'
  Keys '{ESC}'
  Act 'name' 'Machine' | Out-Null; Start-Sleep -Seconds 1; Shot 'menu-machine-open'
  Act 'name' 'Reset...' | Out-Null; Start-Sleep -Seconds 1; Shot 'confirm-dialog'
  Act 'name' 'Cancel' | Out-Null
}
Step 'library, display, pointer capture, keyboard grab' {
  Act 'name' 'Library' | Out-Null
  Start-Sleep -Seconds 2
  Shot 'library-display'
  Dump 'library'
  $d = Find 'id' 'display'
  if ($d) {
    $r = $d.Current.BoundingRectangle
    Log "display rect: $r"
    $cx = [int]($r.X + $r.Width / 2); $cy = [int]($r.Y + $r.Height / 2)
    Click-At $cx $cy $true
    [W]::SetCursorPos($cx + 40, $cy + 20); Start-Sleep -Milliseconds 200
    [W]::SetCursorPos($cx + 80, $cy + 40); Start-Sleep -Milliseconds 200
    Shot 'drag-inside'
    # drag far outside the panel (past the window edge) while the button is still down
    [W]::SetCursorPos([int]($r.X + $r.Width + 60), [int]($r.Y - 30)); Start-Sleep -Milliseconds 300
    [W]::SetCursorPos([int]($r.X + $r.Width + 90), [int]($r.Y - 60)); Start-Sleep -Milliseconds 300
    [W]::mouse_event(0x4, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 500
    Log 'released the button outside the panel'
    Keys 'abc'            # grabbed: should reach the core, not any control
    Keys '^o'; Start-Sleep -Seconds 1  # grabbed: must NOT open the picker
    Shot 'grabbed-after-keys'
    Keys '^%g'            # Ctrl+Alt+G releases
    Shot 'released'
  } else { Log 'display element not found by UIA' }
}
Step 'tabs' {
  Act 'name' 'Add tab' | Out-Null; Start-Sleep -Seconds 1; Shot 'tab-added'
  Dump 'tabs'
}
Step 'second window and console' {
  Act 'name' 'Octane2' | Out-Null
  Act 'name' 'Pop out' | Out-Null; Start-Sleep -Seconds 3; Shot 'popout-window'
  $mw = @(Main-Window); Log "top-level windows now: $($mw.Count)"
  Act 'name' 'More' | Out-Null
  Act 'name' 'Console window' | Out-Null; Start-Sleep -Seconds 3
  Keys 'hello'; Shot 'console-window'
  Dump 'two-windows'
}
Step 'resize' {
  $mw = @(Main-Window)
  if ($mw.Count -gt 0) {
    $hwnd = [IntPtr]$mw[$mw.Count - 1].Current.NativeWindowHandle
    [W]::SetWindowPos($hwnd, [IntPtr]::Zero, 10, 10, 700, 520, 0x40) | Out-Null
    Start-Sleep -Seconds 2; Shot 'resized'
  }
}
Step 'tab tear-out attempt' {
  $ti = Find 'name' 'O2'
  if ($ti) {
    $r = $ti.Current.BoundingRectangle
    $x = [int]($r.X + $r.Width / 2); $y = [int]($r.Y + $r.Height / 2)
    Click-At $x $y $true
    for ($i = 1; $i -le 10; $i++) { [W]::SetCursorPos($x, $y + $i * 30); Start-Sleep -Milliseconds 80 }
    [W]::mouse_event(0x4, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Seconds 1
    Shot 'tab-drag-out'
    Log "top-level windows after tab drag: $(@(Main-Window).Count)"
  } else { Log 'O2 tab not found' }
}
Start-Sleep -Seconds 1
Log "process still running: $(-not $script:proc.HasExited)"
Stop-Process -Id $script:proc.Id -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 1
Log 'done'
