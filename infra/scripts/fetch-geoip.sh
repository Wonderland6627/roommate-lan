#!/usr/bin/env bash
# Download DB-IP IP to City Lite MMDB into infra/geoip/city.mmdb for room-api.
# No account / API key required. License: CC BY 4.0 — https://db-ip.com
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

OUT_DIR="$ROOT/geoip"
OUT_FILE="$OUT_DIR/city.mmdb"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$OUT_DIR"

ym="$(date -u +%Y-%m)"
prev="$(date -u -d '1 month ago' +%Y-%m 2>/dev/null || date -u -v-1m +%Y-%m 2>/dev/null || true)"
archive="$TMP/dbip-city-lite.mmdb.gz"

for try in "$ym" "$prev"; do
  [[ -z "$try" ]] && continue
  url="https://download.db-ip.com/free/dbip-city-lite-${try}.mmdb.gz"
  echo "Downloading DB-IP City Lite (${try})…"
  if curl -fsSL -L -o "$archive" "$url"; then
    gunzip -c "$archive" >"$OUT_FILE"
    echo "Wrote $OUT_FILE (DB-IP City Lite ${try})"
    echo "Attribution: https://db-ip.com (CC BY 4.0)"
    echo "Restart room-api: ./scripts/ops-heal.sh rebuild"
    exit 0
  fi
  echo "  download failed for ${try}, trying next…"
done

echo "Failed to download DB-IP City Lite. Check network / curl access to download.db-ip.com."
exit 1
