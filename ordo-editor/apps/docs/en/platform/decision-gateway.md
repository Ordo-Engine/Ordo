# Decision Model Gateway

Decision models like Jev judge. Ordo decides what happens next. The gateway joins the two steps: your application keeps calling Jev, but the request goes through Ordo first. Ordo forwards it to the model, can run a ruleset on the model's answers on the way back, and writes every call to a decision log. When you later want to move a threshold, you can run `ordo impact` over those real requests.

The gateway speaks the System One API (`/v1/systemone`). TypeSafe's own API, OpenRouter and other compatible services can all be the upstream.

## Turn it on

```bash
export ORDO_DECIDE_API_KEY=...   # the upstream API key, kept on the server

ordo-server \
  --rules-dir ./rulesets \
  --decide-upstream https://api.typesafe.ai \
  --decide-log-dir /var/ordo/decisions
```

With Docker, the rule packs are already in the image:

```bash
mkdir -p decisions && chmod 777 decisions   # the container user writes the log here
docker run -p 8080:8080 -e ORDO_DECIDE_API_KEY=... \
  -v "$PWD/decisions:/decisions" \
  ghcr.io/ordo-engine/ordo:latest \
  --rules-dir /app/rule-packs/jev-routing/rulesets \
  --decide-upstream https://openrouter.ai/api \
  --decide-log-dir /decisions --decide-log-requests
```

The log lands in the mounted `decisions` folder on your machine.

For OpenRouter, use `https://openrouter.ai/api` as the upstream and model ids such as `typesafe/jev-1.13`. Without `ORDO_DECIDE_API_KEY`, Ordo forwards the caller's `Authorization` header to the upstream unchanged.

In the application, change only the base URL:

```bash
export TYPESAFE_BASE_URL=http://ordo:8080/systemone
```

The SDK then calls `http://ordo:8080/systemone/v1/systemone`. Ordo passes the request to the upstream and returns the answer as it came, including errors such as 401, 422 and 429 (with `retry-after`), so the error handling you already have keeps working.

The rules directory can point straight at a rule pack's `rulesets/`. ordo-server reads the rule files that Studio, the CLI and the rule packs write.

## Let the rules decide

Add an `ordo` field to the request with the ruleset and the business facts:

```json
{
  "model": "jev-1.13",
  "state": "Hi, I was charged twice for order #4471 (EUR 39.90 each). Could you refund one of them?",
  "questions": { "intent": { "type": "choice", "...": "..." } },
  "ordo": {
    "ruleset": "refund-routing",
    "facts": {
      "order": { "amount": "39.90", "days_since_delivery": 2 },
      "customer": { "refunds_90d": 0 }
    }
  }
}
```

The `ordo` field is not sent to the model. Once the model has answered, Ordo puts the answers under `jev`, combines them with `facts` into the rule input, runs the ruleset, and adds the decision to the response:

```json
{
  "model": "jev-1.13.0",
  "answers": {
    "intent": { "choice": "refund", "confidence": 0.95, "probabilities": { "...": "..." } },
    "urgency": { "score": 0.7, "...": "..." },
    "chargeback": { "noul": 0.04 }
  },
  "ordo": {
    "decision_id": "dec_202610130530569747fdf5a",
    "ruleset": "refund-routing",
    "version": "1.0.0",
    "code": "AUTO_REFUND",
    "message": "Refund issued without review",
    "output": { "amount": 39.9, "jev_confidence": 0.95, "priority": "normal" },
    "duration_us": 55
  }
}
```

A few details:

- The answers go under `jev` in the rule input by default. Set `ordo.answers_field` to use another name.
- If the ruleset does not exist, Ordo returns 404 without calling the model.
- If the rules fail (a required field is missing, for example), the response is still 200 with the model's answers, and the error is in `ordo.error`. The model has already answered, and that call has already been paid for.
- With multi-tenancy, pick the tenant with `X-Tenant-ID`, as on every other endpoint.

## Decision log

With `--decide-log-dir`, each call adds one line to `decisions-YYYY-MM-DD.jsonl` (rotated daily):

```json
{"decision_id":"dec_…","ts":"…","tenant":"default","model":"jev-1.13.0","latency_ms":171,"upstream_status":200,"answers":{…},"ruleset":"refund-routing","answers_field":"jev","rule_version":"1.0.0","input":{…},"code":"AUTO_REFUND","output":{…},"duration_us":55}
```

