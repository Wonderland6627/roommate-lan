# Roommate MVP 测试矩阵与打包验收

## 前置

1. 完成 [`infra/`](../infra/README.md) 部署：Headscale + Room API（`HEADSCALE_API_KEY`）+ Caddy/宝塔反代。
2. 根目录复制 `.env.example` → `.env`，至少填入 `ROOMMATE_LOGIN_SERVER`（公开包由 CI 注入；开发机可写本地 `.env`）。
3. 拉取 Sidecar（含 `wintun.dll`）：`npm run fetch-bins`
4. 单元测试：`npm run test:rust`
5. 若本机有官方 Tailscale：**先退出/停止服务**（`net stop Tailscale`）。
6. 开发机：管理员执行一次 `npm run dev-service`，确认 `sc query RoommateNetworkService` 为 RUNNING。

## 功能验收矩阵

| 场景 | 步骤 | 期望 |
|------|------|------|
| 创建房间 | 填显示名+房间名 → 创建 | 得 4 位大写码；隧道 Running；成员列表含自己（房主） |
| 加入房间 | 另一台填显示名+房间码（大小写皆可）→ 加入 | 进网成功；房内成员列表双方可见显示名 |
| 房间列表 | 大厅刷新 | 可见房间名与人数；**不下发短码**；点选后仍须手输码 |
| 退出 / 解散 | 队友退出或房主解散 | 先 Room API leave/dissolve 再 disconnect；空房从列表消失 |
| TTL | 等待房间过期或调低 `ROOM_TTL_HOURS` | 过期房间从列表消失 |
| 无闪烁连接 | 普通用户创建/加入 | **同一窗口/同一 PID**；无 UAC；无黑控制台（release）；有 `100.64.x.x` |
| 同局域网 Easy NAT | 两台机器进同一房间，互 ping | 徽章多为 **P2P 直连**，RTT 较低 |
| 跨运营商 / 对称 NAT | 异地或防火墙禁 UDP | 徽章为 **腾讯云 DERP**，仍可 ping / 进房 |
| 仅一人在线 | 单客户端进房 | Peer 列表空态文案，无崩溃；房间成员仍显示自己 |
| 杀软拦截 TUN | 模拟拦截后连接 | UI 显示中文错误；日志见 `%ProgramData%\Roommate-LAN\logs\tailscaled.log` |
| 官方 Tailscale 冲突 | 官方服务 RUNNING 时点连接 | 立即中文报错，要求先退出官方 |
| 服务未就绪 | 停止 `RoommateNetworkService` 后创建/加入 | 提示网络服务未就绪 / 修复安装 |
| 错码 / 限流 | 连续输错码 | 401；频繁后 429 |
| 退出清理 | 进房后退出 App | 30s 内租约过期或立即 Disconnect；sidecar 停止；服务进程仍 Running；房间成员可能残留至 TTL（弱化版） |
| 强杀 GUI | 任务管理器结束 Roommate | ≤30s 后服务自动 down sidecar |
| 复制 IP / 房间码 | 点击队友 IP；房内复制码 | 剪贴板内容正确 |
| 私有 pipe | 连接成功后 | 存在 `\\.\pipe\ProtectedPrefix\Administrators\Roommate\tailscaled`；服务 IPC 为 `\\.\pipe\Roommate\NetworkService` |

## 打包

```powershell
npm run version:check
npm run fetch-bins
npm run tauri build
npm run package-check
```

产物：`src-tauri/target/release/bundle/nsis/`（含 `*-setup.exe`；正式发版还应有 `.sig` 与 `latest.json`）

**GitHub Release：** 推送 `vX.Y.Z` 标签触发 [`.github/workflows/release.yml`](../.github/workflows/release.yml)。公开包不内嵌 `ROOMMATE_AUTH_KEY`；须内嵌/注入 `ROOMMATE_LOGIN_SERVER`。

**门禁：**

1. 全新 Windows 安装 NSIS：**仅安装时**出现一次 UAC；重启后 `RoommateNetworkService` 自动 RUNNING。
2. 普通用户启动 App：**无需 `.env`**，创建或加入房间即可进网（依赖已部署 Room API）。
3. 覆盖升级 / 卸载后无残留服务、进程、Roommate 网卡。
4. 应用内「检查更新」能发现新版、下载签名包并完成安装重启（升级时可能再弹 UAC）。
5. 底部版本号与 `tauri.conf.json` / Git tag 一致。

## Steam Listen 冒烟

1. 双方进同一 Roommate 房间，复制房主 `100.64.0.x`。
2. 游戏内 Listen / Direct IP 填虚拟 IP（依赖真实 Wintun TUN）。
3. 观察卡顿与丢包是否优于公网直连。
