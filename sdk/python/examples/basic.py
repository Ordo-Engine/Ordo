"""Basic usage of the Ordo Python SDK."""

from ordo import OrdoClient

# Create a client
client = OrdoClient(http_address="http://localhost:8080")

# Execute a ruleset
result = client.execute("payment_check", {
    "amount": 1500,
    "currency": "USD",
    "user_level": "premium",
})
print(f"Code: {result.code}")
print(f"Message: {result.message}")
print(f"Output: {result.output}")
print(f"Duration: {result.duration_us}µs")

# List rulesets
rulesets = client.list_rulesets()
for rs in rulesets:
    print(f"  {rs.name} v{rs.version} ({rs.step_count} steps)")

# Evaluate an expression
eval_result = client.eval("amount > 1000 && user_level == 'premium'", {
    "amount": 1500,
    "user_level": "premium",
})
print(f"Eval result: {eval_result.result}")

# Health check
health = client.health()
print(f"Server: {health.status}, {health.ruleset_count} rulesets loaded")

client.close()
