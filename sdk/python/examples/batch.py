"""Batch execution example."""

from ordo import OrdoClient

client = OrdoClient(http_address="http://localhost:8080")

# Server-side batch (single HTTP request)
inputs = [
    {"amount": 100, "user_level": "basic"},
    {"amount": 5000, "user_level": "premium"},
    {"amount": 50000, "user_level": "basic"},
]
result = client.execute_batch("payment_check", inputs)

print(f"Total: {result.summary.total}")
print(f"Success: {result.summary.success}")
print(f"Failed: {result.summary.failed}")
for i, item in enumerate(result.results):
    status = f"ERROR: {item.error}" if item.error else item.code
    print(f"  [{i}] {status}: {item.message}")

# Client-side parallel batch (thread pool)
result = client.execute_batch("payment_check", inputs, parallel=True)
print(f"\nParallel total duration: {result.summary.total_duration_us}µs")

client.close()
