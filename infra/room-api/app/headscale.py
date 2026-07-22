from __future__ import annotations

from datetime import datetime, timedelta, timezone

import httpx

from .settings import settings


async def mint_auth_key() -> str:
    if settings.mock_auth_key:
        return settings.mock_auth_key

    if not settings.headscale_api_key:
        raise RuntimeError(
            "未配置 HEADSCALE_API_KEY。请在 headscale 容器内执行 "
            "`headscale apikeys create` 并把结果写入 infra/.env。"
        )

    expiration = datetime.now(timezone.utc) + timedelta(hours=settings.authkey_ttl_hours)
    url = settings.headscale_api_url.rstrip("/") + "/api/v1/preauthkey"
    payload = {
        "user": settings.headscale_user,
        "reusable": False,
        "ephemeral": False,
        "expiration": expiration.isoformat().replace("+00:00", "Z"),
    }
    headers = {"Authorization": f"Bearer {settings.headscale_api_key}"}

    async with httpx.AsyncClient(timeout=20.0) as client:
        resp = await client.post(url, json=payload, headers=headers)
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
