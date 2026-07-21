# Roommate

跨地区 / 跨运营商 Steam P2P、Listen Server 联机助手：P2P 优先，腾讯云 DERP 保底。  
技术栈：**Tauri 2 + Vue 3 + Tailwind**，网络层由 **RoommateNetworkService**（Windows 服务）托管内置 Tailscale Sidecar（独立 named pipe + Wintun），控制面为自建 **Headscale**。用户只需安装 Roommate；**日常启动与一键连接不再弹 UAC、不重启窗口**。

## 仓库结构

| 路径 | 说明 |
|------|------|
| [`infra/`](infra/README.md) | Headscale + Caddy + DERP（Docker Compose） |
| [`src/`](src/) | Vue 3 前端 |
| [`src-tauri/`](src-tauri/) | Rust 核心 / 网络服务 / Sidecar |
| [`scripts/`](scripts/) | 拉取二进制、开发服务、打包检查 |
| [`docs/`](docs/) | MVP 验收与扩展规划 |

## 快速开始（客户端）

```powershell
# 1. 配置 Headscale 地址与 AuthKey（勿提交）
copy .env.example .env

# 2. 依赖与 Sidecar（含 wintun.dll）
npm install
npm run fetch-bins

# 3. 管理员安装一次开发用网络服务（仅开发机）
#    会编译 release roommate.exe 并注册 RoommateNetworkService
npm run dev-service

# 4. 普通用户启动 UI（不再弹 UAC）
npm run tauri dev

# 5. 单元测试
npm run test:rust
```

> Windows：若本机官方 Tailscale 服务正在运行，请先退出（`net stop Tailscale`），再连接 Roommate。  
> 正式用户：安装 NSIS 包时只弹一次 UAC（注册服务）；之后日常使用零提权。

## 服务端

见 [`infra/README.md`](infra/README.md)。部署后把 `ROOMMATE_LOGIN_SERVER` / `ROOMMATE_AUTH_KEY` 写入客户端 `.env`。

## MVP 功能

- 一键连接 / 断开（预嵌 AuthKey；经网络服务，无窗口闪烁）
- 队友列表、一键复制虚拟 IP
- P2P vs 腾讯云 DERP 状态徽章
- Peer RTT（`tailscale ping`）
- Windows 网络服务托管内置 `tailscaled`（独立 Roommate named pipe）

验收清单：[`docs/mvp-validation.md`](docs/mvp-validation.md)  
扩展规划：[`docs/extensions.md`](docs/extensions.md)

## 许可注意

Sidecar 使用 Tailscale 客户端二进制 + Wintun，连接的是 Roommate 自建 Headscale（非 Tailscale.com）。再分发前请核对：

- [Tailscale 许可与条款](https://tailscale.com/terms) / 上游 [BSD-3 源码](https://github.com/tailscale/tailscale)
- [Wintun Prebuilt Binaries License](https://git.zx2c4.com/wintun/tree/prebuilt-binaries-license.txt)
- 仓库内 [`THIRD_PARTY_NOTICES.txt`](THIRD_PARTY_NOTICES.txt)

本仓库默认 **不** 提交 `.exe` / `wintun.dll`，由 `npm run fetch-bins` 本机拉取。
