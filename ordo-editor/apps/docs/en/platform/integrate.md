# Runtime Integration

Once a ruleset is published, your application calls the **engine** at runtime to
get a decision — one request in, one decision out, in sub-microsecond execution
time. Your app talks to the engine (the hot path), not the control plane.

> In Studio, every project has an **Integrate** tab that generates these calls
> for you — the endpoint, your project's tenant id, and copy-ready curl / Node /
> Python / Go snippets, all pre-filled for the ruleset you pick.

## The decision call

Address a ruleset by name; scope it to your project with the tenant header (your
project id is the execution tenant).

```bash
POST https://<engine>/api/v1/execute/loan-approval
Header: x-tenant-id: <project-id>
Body:   { "input": { "amount": 5000, "is_vip": true } }
```

```json
{
  "code": "APPROVED",
  "message": "Within limit",
  "output": { "approved": true, "amount": 5000 },
  "duration_us": 6
}
```

Branch your application logic on `code` (and read `output` for the computed
fields). For many inputs at once, use `POST /api/v1/execute/<name>/batch`.

## SDKs

Official SDKs wrap the REST/gRPC surface with retries and typed results.

### Python

```python
from ordo import OrdoClient

client = OrdoClient(http_address="https://<engine>", tenant_id="<project-id>")

result = client.execute("loan-approval", {"amount": 5000, "is_vip": True})
if result.code == "APPROVED":
    ...
print(result.code, result.output, f"{result.duration_us}µs")
```

### Go / Java

The `sdk/go` and `sdk/java` clients speak gRPC (`OrdoService.Execute`) with the
`x-tenant-id` metadata. See each SDK's README for the exact API.

## Transports

The engine exposes the same execution over three transports — pick per latency
and environment:

| Transport               | Use it for                                          |
| ----------------------- | --------------------------------------------------- |
| **HTTP REST** (`:8080`) | The default — easy from any language/service        |
| **gRPC** (`:50051`)     | High-throughput services; the Go/Java SDKs use it   |
| **Unix Domain Socket**  | Co-located caller on the same host — lowest latency |

See the [HTTP API](/en/api/http-api) and [gRPC API](/en/api/grpc-api) references
for the full request/response schemas.

## Where the engine runs

- **Managed** — the platform runs the engine; your published rules are callable
  without you hosting anything.
- **Self-hosted** — run `ordo-server` in your own network and connect it to the
  platform with a [connect token](/en/platform/server-registry). Ordo's engine
  is built for internal, trusted networks (auth/TLS are available but optional),
  so you can keep decisioning entirely inside your infrastructure.

## Facts vs. input

A ruleset's conditions reference **input fields**, **facts**, and **concepts**.
Concepts are derived and computed by the engine. Facts are external inputs — in
a runtime call you supply them in the `input` object (a fact that isn't supplied
evaluates as missing/null, not an error). Model the contract of each ruleset in
its [decision contract](/en/platform/contracts).
