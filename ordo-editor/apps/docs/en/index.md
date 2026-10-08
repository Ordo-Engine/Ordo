---
layout: home

hero:
  name: 'Ordo'
  text: 'Take business rules out of your code'
  tagline: Write rules in JSON or YAML, give them tests and versions, and change them without a redeploy. A Rust engine that runs a rule in microseconds. Easy for people to write, and for AI to write.
  image:
    src: /logo.png
    alt: Ordo
  actions:
    - theme: brand
      text: Quick Start
      link: /en/guide/quick-start
    - theme: alt
      text: Try it online
      link: https://ordo-engine.github.io/Ordo/
    - theme: alt
      text: GitHub
      link: https://github.com/Ordo-Engine/Ordo

features:
  - title: Rules are files
    details: Branches, decision tables and sub-rules live in JSON or YAML, in git next to your code, where they can be reviewed and diffed. The expression language is deliberately small, with no loops and no side effects, so rules an AI generates can be checked too.
    link: /en/guide/rule-structure
    linkText: Rule structure
  - title: Test before you ship
    details: Every ruleset carries its own test cases, and ordo test runs them locally and in CI. Execution can be traced step by step, so you can see how each result was reached.
    link: /en/platform/testing
    linkText: Testing rules
  - title: Runs anywhere
    details: A bytecode VM plus a Cranelift JIT. Call it over HTTP, gRPC or a Unix socket, run it in the browser as WASM, or embed it in a Rust program.
    link: /en/guide/execution-model
    linkText: Execution model
---

## What a rule looks like

Pick a discount from membership tier and order amount, written as a decision table:

```yaml
config:
  name: discount
  version: 1.0.0
  entry_step: pick_rate
steps:
  pick_rate:
    id: pick_rate
    name: Pick discount rate
    type: decision_table
    inputs: [user.tier, order.amount]
    outputs: [rate]
    rules:
      - when: [gold, ">= 1000"]
        then: [0.15]
      - when: [gold, "*"]
        then: [0.10]
      - when: ["*", ">= 1000"]
        then: [0.05]
    default: [0]
    next_step: done
  done:
    id: done
    name: Done
    type: terminal
    result:
      code: OK
      output:
        - [rate, $rate]
        - [pay, "order.amount * (1 - $rate)"]
```

```bash
$ ordo exec --rule discount.yaml --input '{"user":{"tier":"gold"},"order":{"amount":1200}}'
code:    OK
output:  {
  "rate": 0.15,
  "pay": 1020.0
}
```

To change the discount, edit the table and run the tests again. The calling code stays the same.

## Where it fits

- Pricing, promotions, loyalty points: rules that change often and must be exact.
- Risk, eligibility, approvals: every decision needs to be explainable and audited.
- Routing and assignment: orders, tickets, payment channels.
- Limits for AI agents: [Ordo Guard](/en/platform/guard) uses the same engine to allow, deny or ask before Claude Code, Codex CLI or Cursor runs a command.

## Where to start

- New to Ordo: [Quick Start](/en/guide/quick-start) gets a first rule running in five minutes.
- The concepts: [What is Ordo?](/en/guide/what-is-ordo)
- Team workflows and visual editing: [Studio & Platform](/en/platform/overview)
