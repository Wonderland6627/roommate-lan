# Roommate

跨地区 / 跨运营商 Steam P2P、Listen Server 联机助手：P2P 优先，腾讯云 DERP 保底。  
技术栈：**Tauri 2 + Vue 3 + Tailwind**，网络层由 **RoommateNetworkService**（Windows 服务）托管内置 Tailscale Sidecar（独立 named pipe + Wintun），控制面为自建 **Headscale**。用户只需安装 Roommate；**日常启动与一键连接不再弹 UAC、不重启窗口**。

## 仓库结构

| 路径 | 说明 |
|------|------|
| [`infra/`](infra/README.md) | Headscale + Caddy + DERP（Docker Compose） |
| [`src/`](src/) | Vue 3 前端 |
| [`src-tauri/`](src-tauri/) | Rust 核心 / 网络服务 / Sidecar |
| [`scripts/`](scripts/) | 拉取二进制、开发服务、打包检查、版本同步 |
| [`.github/workflows/`](.github/workflows/) | `v*` 标签触发的 Release 构建 |
| [`docs/`](docs/) | MVP 验收与扩展规划 |

## 快速开始（客户端）

```powershell
# 1. 配置 Headscale / Room API 地址（公开包由 CI 注入；开发机写 .env）
copy .env.example .env

# 2. 依赖与 Sidecar（含 wintun.dll）
npm install
npm run fetch-bins

# 3. 安装一次开发用网络服务（仅开发机；普通窗口即可，会弹一次 UAC）
#    会编译 release roommate.exe 并注册 RoommateNetworkService
npm run dev-service

# 4. 普通用户启动 UI（不再弹 UAC）
npm run tauri dev

# 5. 单元测试 / 版本一致性
npm run test:rust
npm run version:check
```

> Windows：若本机官方 Tailscale 服务正在运行，请先退出（`net stop Tailscale`），再连接 Roommate。
> 正式用户：安装 NSIS 包时只弹一次 UAC（注册服务）；之后日常使用零提权。
> **检查更新 / 覆盖升级** 仍可能弹出 UAC（per-machine 安装）。

## 版本号

应用版本需在以下文件保持一致：

- `package.json` / `package-lock.json`
- `src-tauri/Cargo.toml` / `Cargo.lock`
- `src-tauri/tauri.conf.json`

```powershell
# 发版前改版本（示例）
npm run version:set -- 0.3.0
npm run version:check
```

界面底部显示运行时版本，并提供「检查更新」。

## 发布（Git tag → GitHub Actions）

### 一次性：Updater 签名密钥

```powershell
npm run tauri signer generate -- -w src-tauri/keys/updater.key
```

1. 将 `src-tauri/keys/updater.key.pub` 内容写入 [`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json) 的 `plugins.updater.pubkey`（仓库已含当前公钥）。
2. 在 GitHub 仓库 Settings → Secrets 配置：
   - `TAURI_SIGNING_PRIVATE_KEY`：私钥文件全文
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：生成时设置的密码
   - （可选）`ROOMMATE_LOGIN_SERVER`：公开包可内嵌 login server URL
3. **切勿**把 `*.key` 提交进 Git；丢失私钥会导致已安装客户端无法再验证新更新。

### AuthKey 门禁

公开 Release **不会**编译进长期 `ROOMMATE_AUTH_KEY`（可从安装包提取）。CI 设置 `ROOMMATE_PUBLIC_RELEASE=1`。
用户通过 App **创建 / 加入房间**，由服务端 Room API 签发短期 AuthKey；只需保证构建时注入 `ROOMMATE_LOGIN_SERVER`（与 Headscale / `/api` 同域）。

开发机仍可用本地 `.env` 的 `ROOMMATE_AUTH_KEY` 作兜底。

### 打标签发布

```powershell
# 1. PR 合并版本 bump 到 master 后：
git tag v0.3.0
git push origin v0.3.0

# 2. Actions 会：校验版本 ↔ tag、拉取 sidecar（校验 MSI SHA-256）、
#    测试、签名构建 NSIS、创建 GitHub Release（含 latest.json / .sig / SHA256SUMS）
```

预发布标签形如 `v0.3.0-rc.1` 会标记为 prerelease。客户端从
`https://github.com/Wonderland6627/roommate-lan/releases/latest/download/latest.json` 检查更新。

## 服务端

见 [`infra/README.md`](infra/README.md)。部署 Headscale + Room API 后，把 `ROOMMATE_LOGIN_SERVER` 写入客户端构建环境（CI secret）。

## MVP 功能

- 创建房间 / 加入房间（4 位短码、房间列表、显示名；经网络服务，无窗口闪烁）
- 房内成员显示名；房主解散 / 队友退出；房间 TTL
- 队友列表、一键复制虚拟 IP
- P2P vs 腾讯云 DERP 状态徽章
- Peer RTT（`tailscale ping`）
- Windows 网络服务托管内置 `tailscaled`（独立 Roommate named pipe）
- 应用内版本显示与签名更新（检查 → 下载 → 安装 → 重启）

验收清单：[`docs/mvp-validation.md`](docs/mvp-validation.md)  
扩展规划：[`docs/extensions.md`](docs/extensions.md)  
宝塔补丁部署（上传、不 git）：[`docs/deploy-baota-beijing.md`](docs/deploy-baota-beijing.md)

## 许可注意

Sidecar 使用 Tailscale 客户端二进制 + Wintun，连接的是 Roommate 自建 Headscale（非 Tailscale.com）。再分发前请核对：

- [Tailscale 许可与条款](https://tailscale.com/terms) / 上游 [BSD-3 源码](https://github.com/tailscale/tailscale)
- [Wintun Prebuilt Binaries License](https://git.zx2c4.com/wintun/tree/prebuilt-binaries-license.txt)
- 仓库内 [`THIRD_PARTY_NOTICES.txt`](THIRD_PARTY_NOTICES.txt)

本仓库默认 **不** 提交 `.exe` / `wintun.dll`，由 `npm run fetch-bins` 本机拉取（CI 同样拉取并校验 MSI 哈希）。
