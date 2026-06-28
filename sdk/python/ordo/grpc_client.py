"""gRPC transport for the Ordo SDK (optional dependency).

Requires compiled protobuf stubs. Generate them with:
    python -m grpc_tools.protoc -I proto \
        --python_out=ordo/_proto --grpc_python_out=ordo/_proto proto/ordo.proto
"""

from __future__ import annotations

import json
from typing import Any

from .errors import APIError, ConnectionError as OrdoConnectionError
from .models import (
    BatchResult,
    BatchSummary,
    EvalResult,
    ExecuteResult,
    ExecuteResultItem,
    ExecutionTrace,
    HealthStatus,
    StepTrace,
)

try:
    import grpc

    HAS_GRPC = True
except ImportError:
    HAS_GRPC = False

# Try to import compiled proto stubs
_pb2 = None
_pb2_grpc = None
try:
    from ordo._proto import ordo_pb2 as _pb2  # type: ignore[assignment]
    from ordo._proto import ordo_pb2_grpc as _pb2_grpc  # type: ignore[assignment]

    HAS_STUBS = True
except ImportError:
    HAS_STUBS = False


def _check_grpc() -> None:
    if not HAS_GRPC:
        raise ImportError(
            "grpcio and protobuf are required for gRPC support. "
            "Install with: pip install ordo-sdk[grpc]"
        )
    if not HAS_STUBS:
        raise ImportError(
            "Compiled proto stubs not found. Generate them with:\n"
            "  pip install grpcio-tools\n"
            "  python -m grpc_tools.protoc -I proto "
            "--python_out=ordo/_proto --grpc_python_out=ordo/_proto proto/ordo.proto"
        )


class GrpcClient:
    """gRPC transport for Ordo API using compiled protobuf stubs."""

    def __init__(
        self,
        address: str,
        tenant_id: str | None = None,
        options: list[tuple[str, Any]] | None = None,
    ):
        _check_grpc()
        self._address = address
        self._tenant_id = tenant_id
        self._channel = grpc.insecure_channel(address, options=options)
        self._stub = _pb2_grpc.OrdoServiceStub(self._channel)  # type: ignore[union-attr]

    def _metadata(self) -> list[tuple[str, str]]:
        md: list[tuple[str, str]] = []
        if self._tenant_id:
            md.append(("x-tenant-id", self._tenant_id))
        return md

    def _handle_rpc_error(self, e: Exception) -> None:
        if HAS_GRPC and isinstance(e, grpc.RpcError):
            code = e.code()  # type: ignore[union-attr]
            details = e.details()  # type: ignore[union-attr]
            raise APIError(
                f"gRPC error: {details}",
                code=code.name if code else None,
                status_code=code.value[0] if code else None,
            ) from e
        raise OrdoConnectionError(f"gRPC call failed: {e}") from e

    # --- Public API ---

    def execute(self, name: str, input_data: Any, include_trace: bool = False) -> ExecuteResult:
        req = _pb2.ExecuteRequest(  # type: ignore[union-attr]
            ruleset_name=name,
            input_json=json.dumps(input_data),
            include_trace=include_trace,
        )
        try:
            resp = self._stub.Execute(req, metadata=self._metadata())
        except Exception as e:
            self._handle_rpc_error(e)
            raise  # unreachable, but satisfies type checker

        output = None
        if resp.output_json:
            try:
                output = json.loads(resp.output_json)
            except (ValueError, TypeError):
                output = resp.output_json
        trace = self._parse_proto_trace(resp.trace)
        return ExecuteResult(
            code=resp.code,
            message=resp.message,
            output=output,
            duration_us=resp.duration_us,
            trace=trace,
        )

    def execute_batch(
        self, name: str, inputs: list[Any], include_trace: bool = False
    ) -> BatchResult:
        options = _pb2.BatchExecuteOptions(  # type: ignore[union-attr]
            parallel=True, include_trace=include_trace
        )
        req = _pb2.BatchExecuteRequest(  # type: ignore[union-attr]
            ruleset_name=name,
            inputs_json=[json.dumps(i) for i in inputs],
            options=options,
        )
        try:
            resp = self._stub.BatchExecute(req, metadata=self._metadata())
        except Exception as e:
            self._handle_rpc_error(e)
            raise

        items = []
        for r in resp.results:
            output = None
            if r.output_json:
                try:
                    output = json.loads(r.output_json)
                except (ValueError, TypeError):
                    output = r.output_json
            items.append(
                ExecuteResultItem(
                    code=r.code,
                    message=r.message,
                    output=output,
                    duration_us=r.duration_us,
                    trace=self._parse_proto_trace(r.trace),
                    error=r.error or None,
                )
            )
        summary = BatchSummary(
            total=resp.summary.total,
            success=resp.summary.success,
            failed=resp.summary.failed,
            total_duration_us=resp.summary.total_duration_us,
        )
        return BatchResult(results=items, summary=summary)

    def eval(self, expression: str, context: Any = None) -> EvalResult:
        req = _pb2.EvalRequest(  # type: ignore[union-attr]
            expression=expression,
            context_json=json.dumps(context) if context is not None else "{}",
        )
        try:
            resp = self._stub.Eval(req, metadata=self._metadata())
        except Exception as e:
            self._handle_rpc_error(e)
            raise

        result = None
        if resp.result_json:
            try:
                result = json.loads(resp.result_json)
            except (ValueError, TypeError):
                result = resp.result_json
        return EvalResult(result=result, parsed=resp.parsed_expression)

    def health(self) -> HealthStatus:
        req = _pb2.HealthRequest()  # type: ignore[union-attr]
        try:
            resp = self._stub.Health(req, metadata=self._metadata())
        except Exception as e:
            self._handle_rpc_error(e)
            raise

        status_map = {0: "unknown", 1: "serving", 2: "not_serving"}
        return HealthStatus(
            status=status_map.get(resp.status, "unknown"),
            version=resp.version,
            ruleset_count=resp.ruleset_count,
            uptime_seconds=resp.uptime_seconds,
        )

    @staticmethod
    def _parse_proto_trace(trace: Any) -> ExecutionTrace | None:
        if not trace or not trace.path:
            return None
        steps = [
            StepTrace(
                step_id=s.step_id,
                step_name=s.step_name,
                duration_us=s.duration_us,
                result=s.result,
            )
            for s in trace.steps
        ]
        return ExecutionTrace(path=trace.path, steps=steps)

    def close(self) -> None:
        self._channel.close()
