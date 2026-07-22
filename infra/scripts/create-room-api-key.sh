#!/usr/bin/env bash
# Create a Headscale API key for Room API (preferred over long-lived client AuthKey).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "Creating Headscale API key (save once; cannot be retrieved again)..."
KEY="$(docker compose exec -T headscale headscale apikeys create | tr -d '\r' | tail -1)"

echo "=============================================="
echo "HEADSCALE_API_KEY (put in infra/.env):"
echo "$KEY"
echo "=============================================="
echo "Then: docker compose up -d --build room-api"
echo "Client builds only need ROOMMATE_LOGIN_SERVER=https://\${HS_DOMAIN}"
