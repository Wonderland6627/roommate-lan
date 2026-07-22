from __future__ import annotations

import hashlib
import secrets
import sqlite3
import string
import time
import uuid
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path
from typing import Iterator

from .app_logging import log_event
from .settings import settings
from .traffic_log import RoomTrafficSnapshot, record_room_closed

CODE_ALPHABET = string.ascii_uppercase  # A-Z only
CODE_LEN = 4


def _now() -> int:
    return int(time.time())


def hash_code(code: str) -> str:
    normalized = normalize_code(code)
    return hashlib.sha256(normalized.encode("utf-8")).hexdigest()


def normalize_code(code: str) -> str:
    return "".join(c for c in code.strip().upper() if c.isalpha())[:CODE_LEN]


def validate_display_name(name: str) -> str:
    cleaned = " ".join(name.strip().split())
    if len(cleaned) < 1 or len(cleaned) > 16:
        raise ValueError("显示名需为 1～16 个字符")
    return cleaned


def validate_room_name(name: str) -> str:
    cleaned = " ".join(name.strip().split())
    if len(cleaned) < 1 or len(cleaned) > 32:
        raise ValueError("房间名需为 1～32 个字符")
    return cleaned


def new_member_token() -> str:
    return secrets.token_urlsafe(24)


def generate_code() -> str:
    return "".join(secrets.choice(CODE_ALPHABET) for _ in range(CODE_LEN))


@dataclass
class Room:
    id: str
    name: str
    host_member_id: str
    expires_at: int
    created_at: int
    member_count: int = 0


@dataclass
class Member:
    id: str
    room_id: str
    display_name: str
    is_host: bool
    joined_at: int
    node_id: str | None = None
    virtual_ip: str | None = None
    presence_at: int | None = None
    relay_bytes: int = 0
    p2p_bytes: int = 0


@dataclass
class PurgedRoom:
    id: str
    name: str
    traffic: RoomTrafficSnapshot | None = None


@dataclass
class LeaveResult:
    room_id: str
    room_name: str
    member_id: str
    display_name: str
    room_deleted: bool
    node_id: str | None = None
    virtual_ip: str | None = None
    traffic: RoomTrafficSnapshot | None = None


@dataclass
class DissolveResult:
    room_id: str
    room_name: str
    member_id: str
    display_name: str
    node_ids: list[tuple[str | None, str | None]] | None = None
    traffic: RoomTrafficSnapshot | None = None


@dataclass
class PresenceResult:
    member: Member
    first_presence: bool


@dataclass
class StaleMember:
    id: str
    room_id: str
    display_name: str
    node_id: str | None
    virtual_ip: str | None


@dataclass
class AddMemberResult:
    member: Member
    member_token: str
    replaced: list[StaleMember]


def _clamp_bytes(value: int | None) -> int:
    if value is None:
        return 0
    try:
        n = int(value)
    except (TypeError, ValueError):
        return 0
    if n < 0:
        return 0
    # Cap absurd values (~1 PiB) to avoid overflow noise.
    return min(n, 1 << 50)


def validate_node_id(node_id: str) -> str:
    cleaned = node_id.strip()
    if not cleaned or len(cleaned) > 128:
        raise ValueError("nodeId 无效")
    if any(c.isspace() for c in cleaned):
        raise ValueError("nodeId 无效")
    return cleaned


def validate_virtual_ip(virtual_ip: str) -> str:
    cleaned = virtual_ip.strip()
    if not cleaned or len(cleaned) > 64:
        raise ValueError("virtualIp 无效")
    # Basic IPv4 / IPv6-ish check without pulling in ipaddress edge cases for Tailscale CGNAT.
    allowed = set("0123456789abcdefABCDEF:.")
    if any(c not in allowed for c in cleaned):
        raise ValueError("virtualIp 无效")
    if cleaned.count(".") == 3:
        parts = cleaned.split(".")
        if all(p.isdigit() and 0 <= int(p) <= 255 for p in parts):
            return cleaned
    if ":" in cleaned:
        return cleaned
    raise ValueError("virtualIp 无效")


