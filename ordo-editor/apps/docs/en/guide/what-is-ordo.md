# What is Ordo?

**Ordo** (Latin for "order") is an open-source decision platform for teams who need to **own their decision logic** — not scatter it across codebases, spreadsheets, and tribal knowledge.

It has three layers:

- **Engine** — sub-microsecond rule execution, JIT-compiled via Cranelift, runs everywhere (HTTP · gRPC · WASM · CLI).
- **Platform** — org and project management, fact catalog, decision contracts, version history, rule templates.
- **Studio** — visual flow editor, test case management, one-click template instantiation.

## Why a Decision Platform?

Most teams start with a rule engine. Then they realize the hard part isn't execution speed — it's knowing what rules exist, who changed them, whether they still work, and how to hand them off to a new engineer.

Ordo addresses the full lifecycle:

| Stage | What Ordo provides |
|-------|--------------------|
| **Author** | Studio flow editor, decision tables, template library |
| **Test** | Per-ruleset test cases, run in CI, export to YAML |
| **Govern** | Fact catalog, typed contracts, version history, audit log |
| **Execute** | Fast engine, hot reload, multi-tenancy |
| **Observe** | Execution traces, Prometheus metrics, structured logs |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Platform (ordo-platform)                  │
│  Org · Project · Fact Catalog · Contracts · Templates · Tests│
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                      Studio (apps/studio)                    │
│         Flow Editor · Test Runner · Template Library         │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                     Engine (ordo-server)                     │
│   HTTP REST · gRPC · Unix Socket · Prometheus metrics        │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                      ordo-core (Rust)                        │
│   Interpreter · Bytecode VM · Cranelift JIT · WASM           │
└─────────────────────────────────────────────────────────────┘
```

## Use Cases

Ordo is a good fit whenever business logic needs to be **visible, testable, and changeable** without a full deploy:

- **Risk & compliance** — credit scoring, fraud detection, KYC, regulatory policy
- **Pricing & promotions** — dynamic pricing, discount rules, campaign eligibility
- **Routing & assignment** — order routing, payment channel selection, load decisions
- **Access & eligibility** — loan approval, feature flags, subscription tier logic
