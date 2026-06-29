"""Tests for the local-snapshot / fail-open degradation feature."""

import pytest

from ordo import DegradationConfig, DegradationMode, OrdoClient
from ordo.errors import ConnectionError as OrdoConnectionError
from ordo.models import EvalResult, ExecuteResult


class FakeTransport:
    """Stand-in for HttpClient/GrpcClient that can be flipped to fail."""

    def __init__(self, execute_result=None, eval_result=None):
        self._execute_result = execute_result
        self._eval_result = eval_result
        self.fail = False
        self.execute_calls = 0
        self.eval_calls = 0

    def execute(self, name, input_data, include_trace=False):
        self.execute_calls += 1
        if self.fail:
            raise OrdoConnectionError("engine unreachable")
        return self._execute_result

    def eval(self, expression, context=None):
        self.eval_calls += 1
        if self.fail:
            raise OrdoConnectionError("engine unreachable")
        return self._eval_result

    def close(self):
        pass


def _client(degradation=None):
    client = OrdoClient(http_only=True, degradation=degradation)
    return client


def test_stale_serves_cached_result_flagged_stale():
    fresh = ExecuteResult(code="APPROVE", message="ok", output={"score": 1})
    client = _client(DegradationConfig(mode=DegradationMode.STALE))
    transport = FakeTransport(execute_result=fresh)
    client._http = transport

    # Prime the cache with a successful call.
    first = client.execute("loan", {"amount": 100})
    assert first.code == "APPROVE"
    assert first.stale is False

    # Engine goes down: cached last-known-good is served, flagged stale.
    transport.fail = True
    degraded = client.execute("loan", {"amount": 100})
    assert degraded.code == "APPROVE"
    assert degraded.stale is True
    client.close()


def test_default_config_fails_hard():
    # No degradation config -> current behavior, the error propagates.
    client = _client()
    transport = FakeTransport(execute_result=ExecuteResult(code="OK", message=""))
    transport.fail = True
    client._http = transport

    with pytest.raises(OrdoConnectionError):
        client.execute("loan", {"amount": 100})
    client.close()


def test_stale_without_primed_cache_still_fails():
    client = _client(DegradationConfig(mode=DegradationMode.STALE))
    transport = FakeTransport(execute_result=ExecuteResult(code="OK", message=""))
    transport.fail = True
    client._http = transport

    with pytest.raises(OrdoConnectionError):
        client.execute("loan", {"amount": 100})
    client.close()


def test_explicit_fail_mode_is_unchanged():
    client = _client(DegradationConfig(mode=DegradationMode.FAIL))
    transport = FakeTransport(execute_result=ExecuteResult(code="OK", message=""))
    client._http = transport
    client.execute("loan", {"amount": 100})  # prime cache

    transport.fail = True
    with pytest.raises(OrdoConnectionError):
        client.execute("loan", {"amount": 100})
    client.close()


def test_fail_open_returns_fallback_flagged_stale():
    fallback = ExecuteResult(code="DEFAULT_DENY", message="fallback", output=None)
    client = _client(DegradationConfig(mode=DegradationMode.FAIL_OPEN, fallback=fallback))
    transport = FakeTransport(execute_result=ExecuteResult(code="OK", message=""))
    transport.fail = True
    client._http = transport

    result = client.execute("loan", {"amount": 100})
    assert result.code == "DEFAULT_DENY"
    assert result.stale is True
    client.close()


def test_fail_open_without_fallback_fails():
    client = _client(DegradationConfig(mode=DegradationMode.FAIL_OPEN))
    transport = FakeTransport(execute_result=ExecuteResult(code="OK", message=""))
    transport.fail = True
    client._http = transport

    with pytest.raises(OrdoConnectionError):
        client.execute("loan", {"amount": 100})
    client.close()


def test_stale_keyed_by_input():
    fresh = ExecuteResult(code="APPROVE", message="ok")
    client = _client(DegradationConfig(mode=DegradationMode.STALE))
    transport = FakeTransport(execute_result=fresh)
    client._http = transport
    client.execute("loan", {"amount": 100})  # cache only this input

    transport.fail = True
    # Same input -> served stale.
    assert client.execute("loan", {"amount": 100}).stale is True
    # Different input -> no cache entry -> fails.
    with pytest.raises(OrdoConnectionError):
        client.execute("loan", {"amount": 999})
    client.close()


def test_eval_degradation():
    fresh = EvalResult(result=42, parsed="1 + 41")
    client = _client(DegradationConfig(mode=DegradationMode.STALE))
    transport = FakeTransport(eval_result=fresh)
    client._http = transport
    client.eval("1 + 41", None)

    transport.fail = True
    degraded = client.eval("1 + 41", None)
    assert degraded.result == 42
    assert degraded.stale is True
    client.close()


def test_disk_persistence_survives_restart(tmp_path):
    path = tmp_path / "snapshot.pkl"
    fresh = ExecuteResult(code="APPROVE", message="ok")

    c1 = _client(DegradationConfig(mode=DegradationMode.STALE, disk_path=str(path)))
    c1._http = FakeTransport(execute_result=fresh)
    c1.execute("loan", {"amount": 100})
    c1.close()

    # New client instance reading the same on-disk cache.
    c2 = _client(DegradationConfig(mode=DegradationMode.STALE, disk_path=str(path)))
    t2 = FakeTransport(execute_result=fresh)
    t2.fail = True
    c2._http = t2
    degraded = c2.execute("loan", {"amount": 100})
    assert degraded.code == "APPROVE"
    assert degraded.stale is True
    c2.close()
