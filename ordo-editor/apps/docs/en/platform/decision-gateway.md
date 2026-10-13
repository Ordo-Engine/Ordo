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
docker run -p 8080:8080 -e ORDO_DECIDE_API_KEY=... ghcr.io/ordo-engine/ordo:latest \
  --rules-dir /app/rule-packs/jev-routing/rulesets \
  --decide-upstream https://openrouter.ai/api
```

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
{"decision_id":"dec_…","ts":"…","tenant":"default","model":"jev-1.13.0","latency_ms":171,"upstream_status":200,"answers":{…},"ruleset":"refund-routing","rule_version":"1.0.0","input":{…},"code":"AUTO_REFUND","output":{…},"duration_us":55}
```

Calls without an `ordo` field are logged too, just without a rule input or a decision. When the upstream returns an error, the line has its `upstream_status`. The customer's text (`state`) and the questions are not logged unless you add `--decide-log-requests`; if you do, treat the log as sensitive data.

## Check a threshold change against real requests

```bash
ordo impact refund-routing --inputs decisions-2026-10-13.jsonl
```

`impact` reads the rule input on each line and skips lines for other rulesets and calls where no ruleset ran. So besides the test cases and boundary probes, you see how many real refund requests a change would send down a different path.

## Options

| Option | Environment variable | What it does |
| --- | --- | --- |
| `--decide-upstream` | `ORDO_DECIDE_UPSTREAM` | Base URL of the upstream; setting it turns the gateway on |
| `--decide-api-key` | `ORDO_DECIDE_API_KEY` | API key sent upstream; required together with `--api-token` |
| `--decide-log-dir` | `ORDO_DECIDE_LOG_DIR` | Decision log directory; no log without it |
| `--decide-log-requests` | `ORDO_DECIDE_LOG_REQUESTS` | Also log `state` and `questions`; off by default |

With `--api-token`, the gateway needs Ordo's token like every other data endpoint. The application's SDK key is then Ordo's token, and the upstream key comes from `ORDO_DECIDE_API_KEY`.

## Not done yet

- Retries, rate limiting and a fallback upstream. The gateway returns upstream errors as they are, and the caller's SDK retries.
- Asking a new model version again (for example, re-asking last week's requests before a Jev upgrade). That needs the `state` text in the log and will come in a later version.
