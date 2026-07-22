from __future__ import annotations

import logging
import sys
from datetime import datetime, timedelta, timezone
from logging.handlers import TimedRotatingFileHandler
from pathlib import Path
from typing import Any


_LOGGER_NAME = "roommate.room_api"
_configured = False
# Fixed China Standard Time for log stamps (independent of container UTC).
CST = timezone(timedelta(hours=8))


def _cst_timetuple(timestamp: float):
    return datetime.fromtimestamp(timestamp, tz=CST).timetuple()


def setup_logging(log_dir: str, retain_days: int = 14) -> None:
    """Configure file + stdout logging once (safe to call repeatedly)."""
    global _configured
    if _configured:
        return

    path = Path(log_dir)
    path.mkdir(parents=True, exist_ok=True)
    log_file = path / "room-api.log"

    logger = logging.getLogger(_LOGGER_NAME)
    logger.setLevel(logging.INFO)
    logger.propagate = False
    logger.handlers.clear()

    fmt = logging.Formatter(
        "%(asctime)s %(levelname)s %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    )
    fmt.converter = _cst_timetuple  # type: ignore[method-assign]

    file_handler = TimedRotatingFileHandler(
        log_file,
        when="midnight",
        interval=1,
        backupCount=max(1, retain_days),
        encoding="utf-8",
        utc=False,
    )
    file_handler.setFormatter(fmt)
    file_handler.setLevel(logging.INFO)
    logger.addHandler(file_handler)

    stream_handler = logging.StreamHandler(sys.stdout)
    stream_handler.setFormatter(fmt)
    stream_handler.setLevel(logging.INFO)
    logger.addHandler(stream_handler)

    _configured = True


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


def log_event(
    event: str,
    *,
    level: int = logging.INFO,
    **fields: Any,
) -> None:
    """Write one structured business event line."""
    logger = logging.getLogger(_LOGGER_NAME)
    body = _format_fields(fields)
    message = f"{event} {body}".rstrip()
    logger.log(level, message)
