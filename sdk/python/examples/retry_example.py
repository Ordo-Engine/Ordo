"""Retry and multi-tenancy example."""

from ordo import OrdoClient, RetryConfig

# Client with retry and multi-tenancy
client = OrdoClient(
    http_address="http://localhost:8080",
    tenant_id="tenant-abc",
    retry=RetryConfig(
        max_attempts=3,
        initial_interval=0.1,  # 100ms
        max_interval=5.0,      # 5s
        jitter=True,
    ),
)

# Requests will automatically retry on 5xx / 429 / connection errors
result = client.execute("risk_check", {"user_id": "u123", "action": "transfer"})
print(f"Result: {result.code} - {result.message}")

# Context manager usage (auto-close)
with OrdoClient(
    http_address="http://localhost:8080",
    retry=RetryConfig(max_attempts=5),
) as c:
    health = c.health()
    print(f"Status: {health.status}")
