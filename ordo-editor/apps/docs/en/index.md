---
layout: home

hero:
  name: 'Ordo'
  text: 'Open-Source Decision Platform'
  tagline: A unified decision infrastructure — Studio for authoring, Platform for governance, Engine for execution. Three layers, clean separation of concerns.
  image:
    src: /logo.png
    alt: Ordo
  actions:
    - theme: brand
      text: Get Started
      link: /en/guide/getting-started
    - theme: alt
      text: Platform Docs
      link: /en/platform/overview
    - theme: alt
      text: Engine Docs
      link: /en/guide/what-is-ordo
    - theme: alt
      text: GitHub
      link: https://github.com/Ordo-Engine/Ordo

features:
  - title: Decision Platform
    details: Organizations, projects, members & RBAC, fact catalog, concept registry, typed contracts, approval & release pipelines, multi-environment rollouts and rollback — built for team-scale decision governance.
    link: /en/platform/overview
    linkText: Platform overview
  - title: Studio Editor
    details: Three authoring modes (flow / form / JSON), decision tables, sub-rules, template instantiation, test suite management, and execution trace panels.
    link: /en/platform/studio
    linkText: Studio guide
  - title: Releases & Environments
    details: Draft → review → release → canary → rollback. Configurable approval policies, change diffs, per-environment delivery, every action recorded in the audit log.
    link: /en/platform/releases
    linkText: Release pipeline
  - title: High-Performance Engine
    details: Sub-microsecond rule execution. Bytecode VM plus Cranelift JIT, expression optimizer. Reach it over HTTP, gRPC, Unix Socket, or WASM.
    link: /en/guide/execution-model
    linkText: Execution model
  - title: Types & Contracts
    details: Project-scoped fact catalog, reusable concepts, typed input/output contracts. Studio and CLI consume the same contract definitions.
    link: /en/platform/catalog
    linkText: Facts & contracts
  - title: Multi-Region Deployment
    details: Central platform governance plus regional engine clusters. Server registry, health checks, per-project execution proxy. Single-binary or containerized deployment.
    link: /en/platform/server-registry
    linkText: Server registry
---

## Architecture

```mermaid
flowchart TB
  Studio["Studio (browser)"]
  CLI["ordo-cli"]
  SDK["SDK / business app"]
  Platform["ordo-platform<br/>governance · drafts · review · release"]
  Server["ordo-server cluster<br/>HTTP · gRPC · UDS"]
  Core["ordo-core engine<br/>VM + JIT + sub-rules + trace"]

  Studio --> Platform
  CLI --> Platform
  SDK --> Server
  Platform -- "release events (NATS / direct sync)" --> Server
  Server --> Core
```

The documentation is organized into two tracks:

- **Platform** — for teams using Ordo Platform / Studio to govern decisions: organization modeling, contracts, release flow, test management.
- **Engine** — for developers integrating ordo-core / ordo-server directly: rule structure, expression syntax, HTTP / gRPC / WASM APIs.

## Quick Example

```json
{
  "config": {
    "name": "discount-check",
    "version": "1.0.0",
    "entry_step": "check_vip"
  },
  "steps": {
    "check_vip": {
      "id": "check_vip",
      "name": "Check VIP Status",
      "type": "decision",
      "branches": [{ "condition": "user.vip == true", "next_step": "vip_discount" }],
      "default_next": "normal_discount"
    },
    "vip_discount": {
      "id": "vip_discount",
      "type": "terminal",
      "result": { "code": "VIP", "message": "20% discount" }
    },
    "normal_discount": {
      "id": "normal_discount",
      "type": "terminal",
      "result": { "code": "NORMAL", "message": "5% discount" }
    }
  }
}
```
