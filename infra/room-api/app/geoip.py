"""Offline DB-IP City Lite MMDB lookups for egress IP geolocation."""

from __future__ import annotations

import logging
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .settings import settings

_log = logging.getLogger("roommate.room_api")

_reader: Any | None = None
_load_attempted = False


@dataclass(frozen=True)
class GeoResult:
    country: str
    region: str
    city: str
    geo_label: str


def _localized_name(names: dict[str, str] | None) -> str:
    if not names:
        return ""
    for key in ("zh-CN", "zh-Hans", "zh", "en"):
        value = names.get(key)
        if value and value.strip():
            return value.strip()
    for value in names.values():
        if value and str(value).strip():
            return str(value).strip()
    return ""


def _build_label(country: str, region: str, city: str) -> str:
    parts: list[str] = []
    for part in (country, region, city):
        if not part or part in parts:
            continue
        parts.append(part)
    return " · ".join(parts)


def _ensure_reader() -> Any | None:
    global _reader, _load_attempted
    if _load_attempted:
        return _reader
    _load_attempted = True
    path = Path(settings.geoip_db_path)
    if not path.is_file():
        _log.info("geoip.db_missing path=%s (geo labels disabled)", path)
        return None
    try:
        from geoip2 import database  # type: ignore[import-untyped]

        _reader = database.Reader(str(path))
        _log.info("geoip.db_loaded path=%s", path)
    except Exception as e:
        _log.warning("geoip.db_open_failed path=%s error=%s", path, e)
        _reader = None
    return _reader


def lookup(ip: str) -> GeoResult:
    """Return country/region/city for a public IP; empty on any failure."""
    cleaned = (ip or "").strip()
    empty = GeoResult(country="", region="", city="", geo_label="")
    if not cleaned or cleaned in {"unknown", "127.0.0.1", "::1"}:
        return empty
    if cleaned.startswith("10.") or cleaned.startswith("192.168."):
        return empty
    if cleaned.startswith("172."):
        try:
            second = int(cleaned.split(".")[1])
            if 16 <= second <= 31:
                return empty
        except (IndexError, ValueError):
            pass

    reader = _ensure_reader()
    if reader is None:
        return empty
    try:
        resp = reader.city(cleaned)
    except Exception:
        return empty

    country = _localized_name(getattr(resp.country, "names", None))
    if not country:
        country = _localized_name(getattr(resp.registered_country, "names", None))
    region = ""
    subdivisions = getattr(resp, "subdivisions", None)
    if subdivisions:
        most_specific = getattr(subdivisions, "most_specific", None)
        if most_specific is not None:
            region = _localized_name(getattr(most_specific, "names", None))
    city = _localized_name(getattr(resp.city, "names", None))
    return GeoResult(
        country=country,
        region=region,
        city=city,
        geo_label=_build_label(country, region, city),
    )


def geo_label_for(ip: str) -> str:
    return lookup(ip).geo_label