Calls without an `ordo` field are logged too, just without a rule input or a decision. When the upstream returns an error, the line has its `upstream_status`.

The request itself (the customer's text in `state`, the questions, the model) is logged only with `--decide-log-requests`. You need it to ask another model the same questions later (see below). It holds customer text, so treat the log as sensitive data.

## See what was decided

```text
$ ordo decisions decisions/
decisions 24 calls in 1 file
  model typesafe/jev-1.13-20260917: 24

  refund-routing
    AUTO_REFUND         10   42%
    HUMAN_REVIEW         6   25%
    ROUTE_TEAM           6   25%
    ESCALATE             2    8%

  latest:
  2026-10-13T12:11:18 AUTO_REFUND  chargeback=0.03, intent=refund 1, urgency=0.09
      "Gift for my mum, she doesn't like it. Can she return it without a receipt?"
  …
```

`ordo decisions` reads one log file or the whole log directory. `--code AUTO_REFUND` shows only the refunds paid without review, `--last 50` lists more, and `--json` gives the same summary to other tools.

## Check a threshold change against real requests

```bash
ordo impact refund-routing --inputs decisions-2026-10-13.jsonl
```

`impact` reads the rule input on each line and skips lines for other rulesets and calls where no ruleset ran. So besides the test cases and boundary probes, you see how many real refund requests a change would send down a different path.

## Check a model switch against real requests

A new Jev version, or another model that speaks `/v1/systemone`, answers the same message with different numbers. Before you switch, ask the new model the logged requests again and see which decisions move:

```text
$ ordo impact refund-routing --inputs decisions/decisions-2026-10-13.jsonl \
    --model cloudflare/clef-flash --repeat 3
impact refund-routing  (HEAD → working tree)
  model: typesafe/jev-1.13-20260917 → cloudflare/clef-flash
  24 logged requests asked again, 3 times each

  AUTO_REFUND → ESCALATE  1 input
  AUTO_REFUND → LLM_DRAFT  3 inputs
  HUMAN_REVIEW → ESCALATE  1 input
  ROUTE_TEAM → HUMAN_REVIEW  2 inputs

CHANGED line:5  (2 of 3 asks)  The jacket is too small. Can I send it back and get my money back?
    code: AUTO_REFUND → ESCALATE
    message: "Refund issued without review" → "Chargeback or legal threat, send to a senior agent"
    …
    jev.chargeback.noul: 0.05 → 0.5312
…
7 changed · 17 unchanged
```

For each logged request, the new model gets the same `state` and `questions`. Its answers replace the old ones in the rule input, the facts stay as they were, and the rules run again. A line counts as changed when the decision's code or message changes; the answers that moved are listed under it.

- The log must have been written with `--decide-log-requests`. Decisions logged without their request are counted and skipped.
- The new model is called at `https://openrouter.ai/api/v1/systemone` unless you pass `--upstream` or set `ORDO_DECIDE_UPSTREAM`. The key comes from `ORDO_DECIDE_API_KEY`, `OPENROUTER_API_KEY` or `TYPESAFE_API_KEY`. Each request is a paid model call.
- Pass the model you already use (`--model typesafe/jev-1.13`) to see how much changes from asking the same model twice. Models do not answer exactly the same way every time. `--repeat 3` asks each request three times and shows on how many asks the decision changed; one that changes on only some asks sits right at a threshold.
- A rule edit in the working tree is applied on the new side too, so you can check "switch model and move the threshold" in one run.
## Options

| Option | Environment variable | What it does |
| --- | --- | --- |
| `--decide-upstream` | `ORDO_DECIDE_UPSTREAM` | Base URL of the upstream; setting it turns the gateway on |
| `--decide-api-key` | `ORDO_DECIDE_API_KEY` | API key sent upstream; required together with `--api-token` |
| `--decide-log-dir` | `ORDO_DECIDE_LOG_DIR` | Decision log directory; no log without it |
| `--decide-log-requests` | `ORDO_DECIDE_LOG_REQUESTS` | Also log the request (`state`, `questions`, `model`), which `ordo impact --model` needs; off by default |

With `--api-token`, the gateway needs Ordo's token like every other data endpoint. The application's SDK key is then Ordo's token, and the upstream key comes from `ORDO_DECIDE_API_KEY`.

## Not done yet

- Retries, rate limiting and a fallback upstream. The gateway returns upstream errors as they are, and the caller's SDK retries.
- Sending a share of live traffic to a second model in the background (shadow mode). Today the comparison runs on logged requests, from the CLI.
