# Roommate 服务端基础设施

在腾讯云轻量应用服务器（Ubuntu）上部署 Headscale（控制面）+ **Room API**（创建/加入房间）+ Caddy（TLS）+ DERP/STUN。

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
./scripts/create-authkey.sh   # 确保 headscale 用户 roommate 存在（可选遗留）

# 创建 Headscale API key 供 Room API 签发短期 AuthKey：
docker compose exec headscale headscale apikeys create
# 把输出写入 .env 的 HEADSCALE_API_KEY=...
docker compose up -d --build room-api caddy
```

客户端公开包只需内嵌同一 `ROOMMATE_LOGIN_SERVER=https://${HS_DOMAIN}`（Room API 挂在 `/api/*`）。**普通用户无需再配置 `.env` AuthKey**。

### Room API 接口摘要

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/rooms` | 未过期房间列表（无短码） |
| POST | `/api/rooms` | 创建房间：`{ name, displayName }` → 4 位码 + AuthKey |
| POST | `/api/join` | 加入：`{ code, displayName }` |
| GET | `/api/rooms/{id}/members` | 成员显示名 |
| POST | `/api/rooms/{id}/leave` | 退出：`{ memberToken }` |
| POST | `/api/rooms/{id}/dissolve` | 房主解散：`{ memberToken }` |

房间 TTL 默认 4h（`ROOM_TTL_HOURS`）；主动离开后若无人则销毁。

### Room API 业务日志

Room API 将房间生命周期等事件写入宿主机目录 `infra/logs/`（容器内 `/data/logs`）：

- 当前文件：`room-api.log`
- 按天滚动，默认保留 14 天（`LOG_RETAIN_DAYS`）
- 宝塔：文件管理进入部署目录下的 `infra/logs/`，打开 `room-api.log` 即可
- 不记录 AuthKey、memberToken、完整房间短码

也可：`docker compose logs -f room-api`（同一内容也会打到 stdout）。

### 宝塔环境（本机 80/443 已被占用）

主机已装宝塔 Nginx 时，不要启动 Compose 里的 Caddy。用覆盖文件把服务只绑本机，由宝塔反代 HTTPS：

```bash
docker compose -f docker-compose.yml -f docker-compose.baota.yml up -d --build
```

Nginx 示例（与 Headscale 同域名）：

```nginx
location /api/ {
    proxy_pass http://127.0.0.1:8081;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}

location / {
    proxy_pass http://127.0.0.1:8080;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}
```

DERP 仍直连主机 `:8443`（证书由宝塔申请后拷入 `derper/certs/`）。

## 验证

```bash
curl -I "https://${HS_DOMAIN}/health"
curl -s "https://${HS_DOMAIN}/api/rooms"
# 推荐：用 Roommate 客户端「创建房间 / 加入房间」
# 或手动用 CLI 指向同一 Headscale（调试用）：
tailscale up --login-server=https://${HS_DOMAIN} --authkey=<KEY> --accept-dns=false
tailscale status
tailscale ping <peer-100.64.ip>
```

强制走中继：在其中一台客户端防火墙上屏蔽 UDP；此时 `tailscale status` 应显示 `relay; derp txy`（或 region 900），同时 `ping` 仍可通。
