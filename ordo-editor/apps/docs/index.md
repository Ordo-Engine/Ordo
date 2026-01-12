---
layout: home

hero:
  name: "Ordo"
  text: "High-Performance Rule Engine"
  tagline: Sub-microsecond latency, 500K+ QPS, with visual editor
  image:
    src: /logo.png
    alt: Ordo
  actions:
    - theme: brand
      text: Get Started
      link: /guide/getting-started
    - theme: alt
      text: Try Playground
      link: https://pama-lee.github.io/Ordo/
    - theme: alt
      text: View on GitHub
      link: https://github.com/Pama-Lee/Ordo

features:
  - icon: ⚡
    title: Blazing Fast
    details: 1.63µs average execution time. 600x faster than 1ms target. Zero-allocation hot path.
  - icon: 🎨
    title: Visual Editor
    details: Design complex rules with drag-and-drop flow editor. Real-time execution with WASM.
  - icon: 🔧
    title: Flexible Rules
    details: Step flow model, rich expressions, built-in functions, and field coalescing.
  - icon: 🛡️
    title: Production Ready
    details: Deterministic execution, full tracing, hot reload, and audit logging.
  - icon: 🔌
    title: Easy Integration
    details: HTTP REST API, gRPC support, and WebAssembly for browser execution.
  - icon: 📊
    title: Observable
    details: Prometheus metrics, health checks, and structured audit logs.
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
      "branches": [
        { "condition": "user.vip == true", "next_step": "vip_discount" }
      ],
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

## Performance

| Metric | Result |
|--------|--------|
| Single rule execution | **1.63 µs** |
| Expression evaluation | **79-211 ns** |
| HTTP API throughput | **54,000 QPS** |
| Projected multi-thread | **500,000+ QPS** |
