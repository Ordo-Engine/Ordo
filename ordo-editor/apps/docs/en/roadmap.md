---
outline: [2, 3]
---

# Roadmap

> Ordo is decision infrastructure for modern software teams.
> This roadmap outlines what we're building and where we're heading.

## Current State (v0.x)

Ordo already ships a production-grade core:

| Module            | Capabilities                                                                                       |
| ----------------- | -------------------------------------------------------------------------------------------------- |
| **Engine**        | Sub-microsecond rule execution, bytecode VM + Cranelift JIT, expression optimizer                  |
| **Transports**    | HTTP REST, gRPC (with TLS/mTLS), Unix Domain Socket                                                |
| **Visual Editor** | Three editing modes (Form / Flow Graph / JSON), decision tables, execution & performance panels    |
| **CLI**           | `ordo eval`, `ordo exec`, `ordo test`                                                              |
| **WASM**          | Run the engine in browsers                                                                         |
| **SDKs**          | Go, Java, Python                                                                                   |
| **Studio**        | Org/project/member management, fact catalog, concept registry, decision contracts, version history |
| **Multi-tenancy** | Per-tenant QPS limits, burst control, timeouts                                                     |
| **Observability** | Prometheus metrics, OTLP tracing, JSON Lines audit log, WAL crash-safe persistence                 |
| **i18n**          | English, Simplified Chinese, Traditional Chinese                                                   |

---

## Milestone 1: First Decision <Badge type="tip" text="v0.5" />

**Goal**: A new user goes from sign-up to executing their first rule in under 5 minutes.

### Rule Templates

Pre-built industry templates — each includes a complete RuleSet, pre-defined Facts & Concepts, sample input data, and a side-by-side "from if/else to Ordo" migration guide.

| Template           | Scenario                                   | Showcases                       |
| ------------------ | ------------------------------------------ | ------------------------------- |
| E-commerce Pricing | Discount tiers + VIP levels + time windows | Decision tables, hit policies   |
| Loan Approval      | Multi-condition branches + scorecard       | Decision graph, multi-step flow |
| API Routing        | Weighted routing + region + fallback       | Action nodes, score aggregation |
| Permission Check   | RBAC + attribute conditions                | Policy layer, DENY_OVERRIDES    |

### Guided Onboarding

Step-by-step walkthrough for first-time users:

1. Sign up → auto-create a default workspace
2. Pick a template or start blank
3. Interactive editor tour: explore nodes → run first execution → modify a condition
4. "Next: integrate with your service via SDK →"

### Playground Upgrade

No-signup, browser-only experience:

- Pre-loaded template RuleSet
- Live editing + instant execution
- Execution Trace visualization
- Clear CTA to sign up when ready

---

## Milestone 2: Deploy & Connect <Badge type="tip" text="v0.6" />

**Goal**: Edit rules in Studio, publish with one click, and your SDK calls get the update instantly.

### Publish Pipeline

Explicit separation between **draft** (editing in Studio) and **deployment** (live on Engine):

```
Edit in Studio → Click "Publish" → Validation runs
  → Version auto-incremented → Pushed to Engine → SDK calls updated
```

- Diff preview before publishing
- Version history with rollback capability
- "Save" (draft) vs "Publish" (deploy) — clearly separated

### Environment Management

Configure multiple environments per project:

- **Development** — auto-deploy on publish (fast iteration)
- **Staging** — manual push (testing)
- **Production** — requires confirmation (safe releases)

Each environment points to a different Engine instance with health monitoring.

### SDK Documentation

Unified docs site with:

- **30-second integration snippets** for Go, Java, Python
- Quick-start tutorials (from zero to calling your first rule)
- REST and gRPC API reference
- Error handling best practices

---

## Milestone 3: Observe <Badge type="tip" text="v0.7" />

**Goal**: See how your rules perform in production at a glance.

### Execution Dashboard

Real-time monitoring per project:

- **Metrics**: QPS, P50/P99 latency, error rate
- **Trend charts**: 1h / 24h / 7d views
- **Hit distribution**: which terminal results are being reached and how often
- **Recent anomalies**: expression errors, timeouts, unexpected patterns

### Trace Explorer

Search, filter, and visualize execution traces:

- Filter by time range, ruleset, terminal result, duration
- Trace detail view: replay the decision path on the flow graph with highlighted steps
- Input / output comparison panel

