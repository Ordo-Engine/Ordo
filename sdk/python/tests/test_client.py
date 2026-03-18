"""Tests for OrdoClient configuration."""

import pytest

from ordo import OrdoClient
from ordo.errors import ConfigError


def test_default_client():
    client = OrdoClient()
    assert client._http is not None
    assert client._grpc is None  # No grpc address provided
    client.close()


def test_http_only():
    client = OrdoClient(http_only=True)
    assert client._http is not None
    assert client._grpc is None
    client.close()


def test_grpc_only_without_address():
    with pytest.raises(ConfigError, match="grpc_address is required"):
        OrdoClient(grpc_only=True)


def test_both_only_modes():
    with pytest.raises(ConfigError, match="Cannot set both"):
        OrdoClient(http_only=True, grpc_only=True)


def test_context_manager():
    with OrdoClient() as client:
        assert client._http is not None


def test_management_requires_http():
    # Can't test grpc_only without grpcio, so test the _require_http method
    client = OrdoClient()
    client._http = None
    with pytest.raises(ConfigError, match="Rule management requires HTTP"):
        client.list_rulesets()
    with pytest.raises(ConfigError, match="Rule management requires HTTP"):
        client.get_ruleset("test")
    with pytest.raises(ConfigError, match="Rule management requires HTTP"):
        client.create_ruleset({})
    with pytest.raises(ConfigError, match="Rule management requires HTTP"):
        client.delete_ruleset("test")
