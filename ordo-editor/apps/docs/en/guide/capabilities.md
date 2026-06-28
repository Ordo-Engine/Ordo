# Capabilities and External Calls

Ordo uses a capability boundary for outbound side effects and runtime integrations. This keeps rule execution deterministic inside the engine while still allowing rules and server components to call metrics, audit sinks, HTTP endpoints, and other providers through a stable interface.

## What a capability is

A capability provider exposes a named runtime service plus one or more operations.

- The provider name is the capability name, such as `metrics.prometheus`, `audit.logger`, or `network.http`.
- The operation is the method invoked on that capability, such as `gauge`, `rule_executed`, or `post`.
- The payload is a typed object that the provider receives and returns.

At runtime, `ExternalCall` actions are translated into a capability request:

```json
{
  "action": "external_call",
  "service": "demo.echo",
  "method": "echo",
  "params": [["amount", { "Field": "amount" }]],
  "result_variable": "echo_result",
  "timeout_ms": 250
}
```

If `result_variable` is set, Ordo stores the capability response under that variable:

- `$echo_result.capability`
- `$echo_result.operation`
- `$echo_result.payload`
- `$echo_result.metadata`

## Built-in server capabilities

The server currently registers these capability providers by default:

| Capability           | Category  | Typical operations                                         | Purpose                                         |
| -------------------- | --------- | ---------------------------------------------------------- | ----------------------------------------------- |
| `metrics.prometheus` | `action`  | `gauge`, `counter`                                         | Record rule metrics through the Prometheus sink |
| `audit.logger`       | `action`  | `rule_executed`                                            | Emit structured execution audit events          |
| `network.http`       | `network` | `get`, `post`, `put`, `patch`, `delete`, `head`, `options` | Send outbound HTTP requests                     |

## Studio `externalCalls` mapping

Studio action steps can define `externalCalls`. The editor adapter now converts them into engine `external_call` actions using these rules.

### HTTP calls

Use `type: "http"` when the target is an outbound HTTP endpoint.

```ts
{
  type: 'http',
  target: 'PATCH https://api.example.com/score',
  params: {
    applicantId: Expr.variable('$.applicant.id'),
    score: Expr.number(720),
  },
  resultVariable: 'http_result',
  timeout: 1500,
}
```

This becomes:

```json
{
  "action": "external_call",
  "service": "network.http",
  "method": "patch",
  "params": [
    ["url", { "Literal": "https://api.example.com/score" }],
    [
      "json_body",
      {
        "Object": [
          ["applicantId", { "Field": "applicant.id" }],
          ["score", { "Literal": 720 }]
        ]
      }
    ]
  ],
  "result_variable": "http_result",
  "timeout_ms": 1500
}
```

Rules:

- If `target` starts with `METHOD <space> URL`, that HTTP method is used.
- If no method prefix is provided, Ordo defaults to `POST`.
- `params` are sent as `json_body`.
- `target` becomes the `url` payload field for `network.http`.

### Function and gRPC calls

For `type: "function"` and `type: "grpc"`, the editor treats `target` as a capability reference.

Supported target forms:

- `demo.echo`
- `demo.echo#echo`
- `demo.echo::echo`

Rules:

- `service` is the capability name.
- `method` is parsed from `#` or `::` when present.
- If no method is supplied, Ordo defaults to `invoke` for `function` and `call` for `grpc`.
- `params` are passed through as the capability payload object.

Example:

```ts
{
  type: 'function',
  target: 'demo.echo#echo',
  params: {
    payload: Expr.object({
      amount: Expr.variable('$.amount'),
      approved: Expr.boolean(true),
    }),
  },
  resultVariable: 'echo_result',
}
```

## Expression support in capability payloads

Capability payload values can use the same expression model as normal rule actions. The editor adapter now serializes:

- literals
- field references
- arrays
- objects
- binary and unary expressions
- conditional expressions
- function calls
- simple member paths like `$.user.profile.id`

## Current limitations

Studio already models some fields that the engine does not execute yet:

- `retry`
- `onError`
- `fallbackValue`

These fields remain editor-level metadata today. They are preserved in the Studio model, but they are not translated into runtime behavior by the engine adapter yet.

That means:

- retries are not applied automatically by `ExternalCall`
- fallback values are not injected automatically on failure
- error handling modes are not yet mapped to engine semantics

If you need those behaviors today, implement them inside the capability provider itself or keep the rule logic explicit in separate steps.

## Example provider

The repository includes a minimal provider example in [`examples/capability-demo`](https://github.com/Ordo-Engine/Ordo/tree/main/examples/capability-demo). It registers a `demo.echo` provider, invokes it from an `ExternalCall`, and reads the result through `$result.payload`.
