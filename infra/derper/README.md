# DERP TLS certificates

`derper` with `DERP_CERT_MODE=manual` expects certificate files named after
`DERP_DOMAIN` inside this directory (mounted at `/app/certs`):

```text
derper/certs/
  derp.example.com.crt
  derp.example.com.key
```

## Quick start (Let's Encrypt on the host, then copy)

```bash
# On the Ubuntu host (certbot standalone — stop Caddy briefly if port 80 is busy,
# or use DNS-01 / Caddy on a second hostname).
sudo certbot certonly --standalone -d derp.example.com

sudo mkdir -p /opt/roommate/infra/derper/certs
sudo cp /etc/letsencrypt/live/derp.example.com/fullchain.pem \
  /opt/roommate/infra/derper/certs/derp.example.com.crt
sudo cp /etc/letsencrypt/live/derp.example.com/privkey.pem \
  /opt/roommate/infra/derper/certs/derp.example.com.key
```

## Port layout (single IP)

| Service   | Host port     | Notes                          |
|-----------|---------------|--------------------------------|
| Caddy     | 80, 443       | Headscale HTTPS                |
| derper    | 8443 → 443    | DERP HTTPS (see derp.yaml)     |
| derper    | 3478/udp      | STUN                           |

Open Tencent Cloud security group: TCP 80/443/8443, UDP 3478.

Do not commit real `.crt` / `.key` files.
