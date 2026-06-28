---
layout: home

hero:
  name: 'Ordo'
  text: '开源决策平台'
  tagline: 由治理平台与高性能引擎组成的一体化决策基础设施。Studio 编排、平台审计、引擎执行——三层职责清晰分离。
  image:
    src: /logo.png
    alt: Ordo
  actions:
    - theme: brand
      text: 开始使用
      link: /zh/guide/getting-started
    - theme: alt
      text: 平台篇
      link: /zh/platform/overview
    - theme: alt
      text: 引擎篇
      link: /zh/guide/what-is-ordo
    - theme: alt
      text: GitHub
      link: https://github.com/Ordo-Engine/Ordo

features:
  - title: 决策平台
    details: 组织 / 项目 / 成员与角色（RBAC）、事实目录、概念注册、决策契约、审批与发布流水线、多环境与回滚——为团队级决策治理而生。
    link: /zh/platform/overview
    linkText: 查看平台文档
  - title: Studio 编辑器
    details: 三种编辑模式（流程图 / 表单 / JSON）、决策表、子规则、模板实例化、测试套件管理与执行追踪面板。
    linkText: 查看 Studio
    link: /zh/platform/studio
  - title: 发布与环境治理
    details: 草稿 → 审批 → 发布 → 灰度 → 回滚。可配置的审批策略、变更对比、按环境分别下发，所有动作进入审计日志。
    link: /zh/platform/releases
    linkText: 发布流程
  - title: 高性能引擎
    details: 亚微秒级规则执行，字节码 VM + Cranelift JIT、表达式优化器。HTTP / gRPC / Unix Socket / WASM 多协议接入。
    link: /zh/guide/execution-model
    linkText: 执行模型
  - title: 类型与契约
    details: 项目级事实目录、可复用概念、带类型的输入/输出契约。Studio 与 CLI 共用同一份契约定义。
    link: /zh/platform/catalog
    linkText: 事实与契约
  - title: 多区域部署
    details: 平台中央治理 + 区域化引擎集群。服务器注册、健康检查、按项目路由的执行代理，支持单 binary 与容器化部署。
    link: /zh/platform/server-registry
    linkText: 服务器注册
---

## 架构概览

```mermaid
flowchart TB
  Studio["Studio (浏览器)"]
  CLI["ordo-cli"]
  SDK["SDK / 业务系统"]
  Platform["ordo-platform<br/>治理 · 草稿 · 审批 · 发布"]
  Server["ordo-server 集群<br/>HTTP · gRPC · UDS"]
  Core["ordo-core 引擎<br/>VM + JIT + 子规则 + 追踪"]

  Studio --> Platform
  CLI --> Platform
  SDK --> Server
  Platform -- "发布事件 (NATS / 直接同步)" --> Server
  Server --> Core
```

Ordo 的文档分为两大部分：

- **平台篇**——面向使用 Ordo Platform / Studio 治理决策的团队：组织建模、契约、发布流程、测试管理。
- **引擎篇**——面向需要直接集成 ordo-core / ordo-server 的开发者：规则结构、表达式语法、HTTP / gRPC / WASM API。

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
