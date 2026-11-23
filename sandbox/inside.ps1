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

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$shell = New-Object -ComObject WScript.Shell
function Save-Shot([string]$name) {
  $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
  $bmp.Save((Join-Path $result "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
}
function Send-ToApp([string]$title, [string]$keys) {
  if ($shell.AppActivate($title)) { Start-Sleep -Milliseconds 400; $shell.SendKeys($keys) } else { "Window '$title' not found for $keys" }
}
function Test-Url([string]$url) {
  try { $r = Invoke-WebRequest $url -UseBasicParsing -TimeoutSec 10; "$url -> $($r.StatusCode)" } catch { "$url -> $($_.Exception.Message)" }
}
function Run-Selftests {
  foreach ($flag in @("--selftest", "--dbtest")) {
    "Running $flag"
    $t = Start-Process $exe.FullName -ArgumentList $flag -PassThru
    if (-not $t.WaitForExit(1800000)) { "$flag timed out"; $t.Kill() }
    "$flag exit code: $($t.ExitCode)"
    Save-Logs
  }
}

switch ($mode) {
  "tour" {
    # Start the app, let first setup run, then visit every page with its shortcut and take screenshots.
    $app = Start-Process $exe.FullName -PassThru
    Start-Sleep 15; Save-Shot "01-first-launch"
    $deadline = (Get-Date).AddMinutes(20); $n = 0
    while ((Get-Date) -lt $deadline) {
      $ok = $false
      try { $ok = (Invoke-WebRequest "http://localhost:8080/" -UseBasicParsing -TimeoutSec 5).StatusCode -eq 200 } catch {}
      if ($ok) { break }
      $n++; if ($n % 4 -eq 0) { Save-Shot ("02-setup-{0:d2}" -f $n) }
      Start-Sleep 15
    }
    "Setup finished or timed out after $n checks"
    Start-Sleep 10
    Test-Url "http://localhost:8080/"
    Test-Url "http://localhost:8080/phpmyadmin/"
    $pages = [ordered]@{ "^1" = "10-overview"; "^2" = "11-logs"; "^3" = "12-apache"; "^4" = "13-php"; "^5" = "14-mariadb"; "^6" = "15-general" }
    foreach ($k in $pages.Keys) { Send-ToApp "Lampion" $k; Start-Sleep 3; Save-Shot $pages[$k] }
    Send-ToApp "Lampion" "^1"; Start-Sleep 2
    Send-ToApp "Lampion" "^d"; Start-Sleep 8; Save-Shot "20-database-manager"
    Save-Logs
    Stop-Process -Id $app.Id -ErrorAction SilentlyContinue; Start-Sleep 5
    Run-Selftests
    "DONE" | Set-Content (Join-Path $result "done.txt")
  }
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
