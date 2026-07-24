# Roommate-LAN peer / tunnel diagnostics for end users.
# Usage:
#   powershell -ExecutionPolicy Bypass -File diagnose.ps1
#   powershell -ExecutionPolicy Bypass -File diagnose.ps1 100.64.0.2
#
# Must use the same LocalAPI pipe as RoommateNetworkService (see config.rs tailscaled_socket).

param(
  [Parameter(Position = 0)]
  [string]$PeerIp = ""
)

$ErrorActionPreference = "Continue"
$Socket = "\\.\pipe\ProtectedPrefix\Administrators\Roommate\tailscaled"
$ServiceName = "RoommateNetworkService"
$script:failed = $false

function Write-Ok([string]$msg) { Write-Host "[OK]  $msg" -ForegroundColor Green }
function Write-Fail([string]$msg) {
  Write-Host "[FAIL] $msg" -ForegroundColor Red
  $script:failed = $true
}
function Write-Info([string]$msg) { Write-Host "[..]  $msg" -ForegroundColor Gray }

function Find-TailscaleExe {
  $dirs = @(
    $PSScriptRoot,
    (Split-Path -Parent $PSScriptRoot)
  )
  if ($PSScriptRoot) {
    $dirs += (Join-Path $PSScriptRoot "resources")
    $parent = Split-Path -Parent $PSScriptRoot
    if ($parent) {
      $dirs += (Join-Path $parent "resources")
    }
  }

  foreach ($dir in $dirs) {
    if (-not $dir) { continue }
    if (-not (Test-Path -LiteralPath $dir)) { continue }
    foreach ($rel in @("tailscale.exe", "binaries\tailscale.exe")) {
      $p = Join-Path $dir $rel
      if (Test-Path -LiteralPath $p) {
        return $dir, $p
      }
    }
  }
  return $null, $null
}

function Invoke-RoommateTailscale {
  param(
    [Parameter(Mandatory = $true)][string]$TailscaleExe,
    [Parameter(Mandatory = $true)][string[]]$CliArgs
  )
  $all = @("--socket", $Socket) + $CliArgs
  $output = & $TailscaleExe @all 2>&1
  $code = $LASTEXITCODE
  [pscustomobject]@{
    Code   = $code
    Output = $output
  }
}

Write-Host ""
Write-Host "Roommate-LAN 连通诊断" -ForegroundColor Cyan
Write-Host "===================="
Write-Host ""

$installDir, $tailscale = Find-TailscaleExe
if (-not $tailscale) {
  Write-Fail "找不到 tailscale.exe（请把本脚本放在 Roommate-LAN 安装目录下运行）"
  Write-Info "常见路径: C:\Program Files\Roommate-LAN\"
  exit 1
}
Write-Ok "找到 CLI: $tailscale"
if ($installDir) {
  Write-Info "安装目录: $installDir"
}

$svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
if (-not $svc) {
  Write-Fail "未找到 Windows 服务 $ServiceName（安装可能不完整）"
} elseif ($svc.Status -ne "Running") {
  Write-Fail "服务 $ServiceName 状态为 $($svc.Status)（应为 Running）"
} else {
  Write-Ok "服务 $ServiceName 正在运行"
}

Write-Host ""
Write-Info "执行: tailscale status"
$status = Invoke-RoommateTailscale -TailscaleExe $tailscale -CliArgs @("status")
if ($status.Output) {
  $status.Output | ForEach-Object { Write-Host $_ }
}
if ($status.Code -ne 0) {
  Write-Fail "tailscale status 失败 (exit $($status.Code))。请确认已创建/加入房间且隧道已连接。"
} else {
  Write-Ok "tailscale status 成功"
}

if ($PeerIp.Trim().Length -gt 0) {
  $ip = $PeerIp.Trim()
  Write-Host ""
  Write-Info "执行: tailscale ping -c 1 $ip"
  $ping = Invoke-RoommateTailscale -TailscaleExe $tailscale -CliArgs @("ping", "-c", "1", $ip)
  if ($ping.Output) {
    $ping.Output | ForEach-Object { Write-Host $_ }
  }
  if ($ping.Code -ne 0) {
    Write-Fail "ping $ip 失败 (exit $($ping.Code))"
  } else {
    Write-Ok "ping $ip 成功"
  }
} else {
  Write-Host ""
  Write-Info "未指定对方虚拟 IP。可从 App 成员列表复制 100.x 后重试:"
  Write-Info "  powershell -ExecutionPolicy Bypass -File `"$PSCommandPath`" 100.64.0.2"
}

Write-Host ""
if ($script:failed) {
  Write-Host "诊断结果: 存在失败项" -ForegroundColor Red
  exit 1
}
Write-Host "诊断结果: 通过" -ForegroundColor Green
exit 0
