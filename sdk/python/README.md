# Ordo Python SDK

Python SDK for the [Ordo Rule Engine](https://github.com/Pama-Lee/Ordo) — a high-performance rule engine with sub-microsecond execution latency.

## Installation

```bash
pip install ordo-sdk

# With gRPC support
pip install ordo-sdk[grpc]
```

## Quick Start

```python
from ordo import OrdoClient

client = OrdoClient(http_address="http://localhost:8080")

# Execute a ruleset
result = client.execute("payment_check", {
    "amount": 1500,
    "currency": "USD",
    "user_level": "premium",
})
print(f"{result.code}: {result.message}")
print(f"Output: {result.output}")
print(f"Latency: {result.duration_us}µs")

client.close()
```

## Client Configuration

```python
from ordo import OrdoClient, RetryConfig

client = OrdoClient(
    http_address="http://localhost:8080",  # HTTP endpoint
    grpc_address="localhost:50051",        # gRPC endpoint (optional)
    prefer_grpc=True,                      # Use gRPC for execution when available
    tenant_id="my-tenant",                 # Multi-tenancy
    timeout=30.0,                          # HTTP timeout (seconds)
    retry=RetryConfig(                     # Retry with exponential backoff
        max_attempts=3,
        initial_interval=0.1,
        max_interval=5.0,
        jitter=True,
    ),
    batch_concurrency=10,                  # Client-side parallel batch limit
)
```

### Protocol Selection

| Mode | Execution | Management |
|------|-----------|------------|
| Default (`prefer_grpc=True`) | gRPC if available, else HTTP | HTTP |
| `http_only=True` | HTTP | HTTP |
| `grpc_only=True` | gRPC | Not available |

### Context Manager

```python
with OrdoClient(http_address="http://localhost:8080") as client:
    result = client.execute("my_rule", {"key": "value"})
```

## API Reference

### Execution

```python
# Single execution
result = client.execute("ruleset_name", {"input": "data"})
result = client.execute("ruleset_name", data, include_trace=True)

# Batch execution (server-side)
batch = client.execute_batch("ruleset_name", [input1, input2, input3])

# Batch execution (client-side parallel)
batch = client.execute_batch("ruleset_name", inputs, parallel=True)
```

### Rule Management (HTTP only)

```python
# List
rulesets = client.list_rulesets()

# Get
ruleset = client.get_ruleset("name")

# Create
client.create_ruleset({"config": {"name": "test", "entry_step": "start"}, "steps": {...}})

# Update
client.update_ruleset("name", ruleset_dict)

# Delete
client.delete_ruleset("name")
```

### Version Management (HTTP only)

```python
versions = client.list_versions("ruleset_name")
rollback = client.rollback("ruleset_name", seq=2)
```

### Expression Evaluation

```python
result = client.eval("age > 18 && status == 'active'", {"age": 25, "status": "active"})
print(result.result)  # True
```

### Health Check

```python
health = client.health()
print(health.status, health.ruleset_count)
```

## Error Handling

```python
from ordo import OrdoClient, APIError, ConnectionError, ConfigError

try:
    result = client.execute("my_rule", data)
except APIError as e:
    print(f"API error: {e} (code={e.code}, status={e.status_code})")
except ConnectionError as e:
    print(f"Connection failed: {e}")
```

### Retry

Retryable errors (automatically retried when `retry` is configured):
- HTTP 5xx server errors
- HTTP 429 rate limiting
- Connection errors and timeouts

Non-retryable errors (raised immediately):
- HTTP 4xx client errors (except 429)

## Data Models

| Class | Fields |
|-------|--------|
| `ExecuteResult` | `code`, `message`, `output`, `duration_us`, `trace` |
| `BatchResult` | `results: list[ExecuteResultItem]`, `summary: BatchSummary` |
| `RuleSetSummary` | `name`, `version`, `description`, `step_count` |
| `VersionList` | `name`, `current_version`, `versions: list[VersionInfo]` |
| `EvalResult` | `result`, `parsed` |
| `HealthStatus` | `status`, `version`, `ruleset_count`, `uptime_seconds`, `storage` |

## Requirements

- Python 3.9+
- `requests` (HTTP transport)
- `grpcio` + `protobuf` (optional, for gRPC transport)

## License

MIT
