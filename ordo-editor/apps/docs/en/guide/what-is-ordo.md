# What is Ordo?

Ordo is an open-source rule engine. You write business rules as JSON or YAML files, and Ordo validates, tests, runs and traces them. The name is Latin for "order".

## The problem it solves

How a discount is calculated, whether an order goes through, who gets a ticket: rules like these usually live as `if/else` branches spread across services. Over time:

- Nobody can say which rules exist or where they are.
- Changing a threshold means changing code and shipping a release.
- A single rule is hard to test on its own, and hard to explain when it gives a surprising answer.

With AI writing more of the code, these branches grow faster and get harder to review.

Ordo moves the rules out of the code and into rule files. Callers pass in data and get a result back. The rules themselves can be changed, tested, reviewed and released on their own.

## Core concepts

A **ruleset** is a graph of steps. Execution starts at the entry step and ends at a terminal step. The step types are:

| Step | What it does |
| --- | --- |
| `decision` | Checks conditions in order and follows the first branch that matches |
| `decision_table` | Matches rows of a table against the input and sets output variables |
| `action` | Sets variables, writes logs and so on, then moves to the next step |
| `sub_rule` | Calls a reusable sub-rule |
| `terminal` | Ends execution and returns a result code and outputs |

**Expressions** appear in conditions and outputs, for example `order.amount >= 1000 && user.tier == "gold"`. The expression language is small on purpose: no loops, no side effects, predictable run time. See [Expression Syntax](./expression-syntax).

**Input validation** with `input_schema` declares the fields and types a rule needs. A missing field or a wrong type returns an error instead of quietly taking the wrong branch. Money can use the `decimal` type for exact arithmetic. See [Rule Structure](./rule-structure).

## Ways to run it

The same rule file runs in several ways:

- **CLI**: `ordo exec`, `ordo test` and `ordo trace` for local development and CI. See [CLI](/en/platform/cli).
- **Service**: `ordo-server` serves HTTP, gRPC and a Unix socket, with live reload, versioning and multi-tenancy. See [HTTP API](/en/api/http-api).
- **Embedded**: use `ordo-core` directly from Rust, or compile to [WebAssembly](/en/api/wasm) and run it in the browser.

When a team needs visual editing, approvals and multiple environments, add [Studio & Platform](/en/platform/overview). The engine is fully usable without it.

## Working with AI

Rules are structured files and the expression language is small, so a rule an AI writes can be compiled and checked with `ordo validate`, tested with `ordo test`, and, when the result is wrong, inspected with `ordo trace` to see which path it took. `ordo mcp` exposes these as tools to a coding agent.

[Ordo Guard](/en/platform/guard) is an example from the other direction: it uses Ordo rules to decide whether each tool call a coding agent makes is allowed, denied or needs a confirmation.

## Good fit, poor fit

A good fit: decision logic that changes often, must be exact, and has to be explained and audited, such as pricing, promotions, risk, eligibility, approvals and routing.

A poor fit: long-running processes with waits and human steps (use a workflow engine), and general computation that needs loops or heavy data processing.

## Next steps

- [Quick Start](./quick-start): write and run a first rule in five minutes
- [Rule Structure](./rule-structure): the full rule file format
- [Decision Table](./decision-table): rules as tables
