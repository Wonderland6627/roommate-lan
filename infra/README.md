# Roommate 服务端基础设施

在腾讯云轻量应用服务器（Ubuntu）上部署 Headscale（控制面）+ Caddy（TLS）+ DERP/STUN。

## 前置条件

- 域名 DNS：`HS_DOMAIN` 与 `DERP_DOMAIN` 的 A 记录指向服务器公网 IP
- 安全组：TCP 80、443、8443；UDP 3478
- Docker + Docker Compose v2
- DERP 所需 TLS 证书（见 [derper/README.md](derper/README.md)）

## 部署

```bash
cd infra
cp .env.example .env
# 编辑 .env — 填写 HS_DOMAIN、DERP_DOMAIN、ACME_EMAIL

# 放好 DERP 证书后执行：
chmod +x scripts/*.sh
./scripts/bootstrap.sh
./scripts/create-authkey.sh
```

### 宝塔环境（本机 80/443 已被占用）

主机已装宝塔 Nginx 时，不要启动 Compose 里的 Caddy。用覆盖文件把 Headscale 只绑本机，由宝塔反代 HTTPS：

```bash
docker compose -f docker-compose.yml -f docker-compose.baota.yml up -d
```

DERP 仍直连主机 `:8443`（证书由宝塔申请后拷入 `derper/certs/`）。

## 验证

```bash
curl -I "https://${HS_DOMAIN}/health"
# 在两台 Windows 电脑上使用官方 Tailscale CLI：
tailscale up --login-server=https://${HS_DOMAIN} --authkey=<KEY> --accept-dns=false
tailscale status
tailscale ping <peer-100.64.ip>
```

强制走中继：在其中一台客户端防火墙上屏蔽 UDP；此时 `tailscale status` 应显示 `relay; derp txy`（或 region 900），同时 `ping` 仍可通。
