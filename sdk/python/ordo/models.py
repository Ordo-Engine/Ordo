"""Data models for the Ordo SDK."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Optional


@dataclass
class StepTrace:
    step_id: str
    step_name: str
    duration_us: int = 0
    result: str = ""


@dataclass
class ExecutionTrace:
    path: str = ""
    steps: list[StepTrace] = field(default_factory=list)


@dataclass
class ExecuteResult:
    code: str
    message: str
    output: Any = None
    duration_us: int = 0
    trace: Optional[ExecutionTrace] = None


@dataclass
class ExecuteResultItem:
    code: str
    message: str
    output: Any = None
    duration_us: int = 0
    trace: Optional[ExecutionTrace] = None
    error: Optional[str] = None


@dataclass
class BatchSummary:
    total: int = 0
    success: int = 0
    failed: int = 0
    total_duration_us: int = 0


@dataclass
class BatchResult:
    results: list[ExecuteResultItem] = field(default_factory=list)
    summary: BatchSummary = field(default_factory=BatchSummary)


@dataclass
class RuleSetConfig:
    name: str
    entry_step: str
    version: str = ""


@dataclass
class RuleSet:
    config: RuleSetConfig
    steps: dict[str, Any] = field(default_factory=dict)


@dataclass
class RuleSetSummary:
    name: str
    version: str = ""
    description: Optional[str] = None
    step_count: int = 0


@dataclass
class VersionInfo:
    seq: int
    version: str
    timestamp: str = ""


@dataclass
class VersionList:
    name: str
    current_version: str = ""
    versions: list[VersionInfo] = field(default_factory=list)


@dataclass
class RollbackResult:
    status: str
    name: str
    from_version: str = ""
    to_version: str = ""


@dataclass
class EvalResult:
    result: Any = None
    parsed: str = ""


@dataclass
class StorageStatus:
    mode: str = ""
    rules_dir: str = ""
    rules_count: int = 0


@dataclass
class HealthStatus:
    status: str = ""
    version: str = ""
    ruleset_count: int = 0
    uptime_seconds: int = 0
    storage: Optional[StorageStatus] = None


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


def parse_execute_result(data: dict) -> ExecuteResult:
    return ExecuteResult(
        code=data.get("code", ""),
        message=data.get("message", ""),
        output=data.get("output"),
        duration_us=data.get("duration_us", 0),
        trace=_parse_trace(data.get("trace")),
    )


def parse_batch_result(data: dict) -> BatchResult:
    items = []
    for r in data.get("results", []):
        items.append(
            ExecuteResultItem(
                code=r.get("code", ""),
                message=r.get("message", ""),
                output=r.get("output"),
                duration_us=r.get("duration_us", 0),
                trace=_parse_trace(r.get("trace")),
                error=r.get("error"),
            )
        )
    s = data.get("summary", {})
    summary = BatchSummary(
        total=s.get("total", 0),
        success=s.get("success", 0),
        failed=s.get("failed", 0),
        total_duration_us=s.get("total_duration_us", 0),
    )
    return BatchResult(results=items, summary=summary)


def parse_health_status(data: dict) -> HealthStatus:
    storage = None
    if "storage" in data and data["storage"]:
        st = data["storage"]
        storage = StorageStatus(
            mode=st.get("mode", ""),
            rules_dir=st.get("rules_dir", ""),
            rules_count=st.get("rules_count", 0),
        )
    return HealthStatus(
        status=data.get("status", ""),
        version=data.get("version", ""),
        ruleset_count=data.get("ruleset_count", 0),
        uptime_seconds=data.get("uptime_seconds", 0),
        storage=storage,
    )
