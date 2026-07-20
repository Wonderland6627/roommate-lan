#Requires -Version 5.1
<#
.SYNOPSIS
  Smoke-check Roommate package artifacts after `npm run tauri build`.
#>
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Bundle = Join-Path $Root "src-tauri\target\release\bundle"

if (-not (Test-Path $Bundle)) {
  Write-Error "Bundle folder missing: $Bundle — run npm run tauri build first."
}

Write-Host "Scanning $Bundle ..."
$nsis = Get-ChildItem -Path $Bundle -Recurse -Filter "*.exe" -ErrorAction SilentlyContinue
if (-not $nsis) {
  Write-Error "No NSIS/exe artifacts found under bundle/"
}

$nsis | ForEach-Object {
  Write-Host ("OK artifact: {0} ({1:N1} MB)" -f $_.FullName, ($_.Length / 1MB))
}

$releaseDir = Join-Path $Root "src-tauri\target\release"
$sidecars = @("tailscale.exe", "tailscaled.exe") | ForEach-Object {
  Get-ChildItem -Path $releaseDir -Filter $_ -ErrorAction SilentlyContinue
}
if ($sidecars.Count -lt 2) {
  Write-Warning "Sidecar exes not found next to release binary — verify externalBin bundling."
} else {
  $sidecars | ForEach-Object { Write-Host "OK sidecar: $($_.Name)" }
}

Write-Host "package-check passed."
