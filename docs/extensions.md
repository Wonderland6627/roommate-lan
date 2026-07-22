# Phase 6 扩展功能规划

实施顺序：**B（进行中）→ C → A**。

## 扩展 B：动态房间口令 / AuthKey

**目标**：房主生成短期口令，队友输入后自动换取 preauth key 并 `tailscale up`。

### 已落地（MVP）

- [`infra/room-api/`](../infra/room-api/)：创建 / 列表 / 加入 / 成员 / 退出 / 解散 + TTL
- 4 位 A–Z 短码（校验忽略大小写）；创建时填房间名与显示名
- 公开 Release 只内嵌 Login Server；AuthKey 由 Room API 短期签发
- 客户端：大厅列表 + 创建/加入；房内成员显示名；房主解散 / 队友退出
- **弱化生命周期**：无心跳；强杀残留成员靠房间 TTL 回收

### 后续（未做）

- 二维码 / `roommate://join?code=`
- 成员心跳 / 对照 Headscale 在线清理幽灵成员
- 网络 ACL「仅房主可…」

**依赖**：Headscale API key、HTTPS、限流

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
