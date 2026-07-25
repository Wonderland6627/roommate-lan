from __future__ import annotations

import asyncio
import logging
from contextlib import asynccontextmanager
from pathlib import Path
from typing import Any

from fastapi import FastAPI, HTTPException, Request
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel, Field

from .app_logging import log_event, setup_logging
from .db import (
    CODE_LEN,
    Database,
    DissolveResult,
    normalize_code,
    validate_display_name,
    validate_node_id,
    validate_room_name,
    validate_virtual_ip,
)
from .geoip import geo_label_for
from .headscale import delete_node_best_effort, mint_auth_key, purge_offline_nodes
from .rate_limit import LogSuppressor, RateLimiter
from .settings import settings

db = Database(Path(settings.data_dir) / "rooms.sqlite3")
limiter = RateLimiter(settings.rate_limit_per_minute)
join_fail_limiter = RateLimiter(settings.join_fail_limit_per_minute)
warn_suppressor = LogSuppressor(60)


async def _cleanup_nodes(
    pairs: list[tuple[str | None, str | None]],
) -> None:
    for node_id, virtual_ip in pairs:
        if not node_id and not virtual_ip:
            continue
        await delete_node_best_effort(node_id, virtual_ip)


def _run_purges() -> list[tuple[str | None, str | None]]:
    nodes: list[tuple[str | None, str | None]] = []
    for room in db.purge_expired():
        if room.node_ids:
            nodes.extend(room.node_ids)
    for room in db.purge_stale_hosts():
        if room.node_ids:
            nodes.extend(room.node_ids)
    stale_members = db.purge_stale_members()
    nodes.extend((m.node_id, m.virtual_ip) for m in stale_members)
    return nodes


async def _ttl_loop() -> None:
    while True:
        try:
            nodes = await asyncio.to_thread(_run_purges)
            await _cleanup_nodes(nodes)
            await purge_offline_nodes()
        except Exception as e:
            log_event("purge.failed", level=logging.ERROR, error=e)
        await asyncio.sleep(60)


@asynccontextmanager
async def lifespan(_app: FastAPI):
    setup_logging(settings.log_dir, settings.log_retain_days)
    log_event(
        "service.started",
        login_server=settings.login_server.rstrip("/"),
        headscale_api_url=settings.headscale_api_url.rstrip("/"),
        room_ttl_h=settings.room_ttl_hours,
        authkey_ttl_h=settings.authkey_ttl_hours,
        host_stale_secs=settings.host_stale_secs,
        member_stale_secs=settings.member_stale_secs,
        authkey_ephemeral=settings.authkey_ephemeral,
        headscale_node_offline_secs=settings.headscale_node_offline_secs,
        geoip_db_path=settings.geoip_db_path,
        log_dir=settings.log_dir,
    )
    task = asyncio.create_task(_ttl_loop())
    yield
    task.cancel()
    try:
        await task
    except asyncio.CancelledError:
        pass
    log_event("service.stopped")


app = FastAPI(title="Roommate Room API", version="1.0.0", lifespan=lifespan)
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["*"],
    allow_headers=["*"],
)


class CreateRoomBody(BaseModel):
    name: str = Field(min_length=1, max_length=32)
    displayName: str = Field(min_length=1, max_length=16)


class JoinBody(BaseModel):
    code: str = Field(min_length=1, max_length=16)
    displayName: str = Field(min_length=1, max_length=16)


class TokenBody(BaseModel):
    memberToken: str = Field(min_length=8, max_length=128)
    relayBytes: int | None = Field(default=None, ge=0)
    p2pBytes: int | None = Field(default=None, ge=0)


class PresenceBody(BaseModel):
    memberToken: str = Field(min_length=8, max_length=128)
    nodeId: str = Field(min_length=1, max_length=128)
    virtualIp: str = Field(min_length=1, max_length=64)
    relayBytes: int | None = Field(default=None, ge=0)
    p2pBytes: int | None = Field(default=None, ge=0)


def _client_ip(request: Request) -> str:
    real_ip = request.headers.get("x-real-ip")
    if real_ip and real_ip.strip():
        return real_ip.strip()
    forwarded = request.headers.get("x-forwarded-for")
    if forwarded:
        return forwarded.split(",")[0].strip()
    if request.client:
        return request.client.host
    return "unknown"


