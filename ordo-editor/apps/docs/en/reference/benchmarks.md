# Benchmarks

Measured on an Apple M1 Pro (10 cores, 16 GB), macOS, on 2026-03-11. Engine numbers use Criterion.rs; HTTP numbers use `hey` for 10 seconds per run, with the server in release mode and `--log-level error`. The test rule is a decision with four branches (`score >= 90 / 70 / 50 / default`) and the input `{"score": 75}`.

Your numbers will differ with hardware and rules. Run them yourself with `cargo bench -p ordo-core`.

## Engine

| What | Time |
| --- | --- |
| Rule with 2 steps (decision, terminal) | ~361 ns |
| Rule with 4 branches | ~573 ns |
| Same rule, compiled `.ordo` binary | ~553 ns |
| One comparison, `age > 18` | ~40 ns |
| `age > 18 && status == "active"` | ~80 ns |
| `score * 0.6 + bonus * 0.4` | ~67 ns |
| Numeric expression after JIT | ~8 ns |
| Parsing an expression (once, at load) | 1 to 2 µs |
| Batch of 1000 executions | ~2.7 million per second |

The same 4-branch rule in other Rust engines, same machine:

| Engine | Time per run |
| --- | --- |
| Ordo (bytecode VM) | 573 ns |
| Rhai (AST interpreter) | 728 ns |
| Zen Engine / GoRules | 4,200 ns |
| Plain Rust `if/else`, for reference | 3.5 ns |

## HTTP server

One instance, `POST /api/v1/execute/:name`:

| Concurrent requests | Requests/s | p50 | p99 |
| --- | --- | --- | --- |
| 1 | 14,634 | 0.06 ms | 0.10 ms |
| 10 | 47,960 | 0.19 ms | 0.40 ms |
| 50 | 59,440 | 0.76 ms | 2.90 ms |
| 200 | 62,511 | 2.78 ms | 9.90 ms |
| 500 | 60,577 | 6.60 ms | 29.2 ms |

Throughput levels off around 60,000 requests per second. Memory was 24.6 MB idle and 46.2 MB at 200 concurrent requests.

Batch execution, 50 concurrent requests:

| Inputs per request | Requests/s | Rules run/s |
| --- | --- | --- |
| 10 | 53,018 | 530,180 |
| 100 | 22,049 | 2,204,900 |
| 500 | 6,128 | 3,064,000 |

A writer and a reader synced through NATS each serve about the same as a single instance. On the same machine, together they reached 83,082 requests per second.

Other engines behind HTTP, 50 concurrent requests:

| Engine | Requests/s | p99 |
| --- | --- | --- |
| Go `net/http` with the logic hard-coded, for reference | 70,134 | 2.30 ms |
| Ordo | 58,374 | 3.40 ms |
| OPA (Go) | 30,398 | 8.50 ms |
| json-rules-engine (Node.js) | 17,205 | 6.10 ms |
| Grule (Go) | 6,547 | 16.2 ms |

Over HTTP, most of the time per request is the HTTP layer, not the rule. Ordo also does more per request than a bare handler: rule lookup, tenant and role checks, audit sampling and timing.
