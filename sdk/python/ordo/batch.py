"""Client-side parallel batch execution."""

from __future__ import annotations

import concurrent.futures
from typing import Any, Callable

from .models import BatchResult, BatchSummary, ExecuteResult, ExecuteResultItem


def execute_batch_parallel(
    execute_fn: Callable[[str, Any, bool], ExecuteResult],
    name: str,
    inputs: list[Any],
    concurrency: int = 10,
    include_trace: bool = False,
) -> BatchResult:
    """Execute multiple inputs in parallel using a thread pool.

    Args:
        execute_fn: The single-execute function to call for each input.
        name: Ruleset name.
        inputs: List of input data.
        concurrency: Maximum concurrent executions.
        include_trace: Whether to include traces.

    Returns:
        Aggregated BatchResult.
    """
    results: list[ExecuteResultItem] = [
        ExecuteResultItem(code="", message="") for _ in inputs
    ]
    success = 0
    failed = 0
    total_duration = 0

    def _run(idx: int, inp: Any) -> tuple[int, ExecuteResultItem]:
        try:
            r = execute_fn(name, inp, include_trace)
            return idx, ExecuteResultItem(
                code=r.code,
                message=r.message,
                output=r.output,
                duration_us=r.duration_us,
                trace=r.trace,
            )
        except Exception as e:
            return idx, ExecuteResultItem(
                code="error",
                message=str(e),
                error=str(e),
            )

    with concurrent.futures.ThreadPoolExecutor(max_workers=concurrency) as pool:
        futures = [pool.submit(_run, i, inp) for i, inp in enumerate(inputs)]
        for future in concurrent.futures.as_completed(futures):
            idx, item = future.result()
            results[idx] = item

    for item in results:
        if item.error:
            failed += 1
        else:
            success += 1
        total_duration += item.duration_us

    return BatchResult(
        results=results,
        summary=BatchSummary(
            total=len(inputs),
            success=success,
            failed=failed,
            total_duration_us=total_duration,
        ),
    )
