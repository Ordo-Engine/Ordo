"""Unified Ordo client with automatic HTTP/gRPC transport selection."""

from __future__ import annotations

from typing import Any

from .batch import execute_batch_parallel
from .errors import ConfigError
from .http_client import HttpClient
from .models import (
    BatchResult,
    EvalResult,
    ExecuteResult,
    HealthStatus,
    RollbackResult,
    RuleSet,
    RuleSetSummary,
    VersionList,
)
from .retry import RetryConfig, retry_call


class OrdoClient:
    """Unified client for the Ordo Rule Engine.

    Supports both HTTP and gRPC transports with automatic protocol selection:
    - Execution operations prefer gRPC (lower latency) when available
    - Management operations (CRUD) always use HTTP

    Args:
        http_address: HTTP server address (e.g. "http://localhost:8080").
        grpc_address: gRPC server address (e.g. "localhost:50051").
        prefer_grpc: Prefer gRPC for execution when both are available.
        http_only: Force HTTP-only mode.
        grpc_only: Force gRPC-only mode.
        tenant_id: Default tenant ID for multi-tenancy.
        timeout: HTTP request timeout in seconds.
        retry: Retry configuration (None to disable).
        batch_concurrency: Max concurrent client-side batch executions.
    """

    def __init__(
        self,
        http_address: str = "http://localhost:8080",
        grpc_address: str | None = None,
        *,
        prefer_grpc: bool = True,
        http_only: bool = False,
        grpc_only: bool = False,
        tenant_id: str | None = None,
        timeout: float = 30.0,
        retry: RetryConfig | None = None,
        batch_concurrency: int = 10,
    ):
        if http_only and grpc_only:
            raise ConfigError("Cannot set both http_only and grpc_only")
        if grpc_only and not grpc_address:
            raise ConfigError("grpc_address is required when grpc_only=True")

        self._prefer_grpc = prefer_grpc
        self._http_only = http_only
        self._grpc_only = grpc_only
        self._retry = retry
        self._batch_concurrency = batch_concurrency

        # Initialize HTTP client
        self._http: HttpClient | None = None
        if not grpc_only:
            self._http = HttpClient(
                address=http_address,
                tenant_id=tenant_id,
                timeout=timeout,
            )

        # Initialize gRPC client (optional)
        self._grpc = None
        if not http_only and grpc_address:
            try:
                from .grpc_client import GrpcClient

                self._grpc = GrpcClient(
                    address=grpc_address,
                    tenant_id=tenant_id,
                )
            except ImportError:
                if grpc_only:
                    raise ConfigError(
                        "grpcio is required for gRPC-only mode. "
                        "Install with: pip install ordo-sdk[grpc]"
                    )

    def _use_grpc(self) -> bool:
        if self._grpc_only:
            return True
        if self._http_only:
            return False
        return self._prefer_grpc and self._grpc is not None

    def _with_retry(self, fn):  # type: ignore[no-untyped-def]
        if self._retry:
            return retry_call(self._retry, fn)
        return fn()

    # --- Execution ---

    def execute(self, name: str, input_data: Any, include_trace: bool = False) -> ExecuteResult:
        """Execute a ruleset with given input."""
        if self._use_grpc():
            return self._with_retry(lambda: self._grpc.execute(name, input_data, include_trace))  # type: ignore[union-attr]
        return self._with_retry(lambda: self._http.execute(name, input_data, include_trace))  # type: ignore[union-attr]

    def execute_batch(
        self,
        name: str,
        inputs: list[Any],
        *,
        include_trace: bool = False,
        parallel: bool = False,
    ) -> BatchResult:
        """Execute a ruleset with multiple inputs.

        Args:
            name: Ruleset name.
            inputs: List of input data.
            include_trace: Include execution traces.
            parallel: Use client-side parallel execution (thread pool).
                      When False, uses the server-side batch API.
        """
        if parallel:
            transport_execute = self._grpc.execute if self._use_grpc() else self._http.execute  # type: ignore[union-attr]
            return execute_batch_parallel(
                transport_execute, name, inputs,
                concurrency=self._batch_concurrency,
                include_trace=include_trace,
            )

        # Server-side batch
        if self._http and not self._grpc_only:
            return self._with_retry(lambda: self._http.execute_batch(name, inputs, include_trace))  # type: ignore[union-attr]
        if self._grpc:
            return self._with_retry(lambda: self._grpc.execute_batch(name, inputs, include_trace))
        raise ConfigError("No transport available for batch execution")

    # --- Rule Management (HTTP only) ---

    def _require_http(self) -> HttpClient:
        if self._http is None:
            raise ConfigError("Rule management requires HTTP transport (not available in gRPC-only mode)")
        return self._http

    def list_rulesets(self) -> list[RuleSetSummary]:
        """List all rulesets."""
        return self._require_http().list_rulesets()

    def get_ruleset(self, name: str) -> RuleSet:
        """Get a ruleset by name."""
        return self._require_http().get_ruleset(name)

    def create_ruleset(self, ruleset: dict[str, Any]) -> None:
        """Create a new ruleset."""
        self._require_http().create_ruleset(ruleset)

    def update_ruleset(self, name: str, ruleset: dict[str, Any]) -> None:
        """Update an existing ruleset."""
        self._require_http().update_ruleset(name, ruleset)

    def delete_ruleset(self, name: str) -> None:
        """Delete a ruleset."""
        self._require_http().delete_ruleset(name)

    # --- Version Management (HTTP only) ---

    def list_versions(self, name: str) -> VersionList:
        """List version history for a ruleset."""
        return self._require_http().list_versions(name)

    def rollback(self, name: str, seq: int) -> RollbackResult:
        """Rollback a ruleset to a specific version."""
        return self._require_http().rollback(name, seq)

    # --- Eval ---

    def eval(self, expression: str, context: Any = None) -> EvalResult:
        """Evaluate an expression."""
        if self._use_grpc():
            return self._with_retry(lambda: self._grpc.eval(expression, context))  # type: ignore[union-attr]
        return self._with_retry(lambda: self._http.eval(expression, context))  # type: ignore[union-attr]

    # --- Health ---

    def health(self) -> HealthStatus:
        """Check server health."""
        if self._use_grpc():
            return self._with_retry(lambda: self._grpc.health())  # type: ignore[union-attr]
        return self._with_retry(lambda: self._http.health())  # type: ignore[union-attr]

    # --- Lifecycle ---

    def close(self) -> None:
        """Close all connections."""
        if self._http:
            self._http.close()
        if self._grpc:
            self._grpc.close()

    def __enter__(self) -> OrdoClient:
        return self

    def __exit__(self, *args: Any) -> None:
        self.close()