def _egress_fields(ip: str) -> tuple[str | None, str | None]:
    cleaned = (ip or "").strip()
    if not cleaned or cleaned == "unknown":
        return None, None
    label = geo_label_for(cleaned) or None
    return cleaned, label


def _room_public(room: Any) -> dict[str, Any]:
    return {
        "id": room.id,
        "name": room.name,
        "memberCount": room.member_count,
        "expiresAt": room.expires_at,
        "createdAt": room.created_at,
    }


def _member_public(m: Any) -> dict[str, Any]:
    return {
        "id": m.id,
        "displayName": m.display_name,
        "isHost": m.is_host,
        "joinedAt": m.joined_at,
        "nodeId": m.node_id,
        "virtualIp": m.virtual_ip,
        "egressIp": m.egress_ip,
        "geoLabel": m.geo_label,
    }


def _token_rate_key(action: str, token: str) -> str:
    # Use a short prefix so keys stay short; full token is still high-entropy enough.
    return f"{action}:tok:{token[:16]}"


def _warn_event(event: str, *, room_id: str | None = None, ip: str, **fields: Any) -> None:
    key = f"{event}|{ip}|{room_id or '-'}"
    emit, suppressed = warn_suppressor.should_log(key)
    if not emit:
        return
    extra: dict[str, Any] = dict(fields)
    if suppressed:
        extra["suppressed"] = suppressed
    log_event(event, level=logging.WARNING, room_id=room_id, ip=ip, **extra)


def _rate_limited(action: str, ip: str, *, room_id: str | None = None) -> None:
    _warn_event("rate_limited", room_id=room_id, ip=ip, action=action)
    raise HTTPException(429, "请求过于频繁，请稍后再试")


@app.get("/health")
def health() -> dict[str, str]:
    return {"status": "ok"}


@app.get("/api/rooms")
def list_rooms(request: Request) -> dict[str, Any]:
    ip = _client_ip(request)
    if not limiter.allow(f"list:{ip}"):
        _rate_limited("list", ip)
    rooms = db.list_rooms()
    return {"rooms": [_room_public(r) for r in rooms]}


@app.post("/api/rooms")
async def create_room(body: CreateRoomBody, request: Request) -> dict[str, Any]:
    ip = _client_ip(request)
    if not limiter.allow(f"create:{ip}"):
        _rate_limited("create", ip)
    try:
        name = validate_room_name(body.name)
        display = validate_display_name(body.displayName)
    except ValueError as e:
        raise HTTPException(400, str(e)) from e

    try:
        auth_key = await mint_auth_key()
    except Exception as e:
        log_event(
            "authkey.mint_failed",
            level=logging.WARNING,
            action="create",
            ip=ip,
            error=e,
        )
        raise HTTPException(502, f"无法签发进网凭证: {e}") from e

    egress_ip, geo_label = _egress_fields(ip)
    room, member, code, member_token = db.create_room(
        name,
        display,
        egress_ip=egress_ip,
        geo_label=geo_label,
    )
    log_event(
        "room.created",
        room_id=room.id,
        name=room.name,
        host=member.display_name,
        member_id=member.id,
        ip=ip,
        egress_ip=egress_ip,
        geo_label=geo_label,
    )
    return {
        "code": code,
        "expiresAt": room.expires_at,
        "loginServer": settings.login_server.rstrip("/"),
        "authKey": auth_key,
        "memberToken": member_token,
        "memberId": member.id,
        "isHost": True,
        "room": _room_public(room),
    }


