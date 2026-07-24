# Roommate-LAN

跨地区 / 跨运营商的联机助手：优先 P2P 直连，必要时走自建 DERP 中继。

**技术栈** Tauri 2 · Vue 3 · Rust  
**网络** Windows 服务托管内置 Tailscale Sidecar · 自建 Headscale + Room API

安装后日常使用无需反复提权；通过「创建 / 加入房间」进网，无需手动配置 AuthKey。

当前版本 **v0.5.4** · [下载最新安装包](https://github.com/Wonderland6627/roommate-lan/releases/latest)

---

## 功能

- 房间大厅：显示名、创建房间、房间列表、4 位邀请码加入
- 房内：邀请码、成员列表（显示名、虚拟 IP、RTT、P2P / DERP）
- 一键测试与队友的连通性，失败时给出简要原因
- 房主解散 / 成员退出；关窗时尽力清理房间
- 全局单实例；应用内检查更新（签名安装包）

## 使用提示

1. 从 [Releases](https://github.com/Wonderland6627/roommate-lan/releases/latest) 安装 Windows 版
2. 打开应用后设置显示名，创建房间或输入邀请码加入
3. 进房后可复制自己的虚拟 IP，或对队友点「测试」查看延迟

若本机已运行官方 Tailscale，请先退出后再使用 Roommate。正式安装时会注册网络服务；日常启动不再提权，检查更新 / 覆盖安装仍可能需要 UAC。

## 仓库结构

| 路径 | 说明 |
|------|------|
| [`src/`](src/) | Vue 前端 |
| [`src-tauri/`](src-tauri/) | Rust 客户端 / 网络服务 / Sidecar |
| [`infra/`](infra/README.md) | Headscale、Room API、DERP |
| [`scripts/`](scripts/) | 拉取二进制、开发服务、版本同步 |
| [`.github/workflows/`](.github/workflows/) | `v*` 标签触发 Release 构建 |

## 本地开发

```powershell
copy .env.example .env
# 编辑 .env，填入可用的 ROOMMATE_LOGIN_SERVER

npm install
npm run fetch-bins
npm run dev-service   # 首次注册开发用网络服务（会弹一次 UAC）
npm run tauri dev
```

版本需在 `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` 保持一致：

```powershell
npm run version:set -- 0.5.4
npm run version:check
git tag v0.5.4
git push origin v0.5.4
```

打上 `v*` 标签后，GitHub Actions 会构建并发布 Windows 安装包。

## 第三方许可

Sidecar 使用 Tailscale 客户端二进制与 Wintun，连接的是自建 Headscale（非 Tailscale.com）。详见：

- [Tailscale 条款](https://tailscale.com/terms) / [上游源码](https://github.com/tailscale/tailscale)
- [Wintun Prebuilt Binaries License](https://git.zx2c4.com/wintun/tree/prebuilt-binaries-license.txt)
- 仓库内 [`THIRD_PARTY_NOTICES.txt`](THIRD_PARTY_NOTICES.txt)

本仓库默认不提交 `.exe` / `wintun.dll`，由 `npm run fetch-bins` 本机拉取（CI 同样拉取并校验）。
