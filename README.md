# Roommate

跨地区 / 跨运营商 Steam P2P、Listen Server 联机助手：P2P 优先，腾讯云 DERP 保底。  
技术栈：**Tauri 2 + Vue 3 + Tailwind**，网络层捆绑 **Tailscale** Sidecar，控制面为自建 **Headscale**。

## 仓库结构

| 路径 | 说明 |
|------|------|
| [`infra/`](infra/README.md) | Headscale + Caddy + DERP（Docker Compose） |
| [`src/`](src/) | Vue 3 前端 |
| [`src-tauri/`](src-tauri/) | Rust 核心 / Sidecar / UAC |
| [`scripts/`](scripts/) | 拉取 Tailscale 二进制、打包检查 |
| [`docs/`](docs/) | MVP 验收与扩展规划 |

## 快速开始（客户端）

```powershell
# 1. 配置 Headscale 地址与 AuthKey（勿提交）
copy .env.example .env

# 2. 依赖与 Sidecar
npm install
npm run fetch-bins

# 3. 开发
npm run tauri dev

# 4. 单元测试（status 解析等）
npm run test:rust
```

## 服务端

见 [`infra/README.md`](infra/README.md)。部署后把 `ROOMMATE_LOGIN_SERVER` / `ROOMMATE_AUTH_KEY` 写入客户端 `.env`。

## MVP 功能

- 一键连接 / 断开（预嵌 AuthKey）
- 队友列表、一键复制虚拟 IP
- P2P vs 腾讯云 DERP 状态徽章
- Peer RTT（`tailscale ping`）
- Windows UAC 提权与 `tailscaled` 进程守护

验收清单：[`docs/mvp-validation.md`](docs/mvp-validation.md)  
扩展规划：[`docs/extensions.md`](docs/extensions.md)

## 许可注意

Sidecar 使用官方 Tailscale 客户端二进制，再分发前请核对 [Tailscale 许可与条款](https://tailscale.com/terms)；本仓库默认 **不** 提交这些 `.exe`，由 `npm run fetch-bins` 本机拉取。
