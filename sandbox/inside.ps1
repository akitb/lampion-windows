# Runs INSIDE Windows Sandbox at logon: installs Lampion from in\ and, depending on in\mode.txt,
# runs the self-tests or leaves the app open for a manual look. Results go to out\.
$ErrorActionPreference = "Continue"
$desk = "C:\Users\WDAGUtilityAccount\Desktop"
$in = Join-Path $desk "in"
$out = Join-Path $desk "out"
$mode = (Get-Content (Join-Path $in "mode.txt") -ErrorAction SilentlyContinue | Select-Object -First 1)
if (-not $mode) { $mode = "interactive" }
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$result = Join-Path $out $stamp
New-Item -ItemType Directory -Force $result | Out-Null
Start-Transcript -Path (Join-Path $result "inside.log") | Out-Null

$installer = Get-ChildItem $in -Filter "*setup*.exe" | Select-Object -First 1
if (-not $installer) { "No installer in in\"; Stop-Transcript; exit 1 }
"Installing $($installer.Name) ($mode)"
$p = Start-Process $installer.FullName -ArgumentList "/S" -PassThru -Wait
"Installer exit code: $($p.ExitCode)"

$exe = Get-ChildItem "$env:LOCALAPPDATA\Programs\Lampion" -Filter "lampion*.exe" -Recurse -ErrorAction SilentlyContinue |
  Where-Object { $_.Name -notmatch "uninstall" } | Select-Object -First 1
if (-not $exe) { "Lampion.exe not found after install"; Stop-Transcript; exit 1 }
"App: $($exe.FullName)"

function Save-Logs {
  Copy-Item "$env:LOCALAPPDATA\Lampion\*.log" $result -ErrorAction SilentlyContinue
  Copy-Item "$env:APPDATA\Lampion\settings.json" $result -ErrorAction SilentlyContinue
  if (Test-Path "C:\Lampion\logs") { Copy-Item "C:\Lampion\logs" (Join-Path $result "server-logs") -Recurse -ErrorAction SilentlyContinue }
}

switch ($mode) {
  "selftest" {
    foreach ($flag in @("--selftest", "--dbtest")) {
      "Running $flag"
      $t = Start-Process $exe.FullName -ArgumentList $flag -PassThru
      if (-not $t.WaitForExit(1800000)) { "$flag timed out"; $t.Kill() }
      "$flag exit code: $($t.ExitCode)"
      Save-Logs
    }
    "DONE" | Set-Content (Join-Path $result "done.txt")
  }
  default {
    # Manual look: start the app and keep collecting its logs every 30 s until the sandbox is closed.
    Start-Process $exe.FullName
    while ($true) { Start-Sleep 30; Save-Logs }
  }
}
Stop-Transcript | Out-Null
