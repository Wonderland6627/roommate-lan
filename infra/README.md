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

# （可选）成员公网 IP 地理位置：免注册下载 DB-IP City Lite：
./scripts/fetch-geoip.sh
./scripts/ops-heal.sh rebuild
```

客户端公开包只需内嵌同一 `ROOMMATE_LOGIN_SERVER=https://${HS_DOMAIN}`（Room API 挂在 `/api/*`）。**普通用户无需再配置 `.env` AuthKey**。

### Room API 接口摘要

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/rooms` | 未过期房间列表（无短码） |
| POST | `/api/rooms` | 创建房间：`{ name, displayName }` → 4 位码 + AuthKey |
| POST | `/api/join` | 加入：`{ code, displayName }` |
| GET | `/api/rooms/{id}/members` | 成员列表（需 `X-Member-Token`） |
| POST | `/api/rooms/{id}/leave` | 退出：`{ memberToken }` |
| POST | `/api/rooms/{id}/dissolve` | 房主解散：`{ memberToken }` |

房间 TTL 默认 4h（`ROOM_TTL_HOURS`）；主动离开后若无人则销毁。

AuthKey 默认签发 **ephemeral** 节点；Room API 会按 `HEADSCALE_NODE_OFFLINE_SECS`（默认 900）定时清理长期 offline 的 Headscale 节点。升级后若仍看到历史 `Ephemeral=false` 幽灵节点，可先手动清空一次：

```bash
docker compose exec headscale headscale nodes list
# 确认无活跃房间后按 ID 删除，例如：
docker compose exec headscale headscale nodes delete -i 1 --force
```

之后依赖 ephemeral + 定时 GC 维持，一般不必再手工清。

### 宝塔一键运维

上传代码到 `/opt/roommate/infra` 后，可随时：

```bash
cd /opt/roommate/infra
chmod +x scripts/ops-heal.sh
./scripts/ops-heal.sh          # 检查 + 清 offline 节点 + 必要时重启
./scripts/ops-heal.sh menu     # 交互菜单
./scripts/ops-heal.sh rebuild  # 上传 room-api 代码后重建
./scripts/ops-heal.sh purge --all   # 强制删光所有 offline 节点
```

脚本自动识别 `docker-compose.baota.yml`。默认只删 **offline 且 Last seen 超过 15 分钟** 的节点（`OFFLINE_MINUTES=30 ./scripts/ops-heal.sh purge` 可改阈值）。

### Room API 业务日志

Room API 将房间生命周期等事件写入宿主机目录 `infra/logs/`（容器内 `/data/logs`）：

- 当前文件：`room-api.log`
- 按天滚动，默认保留 14 天（`LOG_RETAIN_DAYS`）
- 宝塔：文件管理进入部署目录下的 `infra/logs/`，打开 `room-api.log` 即可
- 日志时间戳为 **UTC+8**（与流量文件一致）
- 不记录 AuthKey、memberToken、完整房间短码

也可：`docker compose logs -f room-api`（同一内容也会打到 stdout）。

### 成员地理位置（GeoIP）

Room API 用进房 / 心跳 HTTPS 的公网源 IP，经离线 **DB-IP City Lite** MMDB 解析国家/省州/城市，写入 `egressIp` / `geoLabel` 与业务日志（CC BY 4.0，免注册）。

```bash
cd /opt/roommate/infra
chmod +x scripts/*.sh
./scripts/fetch-geoip.sh
./scripts/ops-heal.sh rebuild
```

会生成 `infra/geoip/city.mmdb`（勿提交 git）。库文件建议每月重新 fetch。成员列表旁有 DB-IP 署名链接（许可要求）。

无 MMDB 时服务照常运行，仅无地理位置文案。

### 房间流量统计（宝塔可查）

客户端按连接路径累计对端 Tx/Rx，经 presence / 退出 上报；**房间销毁时**写入：

| 文件 | 说明 |
|------|------|
| `infra/logs/traffic/rooms-YYYY-MM-DD.log` | 当日每个已销毁房间一行（含中继/P2P 字节与可读单位） |
| `infra/logs/traffic/daily-summary.log` | 按自然日汇总：房间数、中继合计、P2P 合计 |

- **relay**：走自建 DERP 的流量合计（≈云服流量包相关）
- **p2p**：直连 / peer-relay（不经过云服带宽）
- `reporters < members` 时表示部分客户端未上报（旧版或异常退出），中继量可能偏小
- 时间戳与按日文件名均为 **UTC+8**

宝塔：文件管理 → 部署目录 → `infra/logs/traffic/`。

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
