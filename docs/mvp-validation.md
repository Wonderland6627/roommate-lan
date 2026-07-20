# Roommate MVP 测试矩阵与打包验收

## 前置

1. 完成 [`infra/`](../infra/README.md) 部署，拿到可复用 AuthKey。
2. 根目录复制 `.env.example` → `.env`，填入真实 `ROOMMATE_LOGIN_SERVER` / `ROOMMATE_AUTH_KEY`。
3. 拉取 Sidecar：`npm run fetch-bins`
4. 单元测试：`npm run test:rust`

## 功能验收矩阵

| 场景 | 步骤 | 期望 |
|------|------|------|
| 同局域网 Easy NAT | 两台机器一键连接，互 ping | 徽章多为 **P2P 直连**，RTT 较低 |
| 跨运营商 / 对称 NAT | 异地或防火墙禁 UDP | 徽章为 **腾讯云 DERP**，仍可 ping / 进房 |
| 仅一人在线 | 单客户端连接 | 列表空态文案，无崩溃 |
| 杀软拦截 TUN | 模拟拦截后连接 | UI 显示中文错误，可断开重试 |
| 重复点击连接 | 连接中连点 CTA | 幂等；不双开 `tailscaled` |
| 退出清理 | 连接后退出 App | 无残留高占用 `tailscaled` |
| 复制 IP | 点击队友 IP | 剪贴板内容正确 |
| UAC | 非管理员启动后点连接 | 弹出 UAC；确认后可连 |

## 打包

```powershell
npm run fetch-bins
npm run tauri build
npm run package-check
```

产物：`src-tauri/target/release/bundle/nsis/`

全新 Windows 10/11（无预装 Tailscale）安装后应能完成联机。若本机已有官方 Tailscale 服务，请先退出/停用，避免抢占 TUN。

## Steam Listen 冒烟

1. 双方连接 Roommate，复制房主 `100.64.0.x`。
2. 游戏内 Listen / Direct IP 填虚拟 IP。
3. 观察卡顿与丢包是否优于公网直连。
