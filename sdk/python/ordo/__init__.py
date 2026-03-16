"""Ordo Rule Engine Python SDK."""

from .client import OrdoClient
from .errors import APIError, ConfigError, ConnectionError, OrdoError
from .models import (
    BatchResult,
    BatchSummary,
    EvalResult,
    ExecuteResult,
    ExecuteResultItem,
    ExecutionTrace,
    HealthStatus,
    RollbackResult,
    RuleSet,
    RuleSetConfig,
    RuleSetSummary,
    StepTrace,
    StorageStatus,
    VersionInfo,
    VersionList,
)
from .retry import RetryConfig

__version__ = "0.3.0"

__all__ = [
    "OrdoClient",
    "RetryConfig",
    # Errors
    "OrdoError",
    "APIError",
    "ConfigError",
    "ConnectionError",
    # Models
    "ExecuteResult",
    "ExecuteResultItem",
    "ExecutionTrace",
    "StepTrace",
    "BatchResult",
    "BatchSummary",
    "RuleSet",
    "RuleSetConfig",
    "RuleSetSummary",
    "VersionInfo",
    "VersionList",
    "RollbackResult",
    "EvalResult",
    "HealthStatus",
    "StorageStatus",
]
