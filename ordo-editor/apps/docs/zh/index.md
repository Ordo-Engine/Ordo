---
layout: home

hero:
  name: 'Ordo'
  text: '把业务规则从代码里拿出来'
  tagline: 用 JSON 或 YAML 写规则，配上测试和版本，改规则不用重新发版。Rust 引擎，单次执行在微秒级。人能写，AI 也能写。
  image:
    src: /logo.png
    alt: Ordo
  actions:
    - theme: brand
      text: 快速上手
      link: /zh/guide/quick-start
    - theme: alt
      text: 在线试用
      link: https://ordo-engine.github.io/Ordo/
    - theme: alt
      text: GitHub
      link: https://github.com/Ordo-Engine/Ordo

features:
  - title: 规则是文件
    details: 判断分支、决策表、子规则都写在 JSON 或 YAML 里，和代码一起进 git，可以 review，可以 diff。表达式语言是受限的，没有循环，也不能改外部状态，AI 生成的规则也能被校验。
    link: /zh/guide/rule-structure
    linkText: 规则结构
  - title: 先测试，再上线
    details: 每个规则集带自己的测试用例，ordo test 在本地和 CI 里都能跑。执行过程可以逐步追踪，看清每个结果是怎么得出来的。
    link: /zh/platform/testing
    linkText: 测试规则
  - title: 哪里都能跑
    details: 字节码虚拟机加 Cranelift JIT。可以通过 HTTP、gRPC、Unix Socket 调用，也能编译成 WASM 在浏览器里跑，或者直接嵌进 Rust 程序。
    link: /zh/guide/execution-model
    linkText: 执行模型
---

## 一条规则长什么样

按会员等级和订单金额定折扣，写成一张决策表：

```yaml
config:
  name: discount
  version: 1.0.0
  entry_step: pick_rate
steps:
  pick_rate:
    id: pick_rate
    name: 选折扣率
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
    name: 完成
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

改折扣只需要改这张表，再跑一遍测试。调用方的代码不用动。

## 用在哪里

- 定价、优惠、积分：规则常改，又必须算对。
- 风控、准入、审批：每次判断都要能解释、能审计。
- 路由与分配：订单、工单、支付通道的分派逻辑。
- AI Agent 的权限边界：[Ordo Guard](/zh/platform/guard) 用同一个引擎，在 Claude Code、Codex CLI、Cursor 执行命令前做放行、拒绝或询问的判断。

## 从哪里开始

- 第一次用：[快速上手](/zh/guide/quick-start)，五分钟写出并运行第一条规则。
- 想了解概念：[Ordo 是什么](/zh/guide/what-is-ordo)。
- 团队协作和可视化编辑：[Studio 与平台](/zh/platform/overview)。
