#Requires -Version 5.1
<#
.SYNOPSIS
  Set or verify Roommate app version across package / Cargo / Tauri manifests.

.EXAMPLE
  .\scripts\version.ps1 -Check
  .\scripts\version.ps1 -Set 0.3.0
  .\scripts\version.ps1 -Check -ExpectTag v0.3.0
#>
param(
  [string]$Set = "",
  [switch]$Check,
  [string]$ExpectTag = ""
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot

function Get-SemVerOrThrow([string]$Value, [string]$Label) {
  $v = $Value.Trim()
  if ($v -match '^v(.+)$') { $v = $Matches[1] }
  if ($v -notmatch '^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$') {
    throw "Invalid version for ${Label}: '$Value' (expected X.Y.Z or X.Y.Z-prerelease)"
  }
  return $v
}

function Read-JsonField([string]$Path, [string]$Field) {
  $raw = Get-Content -Raw -Path $Path
  $pattern = '"' + [regex]::Escape($Field) + '"\s*:\s*"([^"]*)"'
  $m = [regex]::Match($raw, $pattern)
  if (-not $m.Success) {
    throw "Could not find `"$Field`" in $Path"
  }
  return $m.Groups[1].Value
}

function Set-JsonFieldFirst([string]$Path, [string]$Field, [string]$Value) {
  $raw = Get-Content -Raw -Path $Path
  $pattern = '("' + [regex]::Escape($Field) + '"\s*:\s*")[^"]*(")'
  $updated = [regex]::Replace($raw, $pattern, '${1}' + $Value + '${2}', 1)
  if ($updated -eq $raw -and (Read-JsonField $Path $Field) -ne $Value) {
    throw "Failed to update $Field in $Path"
  }
  [System.IO.File]::WriteAllText($Path, $updated.TrimEnd() + "`n")
}

function Read-CargoVersion([string]$Path) {
  $lines = Get-Content -Path $Path
  $inPackage = $false
  foreach ($line in $lines) {
    if ($line -match '^\s*\[package\]\s*$') { $inPackage = $true; continue }
    if ($inPackage -and $line -match '^\s*\[') { break }
    if ($inPackage -and $line -match '^\s*version\s*=\s*"([^"]+)"\s*$') {
      return Get-SemVerOrThrow $Matches[1] "Cargo.toml"
    }
  }
  throw "Could not find [package].version in $Path"
}

function Set-CargoVersion([string]$Path, [string]$Version) {
  $lines = [System.IO.File]::ReadAllLines($Path)
  $inPackage = $false

  for ($i = 0; $i -lt $lines.Length; $i++) {
    if ($lines[$i] -match '^\s*\[package\]\s*$') {
      $inPackage = $true
      continue
    }
    if ($inPackage -and $lines[$i] -match '^\s*\[') {
      break
    }
    if ($inPackage -and $lines[$i] -match '^\s*version\s*=') {
      $lines[$i] = 'version = "' + $Version + '"'
      [System.IO.File]::WriteAllText($Path, ($lines -join "`n").TrimEnd() + "`n")
      return
    }
  }

  throw "Failed to update [package].version in $Path"
}

function Sync-PackageLock([string]$Version) {
  $lockPath = Join-Path $Root "package-lock.json"
  if (-not (Test-Path $lockPath)) { return }

  $raw = [System.IO.File]::ReadAllText($lockPath)
  # Fix historical root name drift.
  $raw = [regex]::Replace($raw, '(?m)^(\s*"name"\s*:\s*")client-tmp(")', '${1}roommate${2}')
  $raw = [regex]::Replace($raw, '("packages"\s*:\s*\{\s*""\s*:\s*\{[^}]*?"name"\s*:\s*")client-tmp(")', '${1}roommate${2}', 1)

  # Root version (first "version" after opening brace / name).
  $raw = [regex]::Replace($raw, '(?s)^(\{\s*"name"\s*:\s*"[^"]+"\s*,\s*"version"\s*:\s*")[^"]*(")', '${1}' + $Version + '${2}', 1)
  $raw = [regex]::Replace(
    $raw,
    '("packages"\s*:\s*\{\s*""\s*:\s*\{\s*"name"\s*:\s*"[^"]+"\s*,\s*"version"\s*:\s*")[^"]*(")',
    '${1}' + $Version + '${2}',
    1
  )
  [System.IO.File]::WriteAllText($lockPath, $raw.TrimEnd() + "`n")
}

function Sync-CargoLock([string]$Version) {
  $lockPath = Join-Path $Root "src-tauri\Cargo.lock"
  if (-not (Test-Path $lockPath)) { return }

  $text = [System.IO.File]::ReadAllText($lockPath)
  $pattern = '(name = "roommate"\r?\nversion = ")[^"]*(")'
  $updated = [regex]::Replace($text, $pattern, '${1}' + $Version + '${2}', 1)
  [System.IO.File]::WriteAllText($lockPath, $updated.TrimEnd() + "`n")
}

$pkgPath = Join-Path $Root "package.json"
$tauriPath = Join-Path $Root "src-tauri\tauri.conf.json"
$cargoPath = Join-Path $Root "src-tauri\Cargo.toml"
$lockPath = Join-Path $Root "package-lock.json"

if ($Set) {
  $version = Get-SemVerOrThrow $Set "-Set"
  Set-JsonFieldFirst $pkgPath "version" $version
  Set-JsonFieldFirst $tauriPath "version" $version
  Set-CargoVersion $cargoPath $version
  Sync-PackageLock $version
  Sync-CargoLock $version
  Write-Host "Version set to $version in package.json, tauri.conf.json, Cargo.toml (+ lockfiles)."
}

$pkg = Get-SemVerOrThrow (Read-JsonField $pkgPath "version") "package.json"
$tauri = Get-SemVerOrThrow (Read-JsonField $tauriPath "version") "tauri.conf.json"
$cargo = Read-CargoVersion $cargoPath

Write-Host "package.json:      $pkg"
Write-Host "tauri.conf.json:   $tauri"
Write-Host "Cargo.toml:        $cargo"

if ($pkg -ne $tauri -or $pkg -ne $cargo) {
  throw "Version mismatch across manifests (package=$pkg, tauri=$tauri, cargo=$cargo)"
}

if (Test-Path $lockPath) {
  $lockRaw = [System.IO.File]::ReadAllText($lockPath)
  $lockName = Read-JsonField $lockPath "name"
  $lockVer = Get-SemVerOrThrow (Read-JsonField $lockPath "version") "package-lock.json"
  if ($lockName -ne "roommate") {
    throw "package-lock.json root name is '$lockName', expected 'roommate'"
  }
  if ($lockVer -ne $pkg) {
    throw "package-lock.json root version is $lockVer, expected $pkg"
  }
  if ($lockRaw -notmatch '"packages"\s*:\s*\{\s*""\s*:\s*\{[^}]*"version"\s*:\s*"' + [regex]::Escape($pkg) + '"') {
    throw "package-lock.json packages[''].version does not match $pkg"
  }
  Write-Host "package-lock.json: $lockVer ($lockName)"
}

if ($ExpectTag) {
  $tagVersion = Get-SemVerOrThrow $ExpectTag "ExpectTag"
  if ($tagVersion -ne $pkg) {
    throw "Git tag version $tagVersion does not match app version $pkg"
  }
  Write-Host "Git tag:           v$tagVersion (ok)"
}

Write-Host "Version check passed: $pkg"
