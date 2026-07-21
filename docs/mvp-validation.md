# Roommate MVP 测试矩阵与打包验收

## 前置

1. 完成 [`infra/`](../infra/README.md) 部署，拿到可复用 AuthKey。
2. 根目录复制 `.env.example` → `.env`，填入真实 `ROOMMATE_LOGIN_SERVER` / `ROOMMATE_AUTH_KEY`。
3. 拉取 Sidecar（含 `wintun.dll`）：`npm run fetch-bins`
4. 单元测试：`npm run test:rust`
5. 若本机有官方 Tailscale：**先退出/停止服务**（`net stop Tailscale`）。
6. 开发机：管理员执行一次 `npm run dev-service`，确认 `sc query RoommateNetworkService` 为 RUNNING。

## 功能验收矩阵

| 场景 | 步骤 | 期望 |
|------|------|------|
| 无闪烁连接 | 普通用户启动 App → 一键连接 | **同一窗口/同一 PID**；无 UAC；无黑控制台（release）；状态 Running 且有 `100.64.x.x` |
| 同局域网 Easy NAT | 两台机器一键连接，互 ping | 徽章多为 **P2P 直连**，RTT 较低 |
| 跨运营商 / 对称 NAT | 异地或防火墙禁 UDP | 徽章为 **腾讯云 DERP**，仍可 ping / 进房 |
| 仅一人在线 | 单客户端连接 | 列表空态文案，无崩溃 |
| 杀软拦截 TUN | 模拟拦截后连接 | UI 显示中文错误；日志见 `%ProgramData%\Roommate-LAN\logs\tailscaled.log` |
| 官方 Tailscale 冲突 | 官方服务 RUNNING 时点连接 | 立即中文报错，要求先退出官方 |
| 服务未就绪 | 停止 `RoommateNetworkService` 后点连接 | 提示网络服务未就绪 / 修复安装 |
| 重复点击连接 | 连接中连点 CTA | 幂等；不双开 `tailscaled` |
| 退出清理 | 连接后退出 App | 30s 内租约过期或立即 Disconnect；sidecar 停止；服务进程仍 Running |
| 强杀 GUI | 任务管理器结束 Roommate | ≤30s 后服务自动 down sidecar |
| 复制 IP | 点击队友 IP | 剪贴板内容正确 |
| 私有 pipe | 连接成功后 | 存在 `\\.\pipe\ProtectedPrefix\Administrators\Roommate\tailscaled`；服务 IPC 为 `\\.\pipe\Roommate\NetworkService` |

## 打包

```powershell
npm run fetch-bins
npm run tauri build
npm run package-check
```

产物：`src-tauri/target/release/bundle/nsis/`

**门禁：**

1. 全新 Windows 安装 NSIS：**仅安装时**出现一次 UAC；重启后 `RoommateNetworkService` 自动 RUNNING。
2. 普通用户启动 App 一键连接：无 UAC、无窗口重启/闪烁，拿到 `100.64.0.0/24` IP。
3. 覆盖升级 / 卸载后无残留服务、进程、Roommate 网卡。

## Steam Listen 冒烟

1. 双方连接 Roommate，复制房主 `100.64.0.x`。
2. 游戏内 Listen / Direct IP 填虚拟 IP（依赖真实 Wintun TUN）。
3. 观察卡顿与丢包是否优于公网直连。
