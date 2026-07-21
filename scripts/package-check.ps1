#Requires -Version 5.1
<#
.SYNOPSIS
  Smoke-check Roommate package artifacts after `npm run tauri build`.
#>
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Bundle = Join-Path $Root "src-tauri\target\release\bundle"
$NsisDir = Join-Path $Bundle "nsis"

if (-not (Test-Path $Bundle)) {
  Write-Error "Bundle folder missing: $Bundle - run npm run tauri build first."
}

Write-Host "Scanning $Bundle ..."

if (-not (Test-Path $NsisDir)) {
  Write-Error "NSIS folder missing: $NsisDir"
}

$nsis = Get-ChildItem -Path $NsisDir -Filter "*-setup.exe" -ErrorAction SilentlyContinue |
  Where-Object { $_.Name -notlike "*.sig" }
if (-not $nsis) {
  Write-Error "No NSIS setup.exe artifacts found under $NsisDir"
}

$nsis | ForEach-Object {
  Write-Host ("OK artifact: {0} ({1:N1} MB)" -f $_.FullName, ($_.Length / 1MB))
  $sig = "$($_.FullName).sig"
  if (Test-Path $sig) {
    Write-Host "OK signature: $sig"
  } else {
    Write-Warning "Updater signature missing: $sig (set TAURI_SIGNING_PRIVATE_KEY when building releases)"
  }
}

$releaseDir = Join-Path $Root "src-tauri\target\release"
$sidecars = @("tailscale.exe", "tailscaled.exe") | ForEach-Object {
  Get-ChildItem -Path $releaseDir -Filter $_ -ErrorAction SilentlyContinue
}
if ($sidecars.Count -lt 2) {
  Write-Error "Sidecar exes not found next to release binary - verify externalBin bundling."
} else {
  $sidecars | ForEach-Object { Write-Host "OK sidecar: $($_.Name)" }
}

$wintun = Get-ChildItem -Path $releaseDir -Filter "wintun.dll" -ErrorAction SilentlyContinue |
  Select-Object -First 1
if (-not $wintun) {
  Write-Error "wintun.dll missing from release output - ensure bundle.resources includes it and rebuild."
}
Write-Host "OK wintun: $($wintun.FullName)"

$hooks = Join-Path $Root "src-tauri\windows\hooks.nsh"
if (-not (Test-Path $hooks)) {
  Write-Error "NSIS hooks missing: $hooks"
}
Write-Host "OK nsis hooks: $hooks"

$releaseExe = Join-Path $releaseDir "roommate.exe"
if (Test-Path $releaseExe) {
  Write-Host "OK release exe (service host): $releaseExe"
  Write-Host "  Service mode flag: --roommate-service"
} else {
  Write-Warning "release roommate.exe not found - expected after tauri build."
}

Write-Host "package-check passed."
Write-Host "Post-install check: sc query RoommateNetworkService  (expect RUNNING)"