def _member_from_row(r: sqlite3.Row) -> Member:
    keys = r.keys()
    return Member(
        id=r["id"],
        room_id=r["room_id"],
        display_name=r["display_name"],
        is_host=bool(r["is_host"]),
        joined_at=r["joined_at"],
        node_id=r["node_id"] if "node_id" in keys else None,
        virtual_ip=r["virtual_ip"] if "virtual_ip" in keys else None,
        presence_at=r["presence_at"] if "presence_at" in keys else None,
        relay_bytes=int(r["relay_bytes"]) if "relay_bytes" in keys and r["relay_bytes"] is not None else 0,
        p2p_bytes=int(r["p2p_bytes"]) if "p2p_bytes" in keys and r["p2p_bytes"] is not None else 0,
    )


def _snapshot_traffic(
    conn: sqlite3.Connection,
    room_id: str,
    *,
    reason: str,
    closed_at: int | None = None,
) -> RoomTrafficSnapshot | None:
    room = conn.execute(
        "SELECT id, name, created_at FROM rooms WHERE id = ?",
        (room_id,),
    ).fetchone()
    if not room:
        return None
    members = conn.execute(
        "SELECT relay_bytes, p2p_bytes FROM members WHERE room_id = ?",
        (room_id,),
    ).fetchall()
    relay_total = 0
    p2p_total = 0
    reporters = 0
    for m in members:
        keys = m.keys()
        relay = int(m["relay_bytes"]) if "relay_bytes" in keys and m["relay_bytes"] is not None else 0
        p2p = int(m["p2p_bytes"]) if "p2p_bytes" in keys and m["p2p_bytes"] is not None else 0
        relay_total += max(0, relay)
        p2p_total += max(0, p2p)
        if relay > 0 or p2p > 0:
            reporters += 1
    return RoomTrafficSnapshot(
        room_id=room["id"],
        name=room["name"],
        reason=reason,
        created_at=int(room["created_at"]),
        closed_at=closed_at if closed_at is not None else _now(),
        member_count=len(members),
        reporters=reporters,
        relay_bytes=relay_total,
        p2p_bytes=p2p_total,
    )


