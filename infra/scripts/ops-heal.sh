#!/usr/bin/env bash
# Roommate 宝塔运维：检查 / 清理 Headscale 幽灵节点 / 重启异常服务。
#
# 用法（在 infra 目录，或任意处调用本脚本）：
#   ./scripts/ops-heal.sh              # 等同 heal：检查 + 清离线节点 + 重启不健康容器
#   ./scripts/ops-heal.sh status       # 只看状态
#   ./scripts/ops-heal.sh purge        # 只删 offline 节点
#   ./scripts/ops-heal.sh purge --all  # 删掉所有 offline（忽略时长）
#   ./scripts/ops-heal.sh restart      # 重启 headscale + room-api
#   ./scripts/ops-heal.sh rebuild      # 重建并启动 room-api（上传代码后）
#   ./scripts/ops-heal.sh logs         # 跟随 room-api / headscale 日志
#   ./scripts/ops-heal.sh menu         # 交互菜单
#
# 环境变量：
#   OFFLINE_MINUTES  默认 15；purge 时 Last seen 超过该分钟才删（--all 除外）
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

OFFLINE_MINUTES="${OFFLINE_MINUTES:-15}"
COMPOSE=(docker compose)
if [[ -f docker-compose.baota.yml ]]; then
  COMPOSE=(docker compose -f docker-compose.yml -f docker-compose.baota.yml)
fi

dc() { "${COMPOSE[@]}" "$@"; }

say() { printf '\n==> %s\n' "$*"; }
ok() { printf '  [ok] %s\n' "$*"; }
warn() { printf '  [!] %s\n' "$*"; }
fail() { printf '  [x] %s\n' "$*"; }

need_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    echo "未找到 docker，请先在宝塔安装 Docker。"
    exit 1
  fi
  if ! docker info >/dev/null 2>&1; then
    echo "无法连接 Docker daemon（权限或服务未启动）。"
    exit 1
  fi
}

container_running() {
  local name="$1"
  docker ps --format '{{.Names}}' | grep -qx "$name"
}

http_ok() {
  local url="$1"
  curl -fsS --max-time 5 "$url" >/dev/null 2>&1
}

cmd_status() {
  say "Compose 服务"
  dc ps || true

  say "HTTP 探测"
  if http_ok "http://127.0.0.1:8080/health"; then
    ok "headscale http://127.0.0.1:8080/health"
  else
    fail "headscale 本机 8080 无响应"
  fi
  if http_ok "http://127.0.0.1:8081/health"; then
    ok "room-api http://127.0.0.1:8081/health"
  else
    fail "room-api 本机 8081 无响应"
  fi

  if container_running roommate-headscale; then
    say "Headscale 节点"
    dc exec -T headscale headscale nodes list 2>/dev/null || warn "nodes list 失败"
    local offline
    offline="$(list_offline_node_ids || true)"
    if [[ -z "${offline}" ]]; then
      ok "无 offline 节点"
    else
      warn "offline 节点 ID: $(echo "$offline" | tr '\n' ' ')"
    fi
  else
    fail "容器 roommate-headscale 未运行"
  fi

  if container_running roommate-room-api; then
    say "room-api 最近日志（含 error/warn/purge）"
    dc logs --tail=80 room-api 2>/dev/null | grep -Ei 'error|warn|purge|node_gc|failed|started' || true
  fi
}

# 解析 `headscale nodes list` 表：Connected 列为 offline 的 ID。
list_offline_node_ids() {
  dc exec -T headscale headscale nodes list 2>/dev/null \
    | awk -F'|' '
        NR<=2 { next }
        {
          id=$1
          gsub(/^[ \t]+|[ \t]+$/, "", id)
          if (id !~ /^[0-9]+$/) next
          connected=$(NF-1)
          gsub(/^[ \t]+|[ \t]+$/, "", connected)
          if (tolower(connected) ~ /offline|false|no/) {
            # NF-1 在部分版本是 Expired；再扫整行
          }
        }
        tolower($0) ~ /offline/ {
          id=$1
          gsub(/^[ \t]+|[ \t]+$/, "", id)
          if (id ~ /^[0-9]+$/) print id
        }
      ' \
    | sort -u
}

# 带 Last seen 过滤：尽量解析表中的 Last seen 列（UTC）。
# --all 时跳过时间判断。
list_stale_offline_ids() {
  local all="${1:-}"
  if [[ "$all" == "--all" ]]; then
    list_offline_node_ids
    return 0
  fi

  # 用 headscale 输出 + date 判断；解析失败则保守：只报告、不删（由 --all 强制）。
  local now_epoch cutoff_epoch
  now_epoch="$(date -u +%s)"
  cutoff_epoch=$((now_epoch - OFFLINE_MINUTES * 60))

  dc exec -T headscale headscale nodes list 2>/dev/null \
    | awk -F'|' -v cutoff="$cutoff_epoch" '
        NR<=2 { next }
        {
          line=$0
          if (tolower(line) !~ /offline/) next
          id=$1
          gsub(/^[ \t]+|[ \t]+$/, "", id)
          if (id !~ /^[0-9]+$/) next
          # 典型列：... | Last seen | Expiration | Connected | Expired
          # Last seen 约在倒数第 4 列
          n=split(line, a, "|")
          seen=""
          for (i=1; i<=n; i++) {
            gsub(/^[ \t]+|[ \t]+$/, "", a[i])
            if (a[i] ~ /^[0-9]{4}-[0-9]{2}-[0-9]{2}/) {
              seen=a[i]
              break
            }
          }
          print id "\t" seen
        }
      ' \
    | while IFS=$'\t' read -r id seen; do
        [[ -z "$id" ]] && continue
        if [[ -z "$seen" || "$seen" == 0001-* ]]; then
          echo "$id"
          continue
        fi
        # "2026-07-22 15:30:05" → epoch（按 UTC）
        seen_epoch="$(date -u -d "$seen" +%s 2>/dev/null || date -u -j -f "%Y-%m-%d %H:%M:%S" "$seen" +%s 2>/dev/null || echo 0)"
        if [[ "$seen_epoch" -eq 0 || "$seen_epoch" -lt "$cutoff_epoch" ]]; then
          echo "$id"
        fi
      done
}

