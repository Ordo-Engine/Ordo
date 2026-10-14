---
layout: home

hero:
  name: 'Ordo'
  text: '把判断变成决策'
  tagline: 开源决策引擎。模型和评分告诉你发生了什么，Ordo 决定接下来怎么做。规则带测试、带版本，改之前先看会影响哪些决策。
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
  - title: 模型回答之后
    details: Jev 这类决策模型能读懂客户的一段话，给出判断和置信度。退不退款、要不要人工看，还要看金额、期限和你们的政策，这一步写在规则里。
    link: https://github.com/Ordo-Engine/Ordo/tree/main/examples/rule-packs/jev-routing
    linkText: 退款分流规则包
  - title: 改之前先看影响
    details: 调一个阈值之前，ordo impact 把改动和上一个提交放在一起跑，输入包括测试、真实请求和每个阈值两侧的探测值，列出每一个变了的决策。
    link: /zh/platform/cli
    linkText: 命令行
  - title: 规则是文件
    details: 规则是 JSON 或 YAML，和代码一起进 git，带自己的测试。表达式语言有边界，没有循环，也不能改外部状态，人写的和 AI 写的规则都能被校验。单次执行在微秒级，HTTP、gRPC、WASM 都能跑。
    link: /zh/guide/rule-structure
    linkText: 规则结构
---

## 代码里的一个 0.85

```python
if jev.confidence > 0.85:
    refund(order)
```

这一行决定谁能拿回钱。它通常没经过 review，也没有测试，模型一升级，0.85 的含义就变了。

Jev 能读懂客户想退款，但它不知道你们的退款期限、金额上限，也不知道多大把握才敢不经人工直接退钱。这一步是决策，交给 Ordo。

## 一条规则长什么样

退款请求怎么处理，写成一张决策表：

```yaml
config:
  name: refund
  version: 1.0.0
  entry_step: route
steps:
  route:
    id: route
    name: 退款怎么处理
    type: decision_table
    inputs: [jev.intent.choice, jev.intent.confidence, order.amount, order.days_since_delivery]
    outputs: [action]
    rules:
      - when: ["*", "*", "*", "> 30"]
        then: [HUMAN_REVIEW]
      - when: [refund, ">= 0.85", "<= 200", "*"]
        then: [AUTO_REFUND]
      - when: [refund, ">= 0.6", "*", "*"]
        then: [LLM_DRAFT]
    default: [HUMAN_REVIEW]
    next_step: done
  done:
    id: done
    name: 完成
    type: terminal
    result:
      code: OK
      output:
        - [action, $action]
```

```bash
$ ordo exec --rule refund.yaml --input '{"jev":{"intent":{"choice":"refund","confidence":0.72}},
    "order":{"amount":129,"days_since_delivery":1}}'
code:    OK
output:  {
  "action": "LLM_DRAFT"
}
```

Jev 有 0.72 的把握客户想退款，规则认为这还不够直接动钱，所以让 LLM 起草回复，再由客服确认。改门槛只需要改这张表，再跑一遍测试，调用方的代码不用动。

## 改之前先看影响

有人提议把自动退款的门槛从 0.85 降到 0.80。在 [jev-routing 规则包](https://github.com/Ordo-Engine/Ordo/tree/main/examples/rule-packs/jev-routing)里改完，12 个测试全部通过。`ordo impact` 把改动和上一个提交放在一起跑：

```text
$ ordo impact refund-routing
impact refund-routing  (HEAD → working tree)
  309 inputs: 12 tests, 0 captured, 297 boundary probes

  LLM_DRAFT → AUTO_REFUND  18 inputs
  LLM_DRAFT → HUMAN_REVIEW  3 inputs
…
21 changed · 288 unchanged
```

测试都过了，但有 18 个原本要人确认的请求，现在没人看就直接退钱了。加上 `--inputs` 用真实请求跑一遍，看到的就是真实客户的数量，而且是在改动上线之前。

## 用在哪里

- 决策模型之后：Jev 这类模型读懂客户消息、工单或评论，规则决定退款、转人工还是直接处理。
- 定价、优惠、积分：规则常改，又必须算对。
- 风控、准入、审批：每次判断都要能解释、能审计。
- 路由与分配：订单、工单、支付通道的分派逻辑。
- AI Agent 的权限边界：[Ordo Guard](/zh/platform/guard) 用同一个引擎，在 Claude Code、Codex CLI、Cursor 执行命令前做放行、拒绝或询问的判断。

## 从哪里开始

- 第一次用：[快速上手](/zh/guide/quick-start)，五分钟写出并运行第一条规则。
- 想了解概念：[Ordo 是什么](/zh/guide/what-is-ordo)。
- 团队协作和可视化编辑：[Studio 与平台](/zh/platform/overview)。
