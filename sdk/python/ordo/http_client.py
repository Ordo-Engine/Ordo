"""HTTP transport for the Ordo SDK."""

from __future__ import annotations

import json
from typing import Any

import requests

from .errors import APIError, ConnectionError as OrdoConnectionError
from .models import (
    BatchResult,
    EvalResult,
    ExecuteResult,
    HealthStatus,
    RollbackResult,
    RuleSet,
    RuleSetSummary,
    VersionInfo,
    VersionList,
    parse_batch_result,
    parse_execute_result,
    parse_health_status,
)


class HttpClient:
    """HTTP transport for Ordo API."""

    def __init__(
        self,
        address: str,
        tenant_id: str | None = None,
        timeout: float = 30.0,
        session: requests.Session | None = None,
    ):
        self._base = address.rstrip("/")
        if not self._base.endswith("/api/v1"):
            self._api = self._base + "/api/v1"
        else:
            self._api = self._base
            self._base = self._base.rsplit("/api/v1", 1)[0]
        self._tenant_id = tenant_id
        self._timeout = timeout
        self._session = session or requests.Session()

    def _headers(self) -> dict[str, str]:
        h: dict[str, str] = {"Content-Type": "application/json"}
        if self._tenant_id:
            h["X-Tenant-ID"] = self._tenant_id
        return h

    def _request(self, method: str, url: str, **kwargs: Any) -> Any:
        kwargs.setdefault("headers", self._headers())
        kwargs.setdefault("timeout", self._timeout)
        try:
            resp = self._session.request(method, url, **kwargs)
        except requests.ConnectionError as e:
            raise OrdoConnectionError(f"Failed to connect to {url}: {e}") from e
        except requests.Timeout as e:
            raise OrdoConnectionError(f"Request timed out: {e}") from e

        if resp.status_code >= 400:
            try:
                body = resp.json()
                msg = body.get("error", resp.text)
                code = body.get("code")
            except (ValueError, KeyError):
                msg = resp.text
                code = None
            raise APIError(msg, code=code, status_code=resp.status_code)

        if resp.status_code == 204 or not resp.content:
            return None
        return resp.json()

    # --- Execution ---

    def execute(self, name: str, input_data: Any, include_trace: bool = False) -> ExecuteResult:
        body: dict[str, Any] = {"input": input_data}
        if include_trace:
            body["trace"] = True
        data = self._request("POST", f"{self._api}/execute/{name}", json=body)
        return parse_execute_result(data)

    def execute_batch(self, name: str, inputs: list[Any], include_trace: bool = False) -> BatchResult:
        body: dict[str, Any] = {"inputs": inputs}
        if include_trace:
            body["include_trace"] = True
        data = self._request("POST", f"{self._api}/execute/{name}/batch", json=body)
        return parse_batch_result(data)

    # --- Rule Management ---

    def list_rulesets(self) -> list[RuleSetSummary]:
        data = self._request("GET", f"{self._api}/rulesets")
        if isinstance(data, list):
            items = data
        else:
            items = data.get("rulesets", data) if isinstance(data, dict) else []
        return [
            RuleSetSummary(
                name=r.get("name", ""),
                version=r.get("version", ""),
                description=r.get("description"),
                step_count=r.get("step_count", 0),
            )
            for r in items
        ]

    def get_ruleset(self, name: str) -> RuleSet:
        data = self._request("GET", f"{self._api}/rulesets/{name}")
        from .models import RuleSetConfig

        cfg = data.get("config", {})
        return RuleSet(
            config=RuleSetConfig(
                name=cfg.get("name", name),
                entry_step=cfg.get("entry_step", ""),
                version=cfg.get("version", ""),
            ),
            steps=data.get("steps", {}),
        )

    def create_ruleset(self, ruleset: dict[str, Any]) -> None:
        self._request("POST", f"{self._api}/rulesets", json=ruleset)

    def update_ruleset(self, name: str, ruleset: dict[str, Any]) -> None:
        # Server uses POST for both create and update (upsert)
        self._request("POST", f"{self._api}/rulesets", json=ruleset)

    def delete_ruleset(self, name: str) -> None:
        self._request("DELETE", f"{self._api}/rulesets/{name}")

    # --- Version Management ---

    def list_versions(self, name: str) -> VersionList:
        data = self._request("GET", f"{self._api}/rulesets/{name}/versions")
        versions = [
            VersionInfo(seq=v.get("seq", 0), version=v.get("version", ""), timestamp=v.get("timestamp", ""))
            for v in data.get("versions", [])
        ]
        return VersionList(
            name=data.get("name", name),
            current_version=data.get("current_version", ""),
            versions=versions,
        )

    def rollback(self, name: str, seq: int) -> RollbackResult:
        data = self._request("POST", f"{self._api}/rulesets/{name}/rollback", json={"seq": seq})
        return RollbackResult(
            status=data.get("status", ""),
            name=data.get("name", name),
            from_version=data.get("from_version", ""),
            to_version=data.get("to_version", ""),
        )

    # --- Eval ---

    def eval(self, expression: str, context: Any = None) -> EvalResult:
        body: dict[str, Any] = {"expression": expression}
        if context is not None:
            body["context"] = context
        data = self._request("POST", f"{self._api}/eval", json=body)
        return EvalResult(result=data.get("result"), parsed=data.get("parsed", ""))

    # --- Health ---

    def health(self) -> HealthStatus:
        data = self._request("GET", f"{self._base}/health")
        return parse_health_status(data)

    def close(self) -> None:
        self._session.close()
