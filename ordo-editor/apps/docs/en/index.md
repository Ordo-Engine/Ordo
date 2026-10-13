---
layout: home

hero:
  name: 'Ordo'
  text: 'Turn judgments into decisions'
  tagline: An open-source decision engine. Models and scores tell you what is going on. Ordo decides what to do about it, with rules that have tests and versions, and shows which decisions a change moves before you ship it.
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
  - title: Models judge, rules decide
    details: Decision models like Jev read a customer's message and give you a judgment with a confidence. Whether to refund, and whether a person should look first, also depends on the amount, the window and your policy. That part lives in rules.
    link: https://github.com/Ordo-Engine/Ordo/tree/main/examples/rule-packs/jev-routing
    linkText: Refund routing pack
  - title: See the impact first
    details: Before you move a threshold, ordo impact runs the edit next to the last commit, over the tests, real requests and probe inputs on both sides of every threshold, and lists every decision that changed.
    link: /en/platform/cli
    linkText: CLI
  - title: Rules are files
    details: Rules are JSON or YAML, in git next to your code, with their own tests. The expression language is bounded, with no loops and no side effects, so rules written by people and by AI can both be checked. A rule runs in microseconds, over HTTP, gRPC or WASM.
    link: /en/guide/rule-structure
    linkText: Rule structure
---

## Every app that uses a decision model has a 0.85 in it

```python
if jev.confidence > 0.85:
    refund(order)
```

That line decides who gets money back. Nobody reviews it, no test covers it, and when the model is updated, 0.85 starts to mean something else.

Jev can tell that a customer wants a refund. It doesn't know your refund window, your amount limits, or how sure is sure enough to pay out without a person looking. That part is a decision, and it belongs in Ordo.

> Jev turns text into judgments. Ordo turns judgments into decisions.

## What a rule looks like

What to do with a refund request, written as a decision table:

```yaml
config:
  name: refund
  version: 1.0.0
  entry_step: route
steps:
  route:
    id: route
    name: Route the refund request
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
    name: Done
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

Jev is 0.72 sure the customer wants a refund. The rules say that is not enough to move money on its own, so an LLM drafts the reply and an agent approves it. To change the threshold, edit the table and run the tests again. The calling code stays the same.

## See what a change affects

Someone proposes auto refunding from 0.80 instead of 0.85. In the [jev-routing rule pack](https://github.com/Ordo-Engine/Ordo/tree/main/examples/rule-packs/jev-routing), all 12 tests still pass after the edit. `ordo impact` runs it next to the last commit:

```text
$ ordo impact refund-routing
impact refund-routing  (HEAD → working tree)
  309 inputs: 12 tests, 0 captured, 297 boundary probes

  LLM_DRAFT → AUTO_REFUND  18 inputs
  LLM_DRAFT → HUMAN_REVIEW  3 inputs
…
21 changed · 288 unchanged
```

The tests did not notice, but 18 requests that used to wait for a person now get their money back without anyone reading the message. Add `--inputs` with real requests and the count is real customers, before the change ships.

## Where it fits

- After a decision model: models like Jev read messages, tickets or reviews, and rules decide whether to refund, hand off to a person, or act right away.
- Pricing, promotions, loyalty points: rules that change often and must be exact.
- Risk, eligibility, approvals: every decision needs to be explainable and audited.
- Routing and assignment: orders, tickets, payment channels.
- Limits for AI agents: [Ordo Guard](/en/platform/guard) uses the same engine to allow, deny or ask before Claude Code, Codex CLI or Cursor runs a command.

## Where to start

- New to Ordo: [Quick Start](/en/guide/quick-start) gets a first rule running in five minutes.
- The concepts: [What is Ordo?](/en/guide/what-is-ordo)
- Team workflows and visual editing: [Studio & Platform](/en/platform/overview)
