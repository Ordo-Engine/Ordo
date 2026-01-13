---
layout: home

hero:
  name: 'Ordo'
  text: '高性能规则引擎'
  tagline: 亚微秒级延迟，50万+ QPS，配备可视化编辑器
  image:
    src: /logo.png
    alt: Ordo
  actions:
    - theme: brand
      text: 开始使用
      link: /zh/guide/getting-started
    - theme: alt
      text: 尝试演练场
      link: https://pama-lee.github.io/Ordo/
    - theme: alt
      text: GitHub
      link: https://github.com/Pama-Lee/Ordo

features:
  - icon: ⚡
    title: 极速性能
    details: 1.63µs 平均执行时间。比 1ms 目标快 600 倍。零分配热路径。
  - icon: 🎨
    title: 可视化编辑器
    details: 使用拖放流程编辑器设计复杂规则。通过 WASM 实时执行。
  - icon: 🔧
    title: 灵活规则
    details: 步骤流模型，丰富的表达式，内置函数和字段合并。
  - icon: 🛡️
    title: 生产就绪
    details: 确定性执行，完整追踪，热重载和审计日志。
  - icon: 🔌
    title: 易于集成
    details: HTTP REST API，gRPC 支持，以及用于浏览器执行的 WebAssembly。
  - icon: 📊
    title: 可观测性
    details: Prometheus 指标，健康检查和结构化审计日志。
---

## 快速示例

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

## 性能

| 指标            | 结果             |
| --------------- | ---------------- |
| 单规则执行      | **1.63 µs**      |
| 表达式评估      | **79-211 ns**    |
| HTTP API 吞吐量 | **54,000 QPS**   |
| 预计多线程      | **500,000+ QPS** |
