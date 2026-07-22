# Roommate-LAN

跨地区 / 跨运营商的联机助手：优先 P2P 直连，必要时走自建 DERP 中继。

**技术栈**：Tauri 2 · Vue 3 · Rust  
**网络**：Windows 服务托管内置 Tailscale Sidecar，控制面为自建 Headscale + Room API。

安装后日常使用无需反复 UAC；通过「创建 / 加入房间」进网，无需手动配置 AuthKey。

当前版本见 [`package.json`](package.json)（发版标签形如 `v0.5.1`）。

## 功能

- 房间大厅：显示名、创建房间、房间列表、4 位邀请码加入
- 房内：邀请码、成员列表（显示名、虚拟 IP、RTT、P2P / DERP）
- 房主解散 / 成员退出；关窗时尽力清理房间；失活房主由服务端回收
- 全局单实例（再次启动会聚焦已有窗口）
- 应用内检查更新（签名安装包）

## 仓库结构

| 路径 | 说明 |
|------|------|
| [`src/`](src/) | Vue 前端 |
| [`src-tauri/`](src-tauri/) | Rust 客户端 / 网络服务 / Sidecar |
| [`infra/`](infra/README.md) | Headscale、Room API、DERP（Docker Compose） |
| [`scripts/`](scripts/) | 拉取二进制、开发服务、版本同步等 |
| [`.github/workflows/`](.github/workflows/) | `v*` 标签触发的 Release 构建 |

## 开发（Windows）

```powershell
copy .env.example .env
# 编辑 .env：填入可用的 ROOMMATE_LOGIN_SERVER（与 Room API 同域）

npm install
npm run fetch-bins

# 首次：编译并注册开发用 RoommateNetworkService（会弹一次 UAC）
npm run dev-service

# 启动 UI
npm run tauri dev

npm run test:rust
npm run version:check
```

说明：

- 若本机官方 Tailscale 服务在跑，请先停止后再连接 Roommate。
- 正式安装包在安装时注册网络服务；日常启动不再提权。检查更新 / 覆盖安装仍可能需要 UAC。

## 版本与发版

版本需在 `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`（及对应 lockfile）保持一致：

```powershell
npm run version:set -- 0.5.1
npm run version:check

git tag v0.5.1
git push origin v0.5.1
```

打上 `v*` 标签后，GitHub Actions 会构建并发布 Windows 安装包。客户端从本仓库 Releases 的 `latest.json` 检查更新。

维护者需自行保管 Updater 签名私钥与 CI Secrets，**不要**把私钥提交进仓库。公开 Release 不内嵌长期 AuthKey；进网凭证由 Room API 短期签发。

## 服务端

见 [`infra/README.md`](infra/README.md)。部署完成后，将同一 Login Server 地址配置到客户端构建环境（如 CI 变量 `ROOMMATE_LOGIN_SERVER`）。

## 第三方许可

Sidecar 使用 Tailscale 客户端二进制与 Wintun，连接的是自建 Headscale（非 Tailscale.com）。分发前请核对：

- [Tailscale 条款](https://tailscale.com/terms) / [上游源码许可](https://github.com/tailscale/tailscale)
- [Wintun Prebuilt Binaries License](https://git.zx2c4.com/wintun/tree/prebuilt-binaries-license.txt)
- 仓库内 [`THIRD_PARTY_NOTICES.txt`](THIRD_PARTY_NOTICES.txt)

本仓库默认不提交 `.exe` / `wintun.dll`，由 `npm run fetch-bins` 本机拉取（CI 同样拉取并校验）。
