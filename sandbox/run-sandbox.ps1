# Starts Lampion in Windows Sandbox (never on the host). Ask the owner before running this.
#   .\sandbox\run-sandbox.ps1 -Installer <path to Lampion-X.Y-setup.exe> [-Mode selftest|interactive]
# The sandbox gets a read-only copy of the installer; results land in C:\Users\akit\lampion-sandbox\out\<time>.
param(
  [Parameter(Mandatory = $true)][string]$Installer,
  [ValidateSet("selftest", "interactive", "tour")][string]$Mode = "selftest"
)
$ErrorActionPreference = "Stop"
if (Get-Process WindowsSandbox, WindowsSandboxClient, WindowsSandboxRemoteSession -ErrorAction SilentlyContinue) {
  throw "A Windows Sandbox is already running. Close it first (only one can run at a time)."
}
if (-not (Test-Path $Installer)) { throw "Installer not found: $Installer" }

$base = "C:\Users\akit\lampion-sandbox"
$in = Join-Path $base "in"
$out = Join-Path $base "out"
New-Item -ItemType Directory -Force $in, $out | Out-Null
Get-ChildItem $in | Remove-Item -Recurse -Force
Copy-Item $Installer $in
Copy-Item (Join-Path $PSScriptRoot "inside.ps1") $in
Set-Content (Join-Path $in "mode.txt") $Mode

$wsb = (Get-Content (Join-Path $PSScriptRoot "lampion-test.wsb") -Raw).Replace("{{IN}}", $in).Replace("{{OUT}}", $out)
$wsbPath = Join-Path $base "lampion-test.wsb"
Set-Content $wsbPath $wsb -Encoding UTF8
Start-Process "$env:windir\System32\WindowsSandbox.exe" -ArgumentList "`"$wsbPath`""
"Sandbox started ($Mode). Results: $out"