delete_node_ids() {
  local ids=("$@")
  if [[ ${#ids[@]} -eq 0 ]]; then
    ok "无需删除节点"
    return 0
  fi
  local id
  for id in "${ids[@]}"; do
    if dc exec -T headscale headscale nodes delete -i "$id" --force >/dev/null 2>&1; then
      ok "已删除节点 $id"
    else
      warn "删除节点 $id 失败（可能已不存在）"
    fi
  done
}

cmd_purge() {
  local mode="${1:-}"
  say "清理 offline Headscale 节点${mode:+ ($mode)}（阈值 ${OFFLINE_MINUTES} 分钟）"
  if ! container_running roommate-headscale; then
    fail "headscale 未运行，跳过 purge"
    return 1
  fi
  mapfile -t ids < <(list_stale_offline_ids "$mode" || true)
  if [[ ${#ids[@]} -eq 0 ]]; then
    ok "没有需要清理的 offline 节点"
    return 0
  fi
  warn "将删除: ${ids[*]}"
  delete_node_ids "${ids[@]}"
  say "清理后节点列表"
  dc exec -T headscale headscale nodes list 2>/dev/null || true
}

cmd_restart() {
  say "重启 headscale + room-api"
  dc up -d headscale room-api
  dc restart headscale room-api
  sleep 3
  cmd_status
}

cmd_rebuild() {
  say "重建 room-api（上传代码后用）"
  dc up -d --build room-api
  sleep 2
  cmd_status
}

cmd_logs() {
  say "跟随日志（Ctrl+C 退出）"
  dc logs -f --tail=100 room-api headscale
}

heal_unhealthy() {
  local need_restart=0
  if ! container_running roommate-headscale || ! http_ok "http://127.0.0.1:8080/health"; then
    warn "headscale 异常，将重启"
    need_restart=1
  fi
  if ! container_running roommate-room-api || ! http_ok "http://127.0.0.1:8081/health"; then
    warn "room-api 异常，将重启"
    need_restart=1
  fi
  if [[ "$need_restart" -eq 1 ]]; then
    dc up -d headscale room-api
    dc restart headscale room-api || true
    sleep 4
  else
    ok "容器与本机端口健康，无需重启"
  fi
}

cmd_heal() {
  say "一键修复：状态 → 清幽灵节点 → 必要时重启"
  cmd_status || true
  cmd_purge || true
  heal_unhealthy
  say "修复后复查"
  if http_ok "http://127.0.0.1:8080/health"; then ok "headscale OK"; else fail "headscale 仍异常"; fi
  if http_ok "http://127.0.0.1:8081/health"; then ok "room-api OK"; else fail "room-api 仍异常"; fi
  if container_running roommate-headscale; then
    dc exec -T headscale headscale nodes list 2>/dev/null || true
  fi
  say "完成"
}

cmd_menu() {
  while true; do
    cat <<EOF

Roommate 运维菜单  ($ROOT)
  1) 一键修复 heal
  2) 只看状态 status
  3) 清 offline 节点（>${OFFLINE_MINUTES}m）
  4) 清全部 offline 节点
  5) 重启 headscale + room-api
  6) 重建 room-api
  7) 看日志
  0) 退出
EOF
    read -r -p "选择: " choice
    case "$choice" in
      1) cmd_heal ;;
      2) cmd_status ;;
      3) cmd_purge ;;
      4) cmd_purge --all ;;
      5) cmd_restart ;;
      6) cmd_rebuild ;;
      7) cmd_logs ;;
      0) exit 0 ;;
      *) warn "无效选项" ;;
    esac
  done
}

usage() {
  sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'
}

main() {
  need_docker
  local cmd="${1:-heal}"
  shift || true
  case "$cmd" in
    heal|"") cmd_heal "$@" ;;
    status) cmd_status "$@" ;;
    purge) cmd_purge "${1:-}" ;;
    restart) cmd_restart "$@" ;;
    rebuild) cmd_rebuild "$@" ;;
    logs) cmd_logs "$@" ;;
    menu) cmd_menu "$@" ;;
    -h|--help|help) usage ;;
    *)
      echo "未知命令: $cmd"
      usage
      exit 1
      ;;
  esac
}

main "$@"
