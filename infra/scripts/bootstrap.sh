#!/usr/bin/env bash
# Bootstrap Roommate infra on Ubuntu (Tencent Cloud Lighthouse).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ ! -f .env ]]; then
  echo "Copy .env.example to .env and set HS_DOMAIN / DERP_DOMAIN / ACME_EMAIL first."
  exit 1
fi

# shellcheck disable=SC1091
source .env

if [[ ! -f derper/certs/"${DERP_DOMAIN}.crt" || ! -f derper/certs/"${DERP_DOMAIN}.key" ]]; then
  echo "Missing derper/certs/${DERP_DOMAIN}.crt and .key — see derper/README.md"
  exit 1
fi

# Keep Headscale / DERP hostnames in sync with .env
sed -i "s|server_url:.*|server_url: https://${HS_DOMAIN}|" headscale/config.yaml
sed -i "s|hostname:.*|hostname: ${DERP_DOMAIN}|" headscale/derp.yaml

docker compose pull
docker compose up -d

echo "Waiting for Headscale..."
for i in {1..30}; do
  if docker compose exec -T headscale headscale users list >/dev/null 2>&1; then
    break
  fi
  sleep 2
done

if ! docker compose exec -T headscale headscale users list 2>/dev/null | grep -q roommate; then
  docker compose exec -T headscale headscale users create roommate
fi

echo "Bootstrap done. Next: ./scripts/create-authkey.sh"
