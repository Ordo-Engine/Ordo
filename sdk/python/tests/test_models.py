"""Tests for model parsing."""

from ordo.models import (
    parse_batch_result,
    parse_execute_result,
    parse_health_status,
)


def test_parse_execute_result_basic():
    data = {
        "code": "APPROVED",
        "message": "Request approved",
        "output": {"risk_score": 0.2},
        "duration_us": 150,
    }
    result = parse_execute_result(data)
    assert result.code == "APPROVED"
    assert result.message == "Request approved"
    assert result.output == {"risk_score": 0.2}
    assert result.duration_us == 150
    assert result.trace is None


def test_parse_execute_result_with_trace():
    data = {
        "code": "REJECTED",
        "message": "Denied",
        "output": None,
        "duration_us": 42,
        "trace": {
            "path": "step_a -> terminal",
            "steps": [
                {"step_id": "s1", "step_name": "check", "duration_us": 10, "result": "true"},
                {"step_id": "s2", "step_name": "end", "duration_us": 5, "result": "terminal"},
            ],
        },
    }
    result = parse_execute_result(data)
    assert result.trace is not None
    assert result.trace.path == "step_a -> terminal"
    assert len(result.trace.steps) == 2
    assert result.trace.steps[0].step_id == "s1"
    assert result.trace.steps[1].result == "terminal"


def test_parse_batch_result():
    data = {
        "results": [
            {"code": "APPROVED", "message": "OK", "output": {}, "duration_us": 10},
            {"code": "REJECTED", "message": "Bad", "output": None, "duration_us": 20, "error": "timeout"},
        ],
        "summary": {"total": 2, "success": 1, "failed": 1, "total_duration_us": 30},
    }
    result = parse_batch_result(data)
    assert len(result.results) == 2
    assert result.results[0].code == "APPROVED"
    assert result.results[1].error == "timeout"
    assert result.summary.total == 2
    assert result.summary.success == 1
    assert result.summary.failed == 1


def test_parse_health_status():
    data = {
        "status": "healthy",
        "version": "0.3.0",
        "ruleset_count": 5,
        "uptime_seconds": 3600,
        "storage": {"mode": "file", "rules_dir": "/data/rules", "rules_count": 5},
    }
    result = parse_health_status(data)
    assert result.status == "healthy"
    assert result.ruleset_count == 5
    assert result.storage is not None
    assert result.storage.mode == "file"


def test_parse_health_status_no_storage():
    data = {"status": "healthy", "version": "0.3.0"}
    result = parse_health_status(data)
    assert result.storage is None


def test_parse_execute_result_missing_fields():
    data = {}
    result = parse_execute_result(data)
    assert result.code == ""
    assert result.message == ""
    assert result.output is None
    assert result.duration_us == 0
