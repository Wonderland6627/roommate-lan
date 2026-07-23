from __future__ import annotations

import logging
from datetime import datetime, timedelta, timezone
from typing import Any

import httpx

from .app_logging import log_event
from .settings import settings


def _headers() -> dict[str, str]:
    return {"Authorization": f"Bearer {settings.headscale_api_key}"}


def _base() -> str:
    return settings.headscale_api_url.rstrip("/")


async def mint_auth_key() -> str:
    if settings.mock_auth_key:
        return settings.mock_auth_key

    if not settings.headscale_api_key:
        raise RuntimeError(
            "未配置 HEADSCALE_API_KEY。请在 headscale 容器内执行 "
            "`headscale apikeys create` 并把结果写入 infra/.env。"
        )

    expiration = datetime.now(timezone.utc) + timedelta(hours=settings.authkey_ttl_hours)
    url = _base() + "/api/v1/preauthkey"
    payload = {
        "user": settings.headscale_user,
        "reusable": False,
        "ephemeral": settings.authkey_ephemeral,
        "expiration": expiration.isoformat().replace("+00:00", "Z"),
    }

    async with httpx.AsyncClient(timeout=20.0) as client:
        resp = await client.post(url, json=payload, headers=_headers())
        if resp.status_code >= 400:
            # Older Headscale builds may expect user name vs id — surface body.
            raise RuntimeError(
                f"Headscale 签发 AuthKey 失败 ({resp.status_code}): {resp.text[:300]}"
            )
        data = resp.json()
        key = (
            data.get("preAuthKey", {}).get("key")
            or data.get("pre_auth_key", {}).get("key")
            or data.get("key")
        )
        if not key:
            raise RuntimeError(f"Headscale 响应无 AuthKey: {data!r}")
        return str(key)


async def _delete_node_id(client: httpx.AsyncClient, node_id: str) -> bool:
    resp = await client.delete(
        f"{_base()}/api/v1/node/{node_id}",
        headers=_headers(),
    )
    if resp.status_code < 400 or resp.status_code == 404:
        return True
    return False


async def _find_node_id_by_ip(
    client: httpx.AsyncClient, virtual_ip: str
) -> str | None:
    resp = await client.get(f"{_base()}/api/v1/node", headers=_headers())
    if resp.status_code >= 400:
        return None
    data = resp.json()
    nodes = data.get("nodes") or data.get("Nodes") or []
    target = virtual_ip.strip()
    if not target:
        return None
    for node in nodes:
        if not isinstance(node, dict):
            continue
        ips = (
            node.get("ipAddresses")
            or node.get("ip_addresses")
            or node.get("IPAddresses")
            or []
        )
        if target in ips:
            nid = node.get("id") or node.get("ID")
            if nid is not None:
                return str(nid)
    return None


async def delete_node_best_effort(
    node_id: str | None = None,
    virtual_ip: str | None = None,
) -> None:
    """Best-effort Headscale node cleanup after leave / purge.

    Skips when mock auth is enabled or API key is missing.
    """
    if settings.mock_auth_key or not settings.headscale_api_key:
        return
    nid = (node_id or "").strip()
    vip = (virtual_ip or "").strip()
    if not nid and not vip:
        return

    try:
        async with httpx.AsyncClient(timeout=10.0) as client:
            if nid and await _delete_node_id(client, nid):
                log_event("headscale.node_deleted", node_id=nid, virtual_ip=vip or None)
                return
            if vip:
                found = await _find_node_id_by_ip(client, vip)
                if found and await _delete_node_id(client, found):
                    log_event(
                        "headscale.node_deleted",
                        node_id=found,
                        virtual_ip=vip,
                        matched_by="ip",
                    )
                    return
            log_event(
                "headscale.node_delete_miss",
                level=logging.WARNING,
                node_id=nid or None,
                virtual_ip=vip or None,
            )
    except Exception as e:
            log_event(
                "headscale.node_delete_failed",
                level=logging.WARNING,
                node_id=nid or None,
                virtual_ip=vip or None,
                error=e,
            )


def _node_id(node: dict[str, Any]) -> str | None:
    nid = node.get("id") or node.get("ID")
    if nid is None:
        return None
    return str(nid)


def _node_online(node: dict[str, Any]) -> bool:
    for key in ("online", "Online", "connected", "Connected"):
        if key in node:
            return bool(node[key])
    return False


def _parse_last_seen(node: dict[str, Any]) -> datetime | None:
    raw = (
        node.get("lastSeen")
        or node.get("last_seen")
        or node.get("LastSeen")
        or node.get("lastSeenAt")
    )
    if raw is None:
        return None
    if isinstance(raw, (int, float)):
        # Headscale may return unix seconds or milliseconds.
        ts = float(raw)
        if ts > 1e12:
            ts /= 1000.0
        return datetime.fromtimestamp(ts, tz=timezone.utc)
    if not isinstance(raw, str):
        return None
    text = raw.strip()
    if not text or text.startswith("0001-01-01"):
        return None
    if text.endswith("Z"):
        text = text[:-1] + "+00:00"
    try:
        dt = datetime.fromisoformat(text)
    except ValueError:
        return None
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    return dt.astimezone(timezone.utc)


async def list_nodes() -> list[dict[str, Any]]:
    """Return Headscale nodes (empty list when mock/API unavailable)."""
    if settings.mock_auth_key or not settings.headscale_api_key:
        return []
    async with httpx.AsyncClient(timeout=15.0) as client:
        resp = await client.get(f"{_base()}/api/v1/node", headers=_headers())
        if resp.status_code >= 400:
            raise RuntimeError(
                f"Headscale list nodes failed ({resp.status_code}): {resp.text[:300]}"
            )
        data = resp.json()
        nodes = data.get("nodes") or data.get("Nodes") or []
        return [n for n in nodes if isinstance(n, dict)]


async def purge_offline_nodes(
    stale_after_secs: int | None = None,
) -> int:
    """Delete offline nodes whose lastSeen is older than the threshold.

    Returns the number of nodes successfully deleted. No-op when threshold <= 0
    or Headscale API is unavailable / mock mode.
    """
    stale = (
        stale_after_secs
        if stale_after_secs is not None
        else settings.headscale_node_offline_secs
    )
    if stale <= 0:
        return 0
    if settings.mock_auth_key or not settings.headscale_api_key:
        return 0

    cutoff = datetime.now(timezone.utc) - timedelta(seconds=stale)
    deleted = 0
    try:
        nodes = await list_nodes()
    except Exception as e:
        log_event(
            "headscale.node_gc_failed",
            level=logging.WARNING,
            error=e,
        )
        return 0

    async with httpx.AsyncClient(timeout=15.0) as client:
        for node in nodes:
            if _node_online(node):
                continue
            last_seen = _parse_last_seen(node)
            if last_seen is None or last_seen >= cutoff:
                continue
            nid = _node_id(node)
            if not nid:
                continue
            if await _delete_node_id(client, nid):
                deleted += 1
                log_event(
                    "headscale.node_gc",
                    node_id=nid,
                    last_seen=last_seen.isoformat(),
                    name=node.get("name") or node.get("Name") or None,
                )
            else:
                log_event(
                    "headscale.node_gc_miss",
                    level=logging.WARNING,
                    node_id=nid,
                )
    return deleted