@app.post("/api/join")
async def join_room(body: JoinBody, request: Request) -> dict[str, Any]:
    ip = _client_ip(request)
    if not limiter.allow(f"join:{ip}"):
        _rate_limited("join", ip)

    code = normalize_code(body.code)
    if len(code) != CODE_LEN:
        raise HTTPException(400, f"房间码须为 {CODE_LEN} 位字母")

    try:
        display = validate_display_name(body.displayName)
    except ValueError as e:
        raise HTTPException(400, str(e)) from e

    room = db.find_room_by_code(code)
    if not room:
        if not join_fail_limiter.allow(f"joinfail:{ip}"):
            _warn_event("rate_limited", ip=ip, action="join_fail")
            raise HTTPException(429, "错误次数过多，请稍后再试")
        _warn_event(
            "join.rejected",
            ip=ip,
            reason="bad_or_expired_code",
        )
        raise HTTPException(401, "房间码错误或房间已过期")

    try:
        auth_key = await mint_auth_key()
    except Exception as e:
        log_event(
            "authkey.mint_failed",
            level=logging.WARNING,
            action="join",
            room_id=room.id,
            ip=ip,
            error=e,
        )
        raise HTTPException(502, f"无法签发进网凭证: {e}") from e

    try:
        egress_ip, geo_label = _egress_fields(ip)
        added = db.add_member(
            room.id,
            display,
            egress_ip=egress_ip,
            geo_label=geo_label,
        )
    except LookupError as e:
        _warn_event(
            "join.rejected",
            room_id=room.id,
            ip=ip,
            reason="room_gone",
            error=e,
        )
        raise HTTPException(401, str(e)) from e

    member = added.member
    member_token = added.member_token
    if added.replaced:
        for old in added.replaced:
            log_event(
                "member.replaced",
                room_id=room.id,
                name=room.name,
                member=old.display_name,
                old_member_id=old.id,
                new_member_id=member.id,
                ip=ip,
                egress_ip=egress_ip,
                geo_label=geo_label,
            )
        await _cleanup_nodes([(m.node_id, m.virtual_ip) for m in added.replaced])

    room = db.get_room(room.id) or room
    log_event(
        "member.joined",
        room_id=room.id,
        name=room.name,
        member=member.display_name,
        member_id=member.id,
        replaced=len(added.replaced),
        ip=ip,
        egress_ip=egress_ip,
        geo_label=geo_label,
    )
    return {
        "code": code,
        "expiresAt": room.expires_at,
        "loginServer": settings.login_server.rstrip("/"),
        "authKey": auth_key,
        "memberToken": member_token,
        "memberId": member.id,
        "isHost": False,
        "room": _room_public(room),
    }


@app.get("/api/rooms/{room_id}/members")
def list_members(room_id: str, request: Request) -> dict[str, Any]:
    ip = _client_ip(request)
    token = (request.headers.get("x-member-token") or "").strip()
    if len(token) < 8:
        _warn_event(
            "members.rejected",
            room_id=room_id,
            ip=ip,
            error="缺少成员凭证",
        )
        raise HTTPException(403, "缺少成员凭证")
    if not limiter.allow(_token_rate_key("members", token)):
        _rate_limited("members", ip, room_id=room_id)
    try:
        db.require_member(room_id, token)
    except LookupError as e:
        _warn_event(
            "members.rejected",
            room_id=room_id,
            ip=ip,
            error=e,
        )
        raise HTTPException(404, str(e)) from e
    except PermissionError as e:
        _warn_event(
            "members.rejected",
            room_id=room_id,
            ip=ip,
            error=e,
        )
        raise HTTPException(403, str(e)) from e
    room = db.get_room(room_id)
    if not room:
        raise HTTPException(404, "房间不存在或已过期")
    members = db.list_members(room_id)
    return {
        "room": _room_public(room),
        "members": [_member_public(m) for m in members],
    }


