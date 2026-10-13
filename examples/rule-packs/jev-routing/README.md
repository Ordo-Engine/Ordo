# Jev routing

[Jev](https://openrouter.ai/blog/insights/what-is-jev/) is a decision model
from TypeSafe. You give it a customer message and a few typed questions, and it
answers with one of your options plus a probability. It reads the message well.
It does not know your refund policy, your amount limits, or how sure is sure
enough to pay out without a person looking.

That part is rules. This pack keeps it in Ordo: Jev reads the message, Ordo
decides what happens next, and every threshold sits in one file you can test,
review and diff.

```
customer message ──► Jev ──► { intent: refund 1, urgency 0.2, chargeback 0.02 }
                                   │
order + customer facts ────────────┤
                                   ▼
                         Ordo refund-routing ──► AUTO_REFUND / LLM_DRAFT / HUMAN_REVIEW / ESCALATE / ROUTE_TEAM
```

## The Jev questions

[`jev-questions.json`](./jev-questions.json) asks three things about each
message:

| question     | Jev type | answer used by the rules |
|--------------|----------|--------------------------|
| `intent`     | choice   | `choice` (refund, delivery, product_question, account, other) and `confidence` |
| `urgency`    | score    | `score`, 0 (routine) to 2 (deadline or stuck) |
| `chargeback` | noul     | `noul`, the probability the customer threatens a chargeback or legal action |

## Input to Ordo

```json
{
  "jev": {
    "intent": { "choice": "refund", "confidence": 1 },
    "urgency": { "score": 0.2 },
    "chargeback": { "noul": 0.02 }
  },
  "order": { "amount": "39.90", "days_since_delivery": 2 },
  "customer": { "refunds_90d": 0 }
}
```

The Jev part is checked against the input schema like any other field, so a
missing or malformed answer is rejected before a rule runs. `amount` is
`decimal`.

## How it decides

1. **Chargeback or legal threat.** `chargeback >= 0.5` goes straight to a
   senior agent, whatever else is true.
2. **Not a refund.** If Jev is at least 0.6 sure the message is about
   something else, it goes to that team.
3. **Priority.** Urgency 1.5 or more marks the case `high`.
4. **Route**, a decision table where the first matching row wins:

   | intent | confidence | amount | delivered   | refunds in 90 days | decision      | reason          |
   |--------|------------|--------|-------------|--------------------|---------------|-----------------|
   | any    | any        | any    | any         | 3 or more          | HUMAN_REVIEW  | repeat_refunds  |
   | any    | any        | any    | > 30 days   | any                | HUMAN_REVIEW  | outside_window  |
   | refund | >= 0.85    | <= 200 | any         | any                | AUTO_REFUND   |                 |
   | refund | >= 0.85    | > 200  | any         | any                | HUMAN_REVIEW  | amount          |
   | refund | >= 0.6     | any    | any         | any                | LLM_DRAFT     | medium_confidence |
   | otherwise |         |        |             |                    | HUMAN_REVIEW  | low_confidence  |

`LLM_DRAFT` means an LLM writes the reply and an agent approves it: Jev was
fairly sure, but not sure enough to move money alone. Every output carries the
Jev probability the decision was based on, so the decision log shows why.

## Run it

```bash
cd examples/rule-packs/jev-routing
ordo test
node route.mjs samples/double-charge.json
```

`route.mjs` reads a sample, asks Jev, builds the Ordo input and runs
`ordo trace`. Without `OPENROUTER_API_KEY` it uses the `jev_response` stored in
the sample, so it runs offline. The stored responses are real Jev answers,
recorded on 2026-10-13 from `typesafe/jev-1.13` through OpenRouter.

```
$ node route.mjs samples/not-what-i-expected.json
message: The blender arrived yesterday but it is much louder than the video showed. ...
jev (recorded): intent=refund 0.97, urgency=0.43, chargeback=0.04

code:    AUTO_REFUND
message: Refund issued without review
```

The customer only asked what their options are. Jev was 0.97 sure this is a
refund, so the rules paid out 129 EUR without anyone reading the message. That
is not a Jev bug: TypeSafe says its confidence is not a probability and there is
no universal threshold. In these samples Jev answers 0.97 or 1 almost every
time, so the 0.85 line separates very little. Which is why the threshold
belongs in a rule you can test and replay, not in an `if` in your code.

With a key, the message goes to Jev through OpenRouter:

```bash
OPENROUTER_API_KEY=sk-or-... node route.mjs samples/not-what-i-expected.json
```

The four samples: a double charge (auto refund), an unsure "what are my
options" (also auto refund, see above), a bank dispute threat (escalate) and a
delivery question (another team).

## Run it behind Ordo

`route.mjs` calls Jev and Ordo one after the other. In production, ordo-server
can sit in front of Jev and do both in one call, and log every decision:

```bash
export ORDO_DECIDE_API_KEY=sk-or-...
ordo-server --rules-dir rulesets \
  --decide-upstream https://openrouter.ai/api \
  --decide-log-dir decisions
```

The application keeps sending its normal Jev request to
`http://localhost:8080/systemone/v1/systemone` (for the TypeSafe SDKs, set
`TYPESAFE_BASE_URL=http://localhost:8080/systemone`) and adds the order facts:

```json
{
  "model": "typesafe/jev-1.13",
  "state": "Hi, I was charged twice for order #4471 ...",
  "questions": { "...": "contents of jev-questions.json" },
  "ordo": {
    "ruleset": "refund-routing",
    "facts": {
      "order": { "amount": "39.90", "days_since_delivery": 2 },
      "customer": { "refunds_90d": 0 }
    }
  }
}
```

The response is Jev's answer with the decision added:

```json
"ordo": { "code": "AUTO_REFUND", "message": "Refund issued without review", "output": { "jev_confidence": 1, "priority": "normal", ... }, ... }
```

Each call adds a line to `decisions/decisions-YYYY-MM-DD.jsonl`, and those
files are what `ordo impact --inputs` reads in the next section. See the
[decision model gateway docs](https://docs.ordoengine.com/en/platform/decision-gateway)
for the details.

## Change a threshold, see what moves

Support is swamped and someone proposes auto refunding from 0.80 instead of
0.85. Change both `">= 0.85"` cells in the route table to `">= 0.8"` and run
`ordo test`: all 12 tests still pass. Then:

```
$ ordo impact refund-routing
impact refund-routing  (HEAD → working tree)
  309 inputs: 12 tests, 0 captured, 297 boundary probes

  LLM_DRAFT → AUTO_REFUND  18 inputs
  LLM_DRAFT → HUMAN_REVIEW  3 inputs

CHANGED probe:jev.intent.confidence=0.84  {... "order":{"amount":"129.00","days_since_delivery":1}}
    code: LLM_DRAFT → AUTO_REFUND
    message: "An LLM drafts the reply, an agent approves it" → "Refund issued without review"
...
21 changed · 288 unchanged
```

Messages Jev rates between 0.80 and 0.84 used to get a drafted reply that a
person approved. Now they are paid without anyone reading them. Probabilities
also move a little between Jev calls and between model versions, so a
threshold right at the edge is worth replaying. With the gateway's decision
log, run `ordo impact refund-routing --inputs decisions/decisions-2026-10-13.jsonl`
to count how many real refunds the change would send out unreviewed. Any JSON
Lines file of Ordo inputs (Jev answers plus order facts) works the same way.

The thresholds here are illustrative, not advice for any real refund policy.
