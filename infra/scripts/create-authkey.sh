#!/usr/bin/env bash
# Create a reusable pre-auth key for Roommate MVP clients.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

EXPIRATION="${1:-90d}"

if ! docker compose exec -T headscale headscale users list 2>/dev/null | grep -q roommate; then
  docker compose exec -T headscale headscale users create roommate
fi

KEY="$(docker compose exec -T headscale \
  headscale preauthkeys create --user roommate --reusable --expiration "$EXPIRATION" -o json \
  | sed -n 's/.*"key"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)"

if [[ -z "$KEY" ]]; then
  # Fallback for older headscale CLI output (plain text)
  KEY="$(docker compose exec -T headscale \
    headscale preauthkeys create --user roommate --reusable --expiration "$EXPIRATION" | tr -d '\r' | tail -1)"
fi

echo "=============================================="
echo "Reusable AuthKey (do NOT commit to git):"
echo "$KEY"
echo "=============================================="
echo "Client env:"
echo "  ROOMMATE_LOGIN_SERVER=https://\${HS_DOMAIN}"
echo "  ROOMMATE_AUTH_KEY=$KEY"
echo "Store in CI secrets / local .env — never in public repos."