@app.post("/api/rooms/{room_id}/presence")
def report_presence(
    room_id: str, body: PresenceBody, request: Request
) -> dict[str, Any]:
    ip = _client_ip(request)
    if not limiter.allow(_token_rate_key("presence", body.memberToken)):
        _rate_limited("presence", ip, room_id=room_id)
    try:
        node_id = validate_node_id(body.nodeId)
        virtual_ip = validate_virtual_ip(body.virtualIp)
    except ValueError as e:
        raise HTTPException(400, str(e)) from e
    egress_ip, geo_label = _egress_fields(ip)
    try:
        result = db.update_presence(
            room_id,
            body.memberToken,
            node_id,
            virtual_ip,
            relay_bytes=body.relayBytes,
            p2p_bytes=body.p2pBytes,
            egress_ip=egress_ip,
            geo_label=geo_label,
        )
    except PermissionError as e:
        _warn_event(
            "presence.rejected",
            room_id=room_id,
            ip=ip,
            error=e,
        )
        raise HTTPException(403, str(e)) from e
    except LookupError as e:
        _warn_event(
            "presence.rejected",
            room_id=room_id,
            ip=ip,
            error=e,
        )
        raise HTTPException(404, str(e)) from e
    if result.first_presence or result.egress_changed:
        log_event(
            "member.presence",
            room_id=room_id,
            member=result.member.display_name,
            member_id=result.member.id,
            node_id=result.member.node_id,
            virtual_ip=result.member.virtual_ip,
            ip=ip,
            egress_ip=result.member.egress_ip,
            geo_label=result.member.geo_label,
            first_presence=result.first_presence,
            egress_changed=result.egress_changed,
        )
    return {"status": "ok", "member": _member_public(result.member)}


@app.post("/api/rooms/{room_id}/leave")
async def leave_room(
    room_id: str, body: TokenBody, request: Request
) -> dict[str, str]:
    ip = _client_ip(request)
    if not limiter.allow(_token_rate_key("leave", body.memberToken)):
        _rate_limited("leave", ip, room_id=room_id)
    try:
        result = db.leave(
            room_id,
            body.memberToken,
            relay_bytes=body.relayBytes,
            p2p_bytes=body.p2pBytes,
        )
    except PermissionError as e:
        _warn_event(
            "leave.rejected",
            room_id=room_id,
            ip=ip,
            error=e,
        )
        raise HTTPException(403, str(e)) from e
    except LookupError as e:
        _warn_event(
            "leave.rejected",
            room_id=room_id,
            ip=ip,
            error=e,
        )
        raise HTTPException(404, str(e)) from e

    # Host leave is auto-promoted to dissolve to avoid host-less zombie rooms.
    if isinstance(result, DissolveResult):
        egress_ip, geo_label = _egress_fields(ip)
        log_event(
            "room.dissolved",
            room_id=result.room_id,
            name=result.room_name,
            by=result.display_name,
            member_id=result.member_id,
            ip=ip,
            egress_ip=egress_ip,
            geo_label=geo_label,
            via="leave",
        )
        if result.node_ids:
            await _cleanup_nodes(result.node_ids)
        return {"status": "dissolved"}

    log_event(
        "member.left",
        room_id=result.room_id,
        name=result.room_name,
        member=result.display_name,
        member_id=result.member_id,
        room_deleted=result.room_deleted,
        ip=ip,
        egress_ip=result.egress_ip,
        geo_label=result.geo_label,
    )
    if result.room_deleted:
        log_event(
            "room.deleted",
            room_id=result.room_id,
            name=result.room_name,
            reason="empty_after_leave",
        )
    await delete_node_best_effort(result.node_id, result.virtual_ip)
    return {"status": "left"}


@app.post("/api/rooms/{room_id}/dissolve")
async def dissolve_room(
    room_id: str, body: TokenBody, request: Request
) -> dict[str, str]:
    ip = _client_ip(request)
    if not limiter.allow(_token_rate_key("dissolve", body.memberToken)):
        _rate_limited("dissolve", ip, room_id=room_id)
    egress_ip, geo_label = _egress_fields(ip)
    try:
        result = db.dissolve(
            room_id,
            body.memberToken,
            relay_bytes=body.relayBytes,
            p2p_bytes=body.p2pBytes,
        )
    except PermissionError as e:
        _warn_event(
            "dissolve.rejected",
            room_id=room_id,
            ip=ip,
            error=e,
        )
        raise HTTPException(403, str(e)) from e
    except LookupError as e:
        _warn_event(
            "dissolve.rejected",
            room_id=room_id,
            ip=ip,
            error=e,
        )
        raise HTTPException(404, str(e)) from e
    log_event(
        "room.dissolved",
        room_id=result.room_id,
        name=result.room_name,
        by=result.display_name,
        member_id=result.member_id,
        ip=ip,
        egress_ip=egress_ip,
        geo_label=geo_label,
    )
    if result.node_ids:
        await _cleanup_nodes(result.node_ids)
    return {"status": "dissolved"}
