"""Tests for retry logic."""

import time

import pytest

from ordo.errors import APIError, ConnectionError as OrdoConnectionError
from ordo.retry import RetryConfig, retry_call


def test_retry_succeeds_first_try():
    calls = []

    def fn():
        calls.append(1)
        return "ok"

    result = retry_call(RetryConfig(max_attempts=3), fn)
    assert result == "ok"
    assert len(calls) == 1


def test_retry_succeeds_after_failures():
    attempts = []

    def fn():
        attempts.append(1)
        if len(attempts) < 3:
            raise APIError("server error", status_code=500)
        return "ok"

    config = RetryConfig(max_attempts=3, initial_interval=0.01, jitter=False)
    result = retry_call(config, fn)
    assert result == "ok"
    assert len(attempts) == 3


def test_retry_exhausted():
    def fn():
        raise APIError("server error", status_code=500)

    config = RetryConfig(max_attempts=2, initial_interval=0.01, jitter=False)
    with pytest.raises(APIError, match="server error"):
        retry_call(config, fn)


def test_retry_non_retryable_error():
    attempts = []

    def fn():
        attempts.append(1)
        raise APIError("not found", status_code=404)

    config = RetryConfig(max_attempts=3, initial_interval=0.01)
    with pytest.raises(APIError, match="not found"):
        retry_call(config, fn)
    assert len(attempts) == 1  # No retry for 4xx


def test_retry_429_is_retryable():
    attempts = []

    def fn():
        attempts.append(1)
        if len(attempts) < 2:
            raise APIError("rate limited", status_code=429)
        return "ok"

    config = RetryConfig(max_attempts=3, initial_interval=0.01, jitter=False)
    result = retry_call(config, fn)
    assert result == "ok"
    assert len(attempts) == 2


def test_retry_backoff_timing():
    attempts = []
    timestamps = []

    def fn():
        timestamps.append(time.monotonic())
        attempts.append(1)
        if len(attempts) < 3:
            raise APIError("error", status_code=500)
        return "done"

    config = RetryConfig(max_attempts=3, initial_interval=0.05, jitter=False)
    retry_call(config, fn)
    # Second attempt should wait ~50ms, third ~100ms
    gap1 = timestamps[1] - timestamps[0]
    gap2 = timestamps[2] - timestamps[1]
    assert gap1 >= 0.04  # ~50ms with tolerance
    assert gap2 >= 0.08  # ~100ms with tolerance


def test_retry_ordo_connection_error():
    attempts = []

    def fn():
        attempts.append(1)
        if len(attempts) < 2:
            raise OrdoConnectionError("connection refused")
        return "ok"

    config = RetryConfig(max_attempts=3, initial_interval=0.01, jitter=False)
    result = retry_call(config, fn)
    assert result == "ok"
    assert len(attempts) == 2