class Database:
    def __init__(self, path: Path) -> None:
        self.path = path
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self._init()

    @contextmanager
    def connect(self) -> Iterator[sqlite3.Connection]:
        conn = sqlite3.connect(self.path, timeout=10)
        conn.row_factory = sqlite3.Row
        conn.execute("PRAGMA foreign_keys = ON")
        try:
            yield conn
            conn.commit()
        except Exception:
            conn.rollback()
            raise
        finally:
            conn.close()

    def _init(self) -> None:
        with self.connect() as conn:
            conn.executescript(
                """
                CREATE TABLE IF NOT EXISTS rooms (
                  id TEXT PRIMARY KEY,
                  code_hash TEXT NOT NULL UNIQUE,
                  name TEXT NOT NULL,
                  host_member_id TEXT NOT NULL,
                  expires_at INTEGER NOT NULL,
                  created_at INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS members (
                  id TEXT PRIMARY KEY,
                  room_id TEXT NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
                  member_token_hash TEXT NOT NULL UNIQUE,
                  display_name TEXT NOT NULL,
                  is_host INTEGER NOT NULL DEFAULT 0,
                  joined_at INTEGER NOT NULL,
                  node_id TEXT,
                  virtual_ip TEXT,
                  presence_at INTEGER
                );
                CREATE INDEX IF NOT EXISTS idx_rooms_expires ON rooms(expires_at);
                CREATE INDEX IF NOT EXISTS idx_members_room ON members(room_id);
                """
            )
            self._migrate_members_presence(conn)

    def _migrate_members_presence(self, conn: sqlite3.Connection) -> None:
        cols = {
            row["name"]
            for row in conn.execute("PRAGMA table_info(members)").fetchall()
        }
        if "node_id" not in cols:
            conn.execute("ALTER TABLE members ADD COLUMN node_id TEXT")
        if "virtual_ip" not in cols:
            conn.execute("ALTER TABLE members ADD COLUMN virtual_ip TEXT")
        if "presence_at" not in cols:
            conn.execute("ALTER TABLE members ADD COLUMN presence_at INTEGER")
        if "relay_bytes" not in cols:
            conn.execute(
                "ALTER TABLE members ADD COLUMN relay_bytes INTEGER NOT NULL DEFAULT 0"
            )
        if "p2p_bytes" not in cols:
            conn.execute(
                "ALTER TABLE members ADD COLUMN p2p_bytes INTEGER NOT NULL DEFAULT 0"
            )

    def purge_expired(self) -> list[PurgedRoom]:
        now = _now()
        with self.connect() as conn:
            rows = conn.execute(
                "SELECT id, name FROM rooms WHERE expires_at <= ?",
                (now,),
            ).fetchall()
            purged: list[PurgedRoom] = []
            for r in rows:
                snap = _snapshot_traffic(
                    conn, r["id"], reason="expired", closed_at=now
                )
                conn.execute("DELETE FROM rooms WHERE id = ?", (r["id"],))
                purged.append(
                    PurgedRoom(id=r["id"], name=r["name"], traffic=snap)
                )
        for room in purged:
            log_event("room.expired", room_id=room.id, name=room.name)
            if room.traffic:
                record_room_closed(room.traffic)
        return purged

    def purge_stale_hosts(
        self, stale_after_secs: int | None = None
    ) -> list[PurgedRoom]:
        """Remove rooms whose host has not reported presence recently.

        Covers: app killed without dissolve, dissolve API failed, never connected tunnel.
        """
        stale = (
            stale_after_secs
            if stale_after_secs is not None
            else settings.host_stale_secs
        )
        if stale <= 0:
            return []
        cutoff = _now() - stale
        now = _now()
        with self.connect() as conn:
            rows = conn.execute(
                """
                SELECT r.id, r.name
                FROM rooms r
                JOIN members m ON m.id = r.host_member_id
                WHERE COALESCE(m.presence_at, m.joined_at) < ?
                """,
                (cutoff,),
            ).fetchall()
            purged: list[PurgedRoom] = []
            for r in rows:
                snap = _snapshot_traffic(
                    conn, r["id"], reason="stale_host", closed_at=now
                )
                conn.execute("DELETE FROM rooms WHERE id = ?", (r["id"],))
                purged.append(
                    PurgedRoom(id=r["id"], name=r["name"], traffic=snap)
                )
        for room in purged:
            log_event("room.stale_host", room_id=room.id, name=room.name)
            if room.traffic:
                record_room_closed(room.traffic)
        return purged

    def purge_stale_members(
        self, stale_after_secs: int | None = None
    ) -> list[StaleMember]:
        """Remove non-host members that never reported presence or went silent.

        Covers: join succeeded but tunnel stuck, crash without leave, keepalive miss.
        Hosts are handled by purge_stale_hosts (whole room).
        """
        stale = (
            stale_after_secs
            if stale_after_secs is not None
            else settings.member_stale_secs
        )
        if stale <= 0:
            return []
        cutoff = _now() - stale
        with self.connect() as conn:
            rows = conn.execute(
                """
                SELECT id, room_id, display_name, node_id, virtual_ip
                FROM members
                WHERE is_host = 0
                  AND COALESCE(presence_at, joined_at) < ?
                """,
                (cutoff,),
            ).fetchall()
            purged = [
                StaleMember(
                    id=r["id"],
                    room_id=r["room_id"],
                    display_name=r["display_name"],
                    node_id=r["node_id"],
                    virtual_ip=r["virtual_ip"],
                )
                for r in rows
            ]
            for m in purged:
                conn.execute("DELETE FROM members WHERE id = ?", (m.id,))
        for m in purged:
            log_event(
                "member.purged",
                room_id=m.room_id,
                member=m.display_name,
                member_id=m.id,
                node_id=m.node_id,
                virtual_ip=m.virtual_ip,
                reason="stale",
            )
        return purged

    def create_room(self, name: str, display_name: str) -> tuple[Room, Member, str, str]:
        """Returns (room, host_member, plaintext_code, member_token)."""
        self.purge_expired()
        room_id = str(uuid.uuid4())
        member_id = str(uuid.uuid4())
        member_token = new_member_token()
        token_hash = hashlib.sha256(member_token.encode()).hexdigest()
        now = _now()
        expires = now + settings.room_ttl_hours * 3600

        with self.connect() as conn:
            code = None
            code_h = None
            for _ in range(32):
                candidate = generate_code()
                h = hash_code(candidate)
                exists = conn.execute(
                    "SELECT 1 FROM rooms WHERE code_hash = ?", (h,)
                ).fetchone()
                if not exists:
                    code = candidate
                    code_h = h
                    break
            if not code or not code_h:
                raise RuntimeError("无法生成唯一房间码，请重试")

            conn.execute(
                """
                INSERT INTO rooms (id, code_hash, name, host_member_id, expires_at, created_at)
                VALUES (?, ?, ?, ?, ?, ?)
                """,
                (room_id, code_h, name, member_id, expires, now),
            )
            conn.execute(
                """
                INSERT INTO members
                  (id, room_id, member_token_hash, display_name, is_host, joined_at)
                VALUES (?, ?, ?, ?, 1, ?)
                """,
                (member_id, room_id, token_hash, display_name, now),
            )

        room = Room(
            id=room_id,
            name=name,
            host_member_id=member_id,
            expires_at=expires,
            created_at=now,
            member_count=1,
        )
        member = Member(
            id=member_id,
            room_id=room_id,
            display_name=display_name,
            is_host=True,
            joined_at=now,
            node_id=None,
            virtual_ip=None,
            presence_at=None,
        )
        return room, member, code, member_token

    def list_rooms(self) -> list[Room]:
        self.purge_expired()
        now = _now()
        with self.connect() as conn:
            rows = conn.execute(
                """
                SELECT r.*,
                       (SELECT COUNT(*) FROM members m WHERE m.room_id = r.id) AS member_count
                FROM rooms r
                WHERE r.expires_at > ?
                ORDER BY r.created_at DESC
                """,
                (now,),
            ).fetchall()
        return [
            Room(
                id=r["id"],
                name=r["name"],
                host_member_id=r["host_member_id"],
                expires_at=r["expires_at"],
                created_at=r["created_at"],
                member_count=int(r["member_count"]),
            )
            for r in rows
        ]

    def find_room_by_code(self, code: str) -> Room | None:
        self.purge_expired()
        h = hash_code(code)
        if len(normalize_code(code)) != CODE_LEN:
            return None
        now = _now()
        with self.connect() as conn:
            r = conn.execute(
                """
                SELECT r.*,
                       (SELECT COUNT(*) FROM members m WHERE m.room_id = r.id) AS member_count
                FROM rooms r
                WHERE r.code_hash = ? AND r.expires_at > ?
                """,
                (h, now),
            ).fetchone()
        if not r:
            return None
        return Room(
            id=r["id"],
            name=r["name"],
            host_member_id=r["host_member_id"],
            expires_at=r["expires_at"],
            created_at=r["created_at"],
            member_count=int(r["member_count"]),
        )

    def get_room(self, room_id: str) -> Room | None:
        self.purge_expired()
        now = _now()
        with self.connect() as conn:
            r = conn.execute(
                """
                SELECT r.*,
                       (SELECT COUNT(*) FROM members m WHERE m.room_id = r.id) AS member_count
                FROM rooms r
                WHERE r.id = ? AND r.expires_at > ?
                """,
                (room_id, now),
            ).fetchone()
        if not r:
            return None
        return Room(
            id=r["id"],
            name=r["name"],
            host_member_id=r["host_member_id"],
            expires_at=r["expires_at"],
            created_at=r["created_at"],
            member_count=int(r["member_count"]),
        )

    def add_member(self, room_id: str, display_name: str) -> AddMemberResult:
        """Insert a guest member; replace any prior non-host rows with the same display name."""
        member_id = str(uuid.uuid4())
        member_token = new_member_token()
        token_hash = hashlib.sha256(member_token.encode()).hexdigest()
        now = _now()
        with self.connect() as conn:
            room = conn.execute(
                "SELECT id FROM rooms WHERE id = ? AND expires_at > ?",
                (room_id, now),
            ).fetchone()
            if not room:
                raise LookupError("房间不存在或已过期")
            old_rows = conn.execute(
                """
                SELECT id, room_id, display_name, node_id, virtual_ip
                FROM members
                WHERE room_id = ? AND display_name = ? AND is_host = 0
                """,
                (room_id, display_name),
            ).fetchall()
            replaced = [
                StaleMember(
                    id=r["id"],
                    room_id=r["room_id"],
                    display_name=r["display_name"],
                    node_id=r["node_id"],
                    virtual_ip=r["virtual_ip"],
                )
                for r in old_rows
            ]
            for old in replaced:
                conn.execute("DELETE FROM members WHERE id = ?", (old.id,))
            conn.execute(
                """
                INSERT INTO members
                  (id, room_id, member_token_hash, display_name, is_host, joined_at)
                VALUES (?, ?, ?, ?, 0, ?)
                """,
                (member_id, room_id, token_hash, display_name, now),
            )
        member = Member(
            id=member_id,
            room_id=room_id,
            display_name=display_name,
            is_host=False,
            joined_at=now,
            node_id=None,
            virtual_ip=None,
            presence_at=None,
        )
        return AddMemberResult(
            member=member, member_token=member_token, replaced=replaced
        )

    def list_members(self, room_id: str) -> list[Member]:
        with self.connect() as conn:
            rows = conn.execute(
                """
                SELECT * FROM members
                WHERE room_id = ?
                ORDER BY is_host DESC, joined_at ASC
                """,
                (room_id,),
            ).fetchall()
        return [_member_from_row(r) for r in rows]

    def member_by_token(self, room_id: str, member_token: str) -> Member | None:
        token_hash = hashlib.sha256(member_token.encode()).hexdigest()
        with self.connect() as conn:
            r = conn.execute(
                """
                SELECT * FROM members
                WHERE room_id = ? AND member_token_hash = ?
                """,
                (room_id, token_hash),
            ).fetchone()
        if not r:
            return None
        return _member_from_row(r)

    def update_presence(
        self,
        room_id: str,
        member_token: str,
        node_id: str,
        virtual_ip: str,
        relay_bytes: int | None = None,
        p2p_bytes: int | None = None,
    ) -> PresenceResult:
        member = self.member_by_token(room_id, member_token)
        if not member:
            raise PermissionError("无效的成员凭证")
        room = self.get_room(room_id)
        if not room:
            raise LookupError("房间不存在或已过期")
        first_presence = member.presence_at is None
        now = _now()
        relay = max(member.relay_bytes, _clamp_bytes(relay_bytes))
        p2p = max(member.p2p_bytes, _clamp_bytes(p2p_bytes))
        with self.connect() as conn:
            conn.execute(
                """
                UPDATE members
                SET node_id = ?, virtual_ip = ?, presence_at = ?,
                    relay_bytes = ?, p2p_bytes = ?
                WHERE id = ?
                """,
                (node_id, virtual_ip, now, relay, p2p, member.id),
            )
        member.node_id = node_id
        member.virtual_ip = virtual_ip
        member.presence_at = now
        member.relay_bytes = relay
        member.p2p_bytes = p2p
        return PresenceResult(member=member, first_presence=first_presence)

    def update_member_traffic(
        self,
        room_id: str,
        member_token: str,
        relay_bytes: int | None = None,
        p2p_bytes: int | None = None,
    ) -> Member:
        member = self.member_by_token(room_id, member_token)
        if not member:
            raise PermissionError("无效的成员凭证")
        relay = max(member.relay_bytes, _clamp_bytes(relay_bytes))
        p2p = max(member.p2p_bytes, _clamp_bytes(p2p_bytes))
        with self.connect() as conn:
            conn.execute(
                """
                UPDATE members
                SET relay_bytes = ?, p2p_bytes = ?
                WHERE id = ?
                """,
                (relay, p2p, member.id),
            )
        member.relay_bytes = relay
        member.p2p_bytes = p2p
        return member

    def leave(
        self,
        room_id: str,
        member_token: str,
        relay_bytes: int | None = None,
        p2p_bytes: int | None = None,
    ) -> LeaveResult:
        member = self.member_by_token(room_id, member_token)
        if not member:
            raise PermissionError("无效的成员凭证")
        if relay_bytes is not None or p2p_bytes is not None:
            self.update_member_traffic(
                room_id, member_token, relay_bytes, p2p_bytes
            )
            member = self.member_by_token(room_id, member_token) or member
        room = self.get_room(room_id)
        room_name = room.name if room else room_id
        traffic: RoomTrafficSnapshot | None = None
        with self.connect() as conn:
            remaining_before = conn.execute(
                "SELECT COUNT(*) AS c FROM members WHERE room_id = ?",
                (room_id,),
            ).fetchone()["c"]
            will_delete = remaining_before <= 1
            if will_delete:
                traffic = _snapshot_traffic(
                    conn, room_id, reason="empty_after_leave"
                )
            conn.execute("DELETE FROM members WHERE id = ?", (member.id,))
            remaining = conn.execute(
                "SELECT COUNT(*) AS c FROM members WHERE room_id = ?",
                (room_id,),
            ).fetchone()["c"]
            room_deleted = remaining == 0
            if room_deleted:
                conn.execute("DELETE FROM rooms WHERE id = ?", (room_id,))
        if traffic:
            record_room_closed(traffic)
        return LeaveResult(
            room_id=room_id,
            room_name=room_name,
            member_id=member.id,
            display_name=member.display_name,
            room_deleted=room_deleted,
            node_id=member.node_id,
            virtual_ip=member.virtual_ip,
            traffic=traffic if room_deleted else None,
        )

    def dissolve(
        self,
        room_id: str,
        member_token: str,
        relay_bytes: int | None = None,
        p2p_bytes: int | None = None,
    ) -> DissolveResult:
        member = self.member_by_token(room_id, member_token)
        if not member:
            raise PermissionError("无效的成员凭证")
        if not member.is_host:
            raise PermissionError("仅房主可解散房间")
        if relay_bytes is not None or p2p_bytes is not None:
            self.update_member_traffic(
                room_id, member_token, relay_bytes, p2p_bytes
            )
        room = self.get_room(room_id)
        room_name = room.name if room else room_id
        members = self.list_members(room_id)
        node_ids = [(m.node_id, m.virtual_ip) for m in members]
        with self.connect() as conn:
            traffic = _snapshot_traffic(conn, room_id, reason="dissolved")
            conn.execute("DELETE FROM rooms WHERE id = ?", (room_id,))
        if traffic:
            record_room_closed(traffic)
        return DissolveResult(
            room_id=room_id,
            room_name=room_name,
            member_id=member.id,
            display_name=member.display_name,
            node_ids=node_ids,
            traffic=traffic,
        )