### Alerting

Configurable alerts with webhook notification:

| Condition        | Example                              |
| ---------------- | ------------------------------------ |
| Error rate spike | Expression evaluation failures > 1%  |
| Latency anomaly  | P99 > threshold for 5 minutes        |
| Traffic drop     | QPS suddenly falls (upstream issue?) |
| Result shift     | Reject rate jumps from 10% to 40%    |

---

## Milestone 4: Govern <Badge type="tip" text="v0.8" />

**Goal**: Rule changes follow a controlled, auditable process.

### Change Requests

PR-like approval workflow for rule changes:

```
Author submits change (with description)
  → Change Request created (shows diff + impact)
  → Reviewer approves / requests changes / rejects
  → Approved → auto-deploys to target environment
```

### Impact Analysis

Before publishing, automatically answer:

- Which Decision Contracts are affected?
- Which Facts and Concepts does this rule depend on?
- Which downstream consumers will be impacted?
- How would historical inputs produce different results? (replay diff)

### Audit Log

Every significant operation is recorded:

- Rule edits with before/after diff
- Publish events with version and target environment
- Role and permission changes
- Approval decisions with reasoning

---

## Milestone 5: Decision Topology <Badge type="tip" text="v0.9" />

**Goal**: Organization-level visibility into all decision points.

### Decision Service

A new concept layer — the **deployable unit of decision-making capability**:

```
Organization
  └── Project
        └── Decision Service
              ├── RuleSets
              ├── Input / Output Contracts
              ├── Fact & Concept Dependencies
              └── Downstream Consumers
```

### Topology View

Interactive organization-wide graph:

- Nodes = Decision Services, Edges = data/contract dependencies
- Color-coded health status (active / degraded / error)
- Click to drill down into any service
- Search by owner, tag, or status
- "What if" — highlight blast radius when a Fact or Concept changes

---

## Milestone 6: Ordo Cloud <Badge type="tip" text="v1.0" />

**Goal**: Managed platform — sign up and start making decisions, zero infrastructure required.

### What Cloud Adds

| Capability                                    | Self-hosted (OSS)  | Ordo Cloud         |
| --------------------------------------------- | ------------------ | ------------------ |
| Rule editing & publishing                     | :white_check_mark: | :white_check_mark: |
| Self-managed Engine                           | :white_check_mark: | :white_check_mark: |
| **Hosted Engine** (shared or dedicated)       | —                  | :white_check_mark: |
| **Bring your own Engine** (register to Cloud) | —                  | :white_check_mark: |
| **Real-time collaborative editing**           | —                  | :white_check_mark: |
| **SSO / SAML**                                | —                  | :white_check_mark: |
| **Long-term metrics & custom dashboards**     | —                  | :white_check_mark: |
| **Compliance report export**                  | —                  | :white_check_mark: |
| **SLA guarantee + priority support**          | —                  | :white_check_mark: |

---

## Timeline

```
2026 Q2          Q3              Q4           2027 Q1         Q2-Q3
  │               │               │              │              │
  ├── M1 ────────┤               │              │              │
  │  First        ├── M2 ───────┤              │              │
  │  Decision     │  Deploy &    ├── M3 ──────┤              │
  │               │  Connect     │  Observe    ├── M4 ───────┤
  │               │              │             │  Govern      ├── M5+M6
  │               │              │             │              │
```

::: info
Timelines are directional, not commitments. Priorities may shift based on community feedback.
:::

---

## Design Principles

**Each milestone is independently valuable.** You don't need governance (M4) to benefit from deployment (M2).

**Progressive adoption.** Start with one ruleset replacing your most painful if/else. Add governance, monitoring, and topology when your organization is ready.

**Open by default.** Milestones 1–5 are fully open source under the MIT license. Ordo Cloud adds managed hosting and enterprise features on top.

---

## Get Involved

We'd love your input on what to prioritize:

- **Feature requests & feedback**: [GitHub Issues](https://github.com/Pama-Lee/Ordo/issues)
- **Community**: [Discord](https://discord.gg/Y529FkArhh)
- **Contribute**: Check out our [Contributing Guide](https://github.com/Pama-Lee/Ordo/blob/main/CONTRIBUTING.md)

Share your use case — hearing how you're thinking about using Ordo directly shapes what we build next.
