"""Persist per-room traffic on close and maintain a daily summary for Baota."""

from __future__ import annotations

import logging
import re
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Any

from .app_logging import CST, log_event
from .settings import settings

_DAY_LINE_RE = re.compile(
    r"^(?P<day>\d{4}-\d{2}-\d{2})\s+"
    r"rooms=(?P<rooms>\d+)\s+"
    r"relay_bytes=(?P<relay_bytes>\d+)\s+"
    r"relay=(?P<relay>\S+)\s+"
    r"p2p_bytes=(?P<p2p_bytes>\d+)\s+"
    r"p2p=(?P<p2p>\S+)\s*$"
)


@dataclass
class RoomTrafficSnapshot:
    room_id: str
    name: str
    reason: str
    created_at: int
    closed_at: int
    member_count: int
    reporters: int
    relay_bytes: int
    p2p_bytes: int


def format_bytes(n: int) -> str:
    value = float(max(0, int(n)))
    for unit in ("B", "KB", "MB", "GB", "TB"):
        if value < 1024.0 or unit == "TB":
            if unit == "B":
                return f"{int(value)}B"
            return f"{value:.1f}{unit}"
        value /= 1024.0
    return f"{int(n)}B"


def _cst_datetime(ts: int) -> datetime:
    return datetime.fromtimestamp(ts, tz=CST)


def _traffic_dir() -> Path:
    path = Path(settings.log_dir) / "traffic"
    path.mkdir(parents=True, exist_ok=True)
    return path


def _format_fields(fields: dict[str, Any]) -> str:
    parts: list[str] = []
    for key, value in fields.items():
        if value is None:
            continue
        text = str(value).replace("\n", " ").replace("\r", " ")
        if any(c.isspace() for c in text):
            text = text.replace('"', "'")
            parts.append(f'{key}="{text}"')
        else:
            parts.append(f"{key}={text}")
    return " ".join(parts)


def _append_room_line(closed_at: int, fields: dict[str, Any]) -> None:
    when = _cst_datetime(closed_at)
    day = when.strftime("%Y-%m-%d")
    stamp = when.strftime("%Y-%m-%d %H:%M:%S")
    line = f"{stamp} room.closed {_format_fields(fields)}\n"
    rooms_file = _traffic_dir() / f"rooms-{day}.log"
    with rooms_file.open("a", encoding="utf-8") as f:
        f.write(line)


def _load_daily_rows(path: Path) -> dict[str, dict[str, int]]:
    rows: dict[str, dict[str, int]] = {}
    if not path.exists():
        return rows
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as e:
        log_event(
            "traffic.daily_read_failed",
            level=logging.WARNING,
            error=e,
        )
        return rows
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        m = _DAY_LINE_RE.match(line)
        if not m:
            continue
        rows[m.group("day")] = {
            "rooms": int(m.group("rooms")),
            "relay_bytes": int(m.group("relay_bytes")),
            "p2p_bytes": int(m.group("p2p_bytes")),
        }
    return rows


def _write_daily_rows(path: Path, rows: dict[str, dict[str, int]]) -> None:
    header = (
        "# Roommate daily traffic summary (timestamps / day keys = UTC+8)\n"
        "# relay ≈ cloud DERP in+out for closed rooms; p2p does not use cloud bandwidth\n"
    )
    lines = [header]
    for day in sorted(rows.keys()):
        row = rows[day]
        lines.append(
            f"{day} rooms={row['rooms']} "
            f"relay_bytes={row['relay_bytes']} relay={format_bytes(row['relay_bytes'])} "
            f"p2p_bytes={row['p2p_bytes']} p2p={format_bytes(row['p2p_bytes'])}\n"
        )
    path.write_text("".join(lines), encoding="utf-8")


def _update_daily_summary(
    closed_at: int, relay_bytes: int, p2p_bytes: int
) -> None:
    day = _cst_datetime(closed_at).strftime("%Y-%m-%d")
    path = _traffic_dir() / "daily-summary.log"
    rows = _load_daily_rows(path)
    current = rows.get(day, {"rooms": 0, "relay_bytes": 0, "p2p_bytes": 0})
    current["rooms"] = int(current["rooms"]) + 1
    current["relay_bytes"] = int(current["relay_bytes"]) + max(0, relay_bytes)
    current["p2p_bytes"] = int(current["p2p_bytes"]) + max(0, p2p_bytes)
    rows[day] = current
    _write_daily_rows(path, rows)


def record_room_closed(snapshot: RoomTrafficSnapshot) -> None:
    """Append one room close line and bump the daily summary."""
    duration = max(0, snapshot.closed_at - snapshot.created_at)
    fields = {
        "room_id": snapshot.room_id,
        "name": snapshot.name,
        "reason": snapshot.reason,
        "members": snapshot.member_count,
        "reporters": snapshot.reporters,
        "relay_bytes": snapshot.relay_bytes,
        "relay": format_bytes(snapshot.relay_bytes),
        "p2p_bytes": snapshot.p2p_bytes,
        "p2p": format_bytes(snapshot.p2p_bytes),
        "duration_secs": duration,
    }
    try:
        _append_room_line(snapshot.closed_at, fields)
        _update_daily_summary(
            snapshot.closed_at, snapshot.relay_bytes, snapshot.p2p_bytes
        )
    except OSError as e:
        log_event(
            "traffic.write_failed",
            level=logging.ERROR,
            room_id=snapshot.room_id,
            error=e,
        )
        return
    log_event("room.traffic", **fields)
