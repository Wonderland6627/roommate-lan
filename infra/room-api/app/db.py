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

from .settings import settings

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
                  joined_at INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_rooms_expires ON rooms(expires_at);
                CREATE INDEX IF NOT EXISTS idx_members_room ON members(room_id);
                """
            )

    def purge_expired(self) -> int:
        now = _now()
        with self.connect() as conn:
            cur = conn.execute("DELETE FROM rooms WHERE expires_at <= ?", (now,))
            return cur.rowcount

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

    def add_member(self, room_id: str, display_name: str) -> tuple[Member, str]:
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
        )
        return member, member_token

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
        return [
            Member(
                id=r["id"],
                room_id=r["room_id"],
                display_name=r["display_name"],
                is_host=bool(r["is_host"]),
                joined_at=r["joined_at"],
            )
            for r in rows
        ]

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
        return Member(
            id=r["id"],
            room_id=r["room_id"],
            display_name=r["display_name"],
            is_host=bool(r["is_host"]),
            joined_at=r["joined_at"],
        )

    def leave(self, room_id: str, member_token: str) -> None:
        member = self.member_by_token(room_id, member_token)
        if not member:
            raise PermissionError("无效的成员凭证")
        with self.connect() as conn:
            conn.execute("DELETE FROM members WHERE id = ?", (member.id,))
            remaining = conn.execute(
                "SELECT COUNT(*) AS c FROM members WHERE room_id = ?",
                (room_id,),
            ).fetchone()["c"]
            if remaining == 0:
                conn.execute("DELETE FROM rooms WHERE id = ?", (room_id,))

    def dissolve(self, room_id: str, member_token: str) -> None:
        member = self.member_by_token(room_id, member_token)
        if not member:
            raise PermissionError("无效的成员凭证")
        if not member.is_host:
            raise PermissionError("仅房主可解散房间")
        with self.connect() as conn:
            conn.execute("DELETE FROM rooms WHERE id = ?", (room_id,))
