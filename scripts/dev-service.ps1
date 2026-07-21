#Requires -Version 5.1
<#
.SYNOPSIS
  Install / restart the Roommate network Windows service for local development.

.DESCRIPTION
  Always rebuilds release roommate.exe, registers RoommateNetworkService (LocalSystem),
  and starts it. Run from an elevated PowerShell once, then use npm run tauri dev
  as a normal user.

.PARAMETER Action
  install | restart | stop | status | uninstall
#>
param(
  [ValidateSet("install", "restart", "stop", "status", "uninstall")]
  [string]$Action = "install"
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$ServiceName = "RoommateNetworkService"
$DisplayName = "Roommate Network Service"
$Exe = Join-Path $Root "src-tauri\target\release\roommate.exe"

function Get-BinaryPathName {
  # CreateService BinaryPathName: quote exe only when path contains spaces.
  if ($Exe -match '\s') {
    return '"{0}" --roommate-service' -f $Exe
  }
  return '{0} --roommate-service' -f $Exe
}

function Assert-Admin {
  $id = [Security.Principal.WindowsIdentity]::GetCurrent()
  $p = New-Object Security.Principal.WindowsPrincipal($id)
  if (-not $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "Please run this script as Administrator."
  }
}

function Build-Release {
  Write-Host "Building release binary (required for service host)..."
  Push-Location $Root
  try {
    if (-not (Test-Path (Join-Path $Root "src-tauri\binaries\wintun.dll"))) {
      npm run fetch-bins
    }
    cargo build --release --manifest-path (Join-Path $Root "src-tauri\Cargo.toml")
    if ($LASTEXITCODE -ne 0) { throw "cargo build --release failed: $LASTEXITCODE" }
  } finally {
    Pop-Location
  }
  if (-not (Test-Path $Exe)) {
    throw "Release binary missing after build: $Exe"
  }
  # Ensure sidecars sit next to the service host.
  $binDir = Join-Path $Root "src-tauri\binaries"
  $triple = "x86_64-pc-windows-msvc"
  $releaseDir = Split-Path $Exe -Parent
  foreach ($name in @("tailscale", "tailscaled")) {
    $src = Join-Path $binDir "$name-$triple.exe"
    $dst = Join-Path $releaseDir "$name.exe"
    if (Test-Path $src) { Copy-Item -Force $src $dst }
  }
  $wintunSrc = Join-Path $binDir "wintun.dll"
  if (Test-Path $wintunSrc) {
    Copy-Item -Force $wintunSrc (Join-Path $releaseDir "wintun.dll")
  }
}

function Get-ServiceState {
  $svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
  if (-not $svc) { return "MISSING" }
  return $svc.Status.ToString().ToUpperInvariant()
}

function Stop-RoommateServiceHost {
  # Must stop before cargo rebuild: the running service locks roommate.exe.
  $svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
  if (-not $svc) { return }
  if ($svc.Status -eq "Stopped") { return }
  Write-Host "Stopping $ServiceName so roommate.exe can be rebuilt..."
  Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
  $deadline = (Get-Date).AddSeconds(25)
  do {
    Start-Sleep -Milliseconds 400
    $svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
  } while ($svc -and $svc.Status -ne "Stopped" -and (Get-Date) -lt $deadline)
  # Extra beat for handle release on Windows.
  Start-Sleep -Seconds 1
  if ($svc -and $svc.Status -ne "Stopped") {
    throw "Could not stop $ServiceName; close handles and retry."
  }
}

function Remove-RoommateService {
  Stop-RoommateServiceHost
  $svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
  if (-not $svc) { return }
  & sc.exe delete $ServiceName | Out-Null
  Start-Sleep -Seconds 1
}

function Install-RoommateService {
  # Stop first (unlocks exe), rebuild, then recreate + start.
  Stop-RoommateServiceHost
  Build-Release
  $BinaryPathName = Get-BinaryPathName
  Write-Host "BinaryPathName: $BinaryPathName"

  Remove-RoommateService

  New-Service `
    -Name $ServiceName `
    -BinaryPathName $BinaryPathName `
    -DisplayName $DisplayName `
    -StartupType Automatic `
    -ErrorAction Stop | Out-Null

  & sc.exe description $ServiceName "Hosts Roommate private Tailscale sidecar for Steam LAN." | Out-Null
  & sc.exe failure $ServiceName reset= 86400 actions= restart/5000/restart/10000/restart/30000 | Out-Null

  Start-Service -Name $ServiceName -ErrorAction Stop

  $deadline = (Get-Date).AddSeconds(15)
  do {
    Start-Sleep -Milliseconds 500
    $state = Get-ServiceState
  } while ($state -ne "RUNNING" -and (Get-Date) -lt $deadline)

  Write-Host ("Service state: " + (Get-ServiceState))
  Write-Host "--- sc qc ---"
  & sc.exe qc $ServiceName
  $errLog = Join-Path $env:ProgramData "Roommate-LAN\logs\service-start-error.txt"
  if (Test-Path $errLog) {
    Write-Host "--- service-start-error.txt ---"
    Get-Content $errLog
  }
  if ((Get-ServiceState) -ne "RUNNING") {
    throw "Service failed to reach RUNNING. Check Event Viewer or $errLog"
  }
  Write-Host "Done. Now run: npm run tauri dev (as a normal user, no UAC)"
}

Assert-Admin

switch ($Action) {
  "status" {
    Write-Host ("Service state: " + (Get-ServiceState))
    Get-Service -Name $ServiceName -ErrorAction SilentlyContinue | Format-List Name, Status, StartType
    & sc.exe qc $ServiceName
  }
  "stop" {
    Stop-RoommateServiceHost
    Write-Host ("Service state: " + (Get-ServiceState))
  }
  "uninstall" {
    Remove-RoommateService
    Write-Host ("Service state: " + (Get-ServiceState))
  }
  "restart" {
    $state = Get-ServiceState
    if ($state -eq "MISSING") {
      Install-RoommateService
    } else {
      Stop-RoommateServiceHost
      Build-Release
      Start-Service -Name $ServiceName -ErrorAction Stop
      Write-Host ("Service state: " + (Get-ServiceState))
      & sc.exe qc $ServiceName
    }
  }
  "install" {
    Install-RoommateService
  }
}
