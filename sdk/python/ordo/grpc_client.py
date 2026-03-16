"""gRPC transport for the Ordo SDK (optional dependency)."""

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
    from google.protobuf import descriptor_pool, symbol_database

    HAS_GRPC = True
except ImportError:
    HAS_GRPC = False


def _check_grpc() -> None:
    if not HAS_GRPC:
        raise ImportError(
            "grpcio and protobuf are required for gRPC support. "
            "Install with: pip install ordo-sdk[grpc]"
        )


class GrpcClient:
    """gRPC transport for Ordo API.

    Uses a simple JSON-over-gRPC approach via unstructured requests,
    matching the proto service definition.
    """

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

        # Import the generated stubs dynamically, or use generic unary calls
        # We use generic calls to avoid requiring proto compilation
        self._execute = self._channel.unary_unary(
            "/ordo.v1.OrdoService/Execute",
            request_serializer=self._serialize_execute_request,
            response_deserializer=self._deserialize_execute_response,
        )
        self._batch_execute = self._channel.unary_unary(
            "/ordo.v1.OrdoService/BatchExecute",
            request_serializer=self._serialize_batch_request,
            response_deserializer=self._deserialize_batch_response,
        )
        self._eval_rpc = self._channel.unary_unary(
            "/ordo.v1.OrdoService/Eval",
            request_serializer=self._serialize_eval_request,
            response_deserializer=self._deserialize_eval_response,
        )
        self._health_rpc = self._channel.unary_unary(
            "/ordo.v1.OrdoService/Health",
            request_serializer=self._serialize_health_request,
            response_deserializer=self._deserialize_health_response,
        )

    def _metadata(self) -> list[tuple[str, str]]:
        md: list[tuple[str, str]] = []
        if self._tenant_id:
            md.append(("x-tenant-id", self._tenant_id))
        return md

    # --- Simple JSON wire format ---
    # Since we want to avoid proto compilation as a hard requirement,
    # we use a minimal hand-rolled serialization matching the proto schema.
    # This encodes messages as JSON and wraps them for the gRPC wire format.

    @staticmethod
    def _encode_json_proto(fields: dict[str, Any]) -> bytes:
        """Encode a dict as a simple JSON bytes payload for gRPC."""
        return json.dumps(fields).encode("utf-8")

    @staticmethod
    def _decode_json_proto(data: bytes) -> dict[str, Any]:
        """Decode JSON bytes from gRPC response."""
        return json.loads(data)

    # Serializers / Deserializers for each RPC

    @staticmethod
    def _serialize_execute_request(req: dict) -> bytes:
        return json.dumps(req).encode("utf-8")

    @staticmethod
    def _deserialize_execute_response(data: bytes) -> dict:
        return json.loads(data)

    @staticmethod
    def _serialize_batch_request(req: dict) -> bytes:
        return json.dumps(req).encode("utf-8")

    @staticmethod
    def _deserialize_batch_response(data: bytes) -> dict:
        return json.loads(data)

    @staticmethod
    def _serialize_eval_request(req: dict) -> bytes:
        return json.dumps(req).encode("utf-8")

    @staticmethod
    def _deserialize_eval_response(data: bytes) -> dict:
        return json.loads(data)

    @staticmethod
    def _serialize_health_request(req: dict) -> bytes:
        return json.dumps(req).encode("utf-8")

    @staticmethod
    def _deserialize_health_response(data: bytes) -> dict:
        return json.loads(data)

    def _call(self, stub: Any, request: dict, timeout: float | None = None) -> dict:
        try:
            return stub(request, metadata=self._metadata(), timeout=timeout)
        except Exception as e:
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
        req = {
            "ruleset_name": name,
            "input_json": json.dumps(input_data),
            "include_trace": include_trace,
        }
        resp = self._call(self._execute, req)
        output = None
        if resp.get("output_json"):
            try:
                output = json.loads(resp["output_json"])
            except (ValueError, TypeError):
                output = resp["output_json"]
        trace = self._parse_trace(resp.get("trace"))
        return ExecuteResult(
            code=resp.get("code", ""),
            message=resp.get("message", ""),
            output=output,
            duration_us=resp.get("duration_us", 0),
            trace=trace,
        )

    def execute_batch(
        self, name: str, inputs: list[Any], include_trace: bool = False
    ) -> BatchResult:
        req = {
            "ruleset_name": name,
            "inputs_json": [json.dumps(i) for i in inputs],
            "options": {"parallel": True, "include_trace": include_trace},
        }
        resp = self._call(self._batch_execute, req)
        items = []
        for r in resp.get("results", []):
            output = None
            if r.get("output_json"):
                try:
                    output = json.loads(r["output_json"])
                except (ValueError, TypeError):
                    output = r["output_json"]
            items.append(
                ExecuteResultItem(
                    code=r.get("code", ""),
                    message=r.get("message", ""),
                    output=output,
                    duration_us=r.get("duration_us", 0),
                    trace=self._parse_trace(r.get("trace")),
                    error=r.get("error") or None,
                )
            )
        s = resp.get("summary", {})
        summary = BatchSummary(
            total=s.get("total", 0),
            success=s.get("success", 0),
            failed=s.get("failed", 0),
            total_duration_us=s.get("total_duration_us", 0),
        )
        return BatchResult(results=items, summary=summary)

    def eval(self, expression: str, context: Any = None) -> EvalResult:
        req = {
            "expression": expression,
            "context_json": json.dumps(context) if context is not None else "{}",
        }
        resp = self._call(self._eval_rpc, req)
        result = None
        if resp.get("result_json"):
            try:
                result = json.loads(resp["result_json"])
            except (ValueError, TypeError):
                result = resp["result_json"]
        return EvalResult(result=result, parsed=resp.get("parsed_expression", ""))

    def health(self) -> HealthStatus:
        resp = self._call(self._health_rpc, {})
        status_map = {0: "unknown", 1: "serving", 2: "not_serving"}
        return HealthStatus(
            status=status_map.get(resp.get("status", 0), "unknown"),
            version=resp.get("version", ""),
            ruleset_count=resp.get("ruleset_count", 0),
            uptime_seconds=resp.get("uptime_seconds", 0),
        )

    @staticmethod
    def _parse_trace(data: dict | None) -> ExecutionTrace | None:
        if not data:
            return None
        steps = [
            StepTrace(
                step_id=s.get("step_id", ""),
                step_name=s.get("step_name", ""),
                duration_us=s.get("duration_us", 0),
                result=s.get("result", ""),
            )
            for s in data.get("steps", [])
        ]
        return ExecutionTrace(path=data.get("path", ""), steps=steps)

    def close(self) -> None:
        self._channel.close()
