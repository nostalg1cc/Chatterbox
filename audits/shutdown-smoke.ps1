param([string]$Executable = 'src-tauri/target/release/nitro.exe')
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class NitroShutdownSmoke {
  private delegate bool WindowCallback(IntPtr hwnd, IntPtr context);
  [DllImport("user32.dll")] private static extern bool EnumWindows(WindowCallback callback, IntPtr context);
  [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int size);
  [DllImport("user32.dll")] private static extern bool PostMessage(IntPtr hwnd, uint message, IntPtr wParam, IntPtr lParam);
  public static IntPtr Find(uint processId, string title) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((hwnd, context) => {
      uint owner; GetWindowThreadProcessId(hwnd, out owner);
      var text = new StringBuilder(256); GetWindowText(hwnd, text, text.Capacity);
      if (owner == processId && text.ToString() == title) found = hwnd;
      return true;
    }, IntPtr.Zero);
    return found;
  }
  public static bool Close(IntPtr hwnd) { return PostMessage(hwnd, 0x0010, IntPtr.Zero, IntPtr.Zero); }
}
'@
$appPath = (Resolve-Path -LiteralPath $Executable).Path
$profileRoot = Join-Path (Get-Location).Path '.release-workspace/shutdown-smoke-profile'
$previousProfile = $env:WEBVIEW2_USER_DATA_FOLDER
$testProcess = $null
try {
  $env:WEBVIEW2_USER_DATA_FOLDER = $profileRoot
  $testProcess = Start-Process -FilePath $appPath -WindowStyle Hidden -PassThru
  $deadline = [DateTime]::UtcNow.AddSeconds(20)
  do {
    $main = [NitroShutdownSmoke]::Find($testProcess.Id, 'Nitro')
    $hud = [NitroShutdownSmoke]::Find($testProcess.Id, 'Nitro Voice')
    if ($main -ne [IntPtr]::Zero -and $hud -ne [IntPtr]::Zero) { break }
    Start-Sleep -Milliseconds 200
  } while ([DateTime]::UtcNow -lt $deadline)
  if ($main -eq [IntPtr]::Zero -or $hud -eq [IntPtr]::Zero) { throw 'Both native windows did not initialize.' }
  Start-Sleep -Seconds 2
  $allProcesses = @(Get-CimInstance Win32_Process)
  $ownedIds = @([uint32]$testProcess.Id)
  do {
    $newIds = @($allProcesses | Where-Object { $_.ParentProcessId -in $ownedIds -and $_.ProcessId -notin $ownedIds } | ForEach-Object { [uint32]$_.ProcessId })
    $ownedIds += $newIds
  } while ($newIds.Count -gt 0)
  if (-not [NitroShutdownSmoke]::Close($main)) { throw 'Main close message failed.' }
  if (-not $testProcess.WaitForExit(10000)) { throw 'Nitro stayed running after its main window closed.' }
  $deadline = [DateTime]::UtcNow.AddSeconds(10)
  do {
    $remaining = @($ownedIds | Where-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue })
    if ($remaining.Count -eq 0) { break }
    Start-Sleep -Milliseconds 200
  } while ([DateTime]::UtcNow -lt $deadline)
  if ($remaining.Count -gt 0) { throw "App-owned processes remain: $remaining" }
  if ($testProcess.ExitCode -ne 0) { throw "Unexpected exit code: $($testProcess.ExitCode)" }
  Write-Output "PASS: main and hidden HUD initialized; native close exited Nitro and $($ownedIds.Count - 1) child processes."
} finally {
  $env:WEBVIEW2_USER_DATA_FOLDER = $previousProfile
  if ($testProcess -and -not $testProcess.HasExited) { Stop-Process -Id $testProcess.Id }
}
