from __future__ import annotations

import time
from collections import defaultdict, deque
from threading import Lock


class RateLimiter:
    def __init__(self, limit: int, window_secs: int = 60) -> None:
        self.limit = limit
        self.window = window_secs
        self._hits: dict[str, deque[float]] = defaultdict(deque)
        self._lock = Lock()

    def allow(self, key: str) -> bool:
        now = time.time()
        with self._lock:
            q = self._hits[key]
            while q and now - q[0] > self.window:
                q.popleft()
            if len(q) >= self.limit:
                return False
            q.append(now)
            return True


class LogSuppressor:
    """Suppress duplicate warning logs within a time window."""

    def __init__(self, window_secs: int = 60) -> None:
        self.window = window_secs
        self._lock = Lock()
        # key -> (window_start, suppressed_count_in_window)
        self._state: dict[str, tuple[float, int]] = {}

    def should_log(self, key: str) -> tuple[bool, int]:
        """Return (emit_now, prior_suppressed).

        First event in a window emits immediately.
        Later events in the same window are suppressed.
        First event after the window emits and reports how many were suppressed before.
        """
        now = time.time()
        with self._lock:
            prev = self._state.get(key)
            if prev is None:
                self._state[key] = (now, 0)
                return True, 0
            started, suppressed = prev
            if now - started <= self.window:
                self._state[key] = (started, suppressed + 1)
                return False, suppressed + 1
            prior_suppressed = suppressed
            self._state[key] = (now, 0)
            return True, prior_suppressed
