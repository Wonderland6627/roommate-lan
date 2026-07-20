# Phase 6 扩展功能规划

实施顺序：**B → C → A**。

## 扩展 B：动态房间口令 / AuthKey

**目标**：房主生成短期口令或二维码，队友输入后自动换取 preauth key 并 `tailscale up`。

**后端（建议与 Headscale 同机，Caddy 反代）**

- `POST /api/rooms` → 创建房间，调用 `headscale preauthkeys create --expiration 2h`
- `POST /api/rooms/join` → `{ code }` 校验后返回 `{ loginServer, authKey }`
- 口令存储：SQLite + TTL；用完可作废（非 reusable）

**客户端**

- 连接前增加「口令」输入；覆盖编译期默认 AuthKey
- 二维码：口令字符串或 `roommate://join?code=`

**依赖**：Headscale API/CLI 权限、HTTPS、防爆破（rate limit）

---

## 扩展 C：Steam 快速唤起

**目标**：列表项或游戏快捷入口触发 `steam://`。

- `steam://run/<appid>` 启动游戏
- 可选：展示常用 AppID 配置（本地 JSON）
- 用 `@tauri-apps/plugin-opener` / `shell open` 打开协议 URL

**依赖**：用户已安装 Steam；部分游戏仍需手动填 Listen IP

---

## 扩展 A：网络诊断工具箱

**目标**：一键查看防火墙配置文件、UPnP/NAT-PMP、网卡状态；可选「重置网卡」。

**实现要点**

- PowerShell：`Get-NetFirewallProfile`、`Get-NetAdapter`
- UPnP：可选 COM / 第三方库（注意权限与稳定性）
- 重置：`netsh int ip reset` 等 — **必须二次确认**，并单独 capability

**依赖**：管理员权限、清晰的风险文案
