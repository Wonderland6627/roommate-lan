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
    normalize_code,
    validate_display_name,
    validate_node_id,
    validate_room_name,
    validate_virtual_ip,
)
from .headscale import mint_auth_key
from .rate_limit import RateLimiter
from .settings import settings

db = Database(Path(settings.data_dir) / "rooms.sqlite3")
limiter = RateLimiter(settings.rate_limit_per_minute)
join_fail_limiter = RateLimiter(settings.join_fail_limit_per_minute)


def _run_purges() -> None:
    db.purge_expired()
    db.purge_stale_hosts()


async def _ttl_loop() -> None:
    while True:
        try:
            _run_purges()
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
    forwarded = request.headers.get("x-forwarded-for")
    if forwarded:
        return forwarded.split(",")[0].strip()
    if request.client:
        return request.client.host
    return "unknown"


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
    }


@app.get("/health")
def health() -> dict[str, str]:
    return {"status": "ok"}


@app.get("/api/rooms")
def list_rooms(request: Request) -> dict[str, Any]:
    ip = _client_ip(request)
    if not limiter.allow(f"list:{ip}"):
        log_event("rate_limited", level=logging.WARNING, action="list", ip=ip)
        raise HTTPException(429, "请求过于频繁，请稍后再试")
    rooms = db.list_rooms()
    return {"rooms": [_room_public(r) for r in rooms]}


@app.post("/api/rooms")
async def create_room(body: CreateRoomBody, request: Request) -> dict[str, Any]:
    ip = _client_ip(request)
    if not limiter.allow(f"create:{ip}"):
        log_event("rate_limited", level=logging.WARNING, action="create", ip=ip)
        raise HTTPException(429, "请求过于频繁，请稍后再试")
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

    room, member, code, member_token = db.create_room(name, display)
    log_event(
        "room.created",
        room_id=room.id,
        name=room.name,
        host=member.display_name,
        member_id=member.id,
        ip=ip,
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
        log_event("rate_limited", level=logging.WARNING, action="join", ip=ip)
        raise HTTPException(429, "请求过于频繁，请稍后再试")

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
            log_event(
                "rate_limited",
                level=logging.WARNING,
                action="join_fail",
                ip=ip,
            )
            raise HTTPException(429, "错误次数过多，请稍后再试")
        log_event(
            "join.rejected",
            level=logging.WARNING,
            reason="bad_or_expired_code",
            ip=ip,
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
        member, member_token = db.add_member(room.id, display)
    except LookupError as e:
        log_event(
            "join.rejected",
            level=logging.WARNING,
            reason="room_gone",
            room_id=room.id,
            ip=ip,
            error=e,
        )
        raise HTTPException(401, str(e)) from e

    room = db.get_room(room.id) or room
    log_event(
        "member.joined",
        room_id=room.id,
        name=room.name,
        member=member.display_name,
        member_id=member.id,
        ip=ip,
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
    if not limiter.allow(f"members:{ip}"):
        log_event("rate_limited", level=logging.WARNING, action="members", ip=ip)
        raise HTTPException(429, "请求过于频繁，请稍后再试")
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
    if not limiter.allow(f"presence:{ip}"):
        log_event("rate_limited", level=logging.WARNING, action="presence", ip=ip)
        raise HTTPException(429, "请求过于频繁，请稍后再试")
    try:
        node_id = validate_node_id(body.nodeId)
        virtual_ip = validate_virtual_ip(body.virtualIp)
    except ValueError as e:
        raise HTTPException(400, str(e)) from e
    try:
        result = db.update_presence(
            room_id,
            body.memberToken,
            node_id,
            virtual_ip,
            relay_bytes=body.relayBytes,
            p2p_bytes=body.p2pBytes,
        )
    except PermissionError as e:
        log_event(
            "presence.rejected",
            level=logging.WARNING,
            room_id=room_id,
            ip=ip,
            error=e,
        )
        raise HTTPException(403, str(e)) from e
    except LookupError as e:
        log_event(
            "presence.rejected",
            level=logging.WARNING,
            room_id=room_id,
            ip=ip,
            error=e,
        )
        raise HTTPException(404, str(e)) from e
    if result.first_presence:
        log_event(
            "member.presence",
            room_id=room_id,
            member=result.member.display_name,
            member_id=result.member.id,
            node_id=result.member.node_id,
            virtual_ip=result.member.virtual_ip,
            ip=ip,
        )
    return {"status": "ok", "member": _member_public(result.member)}


@app.post("/api/rooms/{room_id}/leave")
def leave_room(room_id: str, body: TokenBody, request: Request) -> dict[str, str]:
    ip = _client_ip(request)
    if not limiter.allow(f"leave:{ip}"):
        log_event("rate_limited", level=logging.WARNING, action="leave", ip=ip)
        raise HTTPException(429, "请求过于频繁，请稍后再试")
    try:
        result = db.leave(
            room_id,
            body.memberToken,
            relay_bytes=body.relayBytes,
            p2p_bytes=body.p2pBytes,
        )
    except PermissionError as e:
        log_event(
            "leave.rejected",
            level=logging.WARNING,
            room_id=room_id,
            ip=ip,
            error=e,
        )
        raise HTTPException(403, str(e)) from e
    log_event(
        "member.left",
        room_id=result.room_id,
        name=result.room_name,
        member=result.display_name,
        member_id=result.member_id,
        room_deleted=result.room_deleted,
        ip=ip,
    )
    if result.room_deleted:
        log_event(
            "room.deleted",
            room_id=result.room_id,
            name=result.room_name,
            reason="empty_after_leave",
        )
    return {"status": "left"}


@app.post("/api/rooms/{room_id}/dissolve")
def dissolve_room(room_id: str, body: TokenBody, request: Request) -> dict[str, str]:
    ip = _client_ip(request)
    if not limiter.allow(f"dissolve:{ip}"):
        log_event("rate_limited", level=logging.WARNING, action="dissolve", ip=ip)
        raise HTTPException(429, "请求过于频繁，请稍后再试")
    try:
        result = db.dissolve(
            room_id,
            body.memberToken,
            relay_bytes=body.relayBytes,
            p2p_bytes=body.p2pBytes,
        )
    except PermissionError as e:
        log_event(
            "dissolve.rejected",
            level=logging.WARNING,
            room_id=room_id,
            ip=ip,
            error=e,
        )
        raise HTTPException(403, str(e)) from e
    log_event(
        "room.dissolved",
        room_id=result.room_id,
        name=result.room_name,
        by=result.display_name,
        member_id=result.member_id,
        ip=ip,
    )
    return {"status": "dissolved"}
