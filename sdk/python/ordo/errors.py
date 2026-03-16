"""Exception types for the Ordo SDK."""

from __future__ import annotations

from typing import Optional


class OrdoError(Exception):
    """Base exception for all Ordo SDK errors."""


class APIError(OrdoError):
    """Error returned by the Ordo API."""

    def __init__(self, message: str, code: Optional[str] = None, status_code: Optional[int] = None):
        self.code = code
        self.status_code = status_code
        super().__init__(message)


class ConfigError(OrdoError):
    """Invalid client configuration."""


class ConnectionError(OrdoError):
    """Failed to connect to the Ordo server."""
