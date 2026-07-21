#Requires -Version 5.1
<#
.SYNOPSIS
  Download Tailscale Windows binaries and rename for Tauri externalBin.

.DESCRIPTION
  Fetches the official Tailscale MSI, extracts tailscale.exe / tailscaled.exe /
  wintun.dll, and places them under src-tauri/binaries/ with the Rust
  target-triple suffix (exe) or plain name (wintun.dll).
#>
param(
  [string]$Version = "1.82.0",
  [string]$OutDir = "",
  # Override pinned hash if needed. Empty uses KnownHashes[$Version].
  [string]$ExpectedSha256 = "",
  [switch]$SkipHashCheck
)

$ErrorActionPreference = "Stop"

# Verified MSI hashes (amd64). Update this map when bumping -Version.
$KnownHashes = @{
  "1.82.0" = "32B8AD3CA2202D090BEBE8E2D8108BCDD8C9371D4840EBF7C03288A9AF133715"
}

$Root = Split-Path -Parent $PSScriptRoot
if (-not $OutDir) {
  $OutDir = Join-Path $Root "src-tauri\binaries"
}

$Triple = "x86_64-pc-windows-msvc"
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$Work = Join-Path $env:TEMP "roommate-tailscale-$Version"
if (Test-Path $Work) { Remove-Item -Recurse -Force $Work }
New-Item -ItemType Directory -Force -Path $Work | Out-Null

$MsiUrl = "https://pkgs.tailscale.com/stable/tailscale-setup-$Version-amd64.msi"
$MsiPath = Join-Path $Work "tailscale.msi"
$Extract = Join-Path $Work "extract"

Write-Host "Downloading $MsiUrl ..."
Invoke-WebRequest -Uri $MsiUrl -OutFile $MsiPath -UseBasicParsing

$hash = (Get-FileHash -Algorithm SHA256 -Path $MsiPath).Hash.ToUpperInvariant()
Write-Host "MSI SHA-256: $hash"

$expected = if ($ExpectedSha256) {
  $ExpectedSha256.ToUpperInvariant()
} elseif ($KnownHashes.ContainsKey($Version)) {
  $KnownHashes[$Version].ToUpperInvariant()
} else {
  ""
}

if (-not $SkipHashCheck) {
  if ([string]::IsNullOrWhiteSpace($expected)) {
    throw "No pinned SHA-256 for Tailscale $Version. Pass -ExpectedSha256 or update KnownHashes."
  }
  if ($hash -ne $expected) {
    throw "Tailscale MSI hash mismatch for $Version. Expected $expected, got $hash"
  }
  Write-Host "MSI hash verified."
} else {
  Write-Warning "SkipHashCheck enabled - not verifying MSI integrity."
}

Write-Host "Extracting MSI (administrative install) ..."
New-Item -ItemType Directory -Force -Path $Extract | Out-Null
$p = Start-Process -FilePath "msiexec.exe" -ArgumentList @(
  "/a", "`"$MsiPath`"",
  "/qn",
  "TARGETDIR=`"$Extract`""
) -Wait -PassThru
if ($p.ExitCode -ne 0) {
  throw "msiexec failed with exit $($p.ExitCode)"
}

$found = Get-ChildItem -Path $Extract -Recurse -Include "tailscale.exe", "tailscaled.exe", "wintun.dll" -ErrorAction SilentlyContinue
$ts = $found | Where-Object { $_.Name -eq "tailscale.exe" } | Select-Object -First 1
$td = $found | Where-Object { $_.Name -eq "tailscaled.exe" } | Select-Object -First 1
$wt = $found | Where-Object { $_.Name -eq "wintun.dll" } | Select-Object -First 1

if (-not $ts -or -not $td) {
  throw "Could not find tailscale.exe / tailscaled.exe inside MSI extract tree under $Extract"
}
if (-not $wt) {
  throw "Could not find wintun.dll inside MSI extract tree under $Extract"
}

$destTs = Join-Path $OutDir "tailscale-$Triple.exe"
$destTd = Join-Path $OutDir "tailscaled-$Triple.exe"
$destWt = Join-Path $OutDir "wintun.dll"
Copy-Item -Force $ts.FullName $destTs
Copy-Item -Force $td.FullName $destTd
Copy-Item -Force $wt.FullName $destWt

Write-Host "Wrote:"
Write-Host "  $destTs"
Write-Host "  $destTd"
Write-Host "  $destWt"
Write-Host "Done. Sidecar version pinned: $Version"
