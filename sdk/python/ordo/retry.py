"""Retry logic with exponential backoff and jitter."""

from __future__ import annotations

import random
import time
from dataclasses import dataclass
from typing import Callable, TypeVar

import requests

from .errors import APIError, ConnectionError as OrdoConnectionError

T = TypeVar("T")


@dataclass
class RetryConfig:
    """Configuration for retry behavior.

    Args:
        max_attempts: Maximum number of attempts (including the first).
        initial_interval: Initial backoff interval in seconds.
        max_interval: Maximum backoff interval in seconds.
        jitter: Whether to add ±30% random jitter.
    """

    max_attempts: int = 3
    initial_interval: float = 0.1
    max_interval: float = 5.0
    jitter: bool = True


def _is_retryable(exc: Exception) -> bool:
    if isinstance(exc, APIError):
        if exc.status_code is not None:
            return exc.status_code >= 500 or exc.status_code == 429
        return False
    if isinstance(exc, OrdoConnectionError):
        return True
    if isinstance(exc, requests.ConnectionError):
        return True
    if isinstance(exc, requests.Timeout):
        return True
    return False


def _backoff_duration(config: RetryConfig, attempt: int) -> float:
    delay = config.initial_interval * (2 ** attempt)
    delay = min(delay, config.max_interval)
    if config.jitter:
        delay *= 1.0 + random.uniform(-0.3, 0.3)  # noqa: S311
    return max(0, delay)


def retry_call(config: RetryConfig, fn: Callable[[], T]) -> T:
    """Execute fn with retry logic. Returns the result or raises the last exception."""
    last_exc: Exception | None = None
    for attempt in range(config.max_attempts):
        try:
            return fn()
        except Exception as e:
            last_exc = e
            if not _is_retryable(e):
                raise
            if attempt < config.max_attempts - 1:
                time.sleep(_backoff_duration(config, attempt))
    raise last_exc  # type: ignore[misc]
