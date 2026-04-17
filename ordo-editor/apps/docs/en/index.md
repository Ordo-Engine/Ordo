---
layout: home

hero:
  name: 'Ordo'
  text: 'Open-Source Decision Platform'
  tagline: Author, test, and govern business rules — with Studio, platform governance, and a fast engine under the hood.
  image:
    src: /logo.png
    alt: Ordo
  actions:
    - theme: brand
      text: Get Started
      link: /en/guide/getting-started
    - theme: alt
      text: Try Playground
      link: https://pama-lee.github.io/Ordo/
    - theme: alt
      text: View on GitHub
      link: https://github.com/Pama-Lee/Ordo

features:
  - icon: 🏛️
    title: Decision Platform
    details: Org & project management, fact catalog, typed decision contracts, and full version history. Own your decision logic — don't scatter it across codebases and spreadsheets.
  - icon: 🎨
    title: Studio
    details: Drag-and-drop flow editor, decision tables, one-click template instantiation, and test case management. Author rules without friction.
  - icon: 🧪
    title: Test Management
    details: Create, run, and export test suites per ruleset. CI-compatible YAML. Know your rules work before they ship.
  - icon: ⚡
    title: Fast Engine
    details: Sub-microsecond execution with Cranelift JIT. Runs as HTTP · gRPC · WASM · CLI or embedded in any Rust application.
  - icon: 🛡️
    title: Governance
    details: Typed input/output contracts, audit logging, Ed25519 rule signing, and rollback. Traceable and compliant by default.
  - icon: 🔌
    title: Runs Everywhere
    details: Single binary server, browser via WebAssembly, embedded in Rust apps. One engine across every deployment target.
---

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

