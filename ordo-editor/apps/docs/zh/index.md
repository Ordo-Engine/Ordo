---
layout: home

hero:
  name: 'Ordo'
  text: '开源决策平台'
  tagline: 编写、测试、治理业务规则 — Studio 可视化编辑、平台级治理，底层引擎快到感觉不到。
  image:
    src: /logo.png
    alt: Ordo
  actions:
    - theme: brand
      text: 开始使用
      link: /zh/guide/getting-started
    - theme: alt
      text: 尝试演练场
      link: https://ordo-engine.github.io/Ordo/
    - theme: alt
      text: GitHub
      link: https://github.com/Ordo-Engine/Ordo

features:
  - icon: 🏛️
    title: 决策平台
    details: 组织与项目管理、事实目录、带类型的决策契约，以及完整的版本历史。让团队真正拥有自己的决策逻辑，而不是散落在代码库和电子表格里。
  - icon: 🎨
    title: Studio
    details: 拖拽式流程编辑器、决策表、一键实例化模板，以及测试用例管理。低摩擦地编写规则。
  - icon: 🧪
    title: 测试管理
    details: 为每个规则集创建、运行、导出测试套件。兼容 ordo-cli 的 YAML 格式，直接接入 CI/CD。上线前确保规则正确。
  - icon: ⚡
    title: 高性能引擎
    details: 亚微秒级执行，Cranelift JIT 编译。支持 HTTP · gRPC · WASM · CLI，或嵌入任意 Rust 应用。
  - icon: 🛡️
    title: 治理
    details: 带类型的输入/输出契约、审计日志、Ed25519 规则签名与一键回滚。默认可追溯、合规。
  - icon: 🔌
    title: 随处运行
    details: 单二进制服务器、浏览器端 WebAssembly、嵌入式 Rust 集成。同一个引擎覆盖所有部署场景。
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
