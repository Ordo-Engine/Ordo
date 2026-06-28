"""Local-snapshot cache and fail-open degradation for offline resilience.

The Ordo engine is reached over the network, so an unreachable engine would
otherwise fail every caller. This module provides an opt-in last-known-good
snapshot cache plus a configurable degradation policy that kicks in *after*
retries are exhausted. It is strictly opt-in: with no ``DegradationConfig`` the
client behaves exactly as before.
"""

from __future__ import annotations

import dataclasses
import hashlib
import json
import pickle
import threading
import time
from collections import OrderedDict
from dataclasses import dataclass
from enum import Enum
from typing import Any, Optional

from .models import EvalResult, ExecuteResult


class DegradationMode(str, Enum):
    """Behavior when the engine is unreachable after retries are exhausted."""

    FAIL = "fail"
    """Propagate the error (default; preserves current behavior)."""

    STALE = "stale"
    """Serve the last-known-good cached result for this ruleset+input, flagged stale."""

    FAIL_OPEN = "fail_open"
    """Return a caller-supplied fallback result, flagged stale."""


@dataclass
class DegradationConfig:
    """Configuration for snapshot caching and degradation.

    Args:
        mode: Policy applied when a call fails after retries.
        ttl: Time-to-live for cached snapshots, in seconds.
        max_entries: Maximum number of cached snapshots (LRU eviction).
        disk_path: Optional file path to persist the cache across restarts.
        fallback: Result returned by ``execute`` under ``FAIL_OPEN``.
        eval_fallback: Result returned by ``eval`` under ``FAIL_OPEN``.
    """

    mode: DegradationMode = DegradationMode.FAIL
    ttl: float = 300.0
    max_entries: int = 1024
    disk_path: Optional[str] = None
    fallback: Optional[ExecuteResult] = None
    eval_fallback: Optional[EvalResult] = None


def _hash_key(*parts: Any) -> str:
    raw = json.dumps(parts, sort_keys=True, default=str).encode("utf-8")
    return hashlib.sha256(raw).hexdigest()


class _SnapshotCache:
    """In-memory TTL + LRU cache, with optional on-disk persistence."""

    def __init__(self, ttl: float, max_entries: int, disk_path: Optional[str] = None):
        self._ttl = ttl if ttl > 0 else 300.0
        self._max = max_entries if max_entries > 0 else 1024
        self._disk = disk_path
        self._lock = threading.Lock()
        # key -> (value, expires_at_epoch); wall-clock expiry so it survives restarts.
        self._entries: "OrderedDict[str, tuple[Any, float]]" = OrderedDict()
        if disk_path:
            self._load()

    def get(self, key: str) -> Any:
        with self._lock:
            item = self._entries.get(key)
            if item is None:
                return None
            value, expires_at = item
            if expires_at < time.time():
                del self._entries[key]
                return None
            self._entries.move_to_end(key)
            return value

    def put(self, key: str, value: Any) -> None:
        with self._lock:
            self._entries[key] = (value, time.time() + self._ttl)
            self._entries.move_to_end(key)
            while len(self._entries) > self._max:
                self._entries.popitem(last=False)
            if self._disk:
                self._save_locked()

    def _save_locked(self) -> None:
        try:
            with open(self._disk, "wb") as f:  # type: ignore[arg-type]
                pickle.dump(self._entries, f)
        except OSError:
            pass

    def _load(self) -> None:
        try:
            with open(self._disk, "rb") as f:  # type: ignore[arg-type]
                data = pickle.load(f)
            if isinstance(data, OrderedDict):
                self._entries = data
        except (OSError, pickle.PickleError, EOFError, AttributeError):
            pass


class Degrader:
    """Couples the snapshot cache with the configured degradation policy."""

    def __init__(self, config: DegradationConfig):
        self._cfg = config
        self._cache = _SnapshotCache(config.ttl, config.max_entries, config.disk_path)

    # --- caching successful results ---

    def store_execute(self, name: str, input_data: Any, result: ExecuteResult) -> None:
        self._cache.put(_hash_key("execute", name, input_data), result)

    def store_eval(self, expression: str, context: Any, result: EvalResult) -> None:
        self._cache.put(_hash_key("eval", expression, context), result)

    # --- degradation on failure ---

    def on_execute_failure(self, name: str, input_data: Any) -> Optional[ExecuteResult]:
        if self._cfg.mode == DegradationMode.STALE:
            cached = self._cache.get(_hash_key("execute", name, input_data))
            if cached is not None:
                return dataclasses.replace(cached, stale=True)
        elif self._cfg.mode == DegradationMode.FAIL_OPEN:
            if self._cfg.fallback is not None:
                return dataclasses.replace(self._cfg.fallback, stale=True)
        return None

    def on_eval_failure(self, expression: str, context: Any) -> Optional[EvalResult]:
        if self._cfg.mode == DegradationMode.STALE:
            cached = self._cache.get(_hash_key("eval", expression, context))
            if cached is not None:
                return dataclasses.replace(cached, stale=True)
        elif self._cfg.mode == DegradationMode.FAIL_OPEN:
            if self._cfg.eval_fallback is not None:
                return dataclasses.replace(self._cfg.eval_fallback, stale=True)
        return None
