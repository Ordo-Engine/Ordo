# 决策模型网关

Jev 这类决策模型负责判断，Ordo 负责决定接下来怎么做。网关把这两步接在一起：应用照常调用 Jev，只是请求先经过 Ordo。Ordo 把请求转给模型，可以顺便用模型的回答跑一个规则集，并把每一次调用记进决策日志。以后要改门槛，就能拿这些真实请求跑 `ordo impact`。

网关兼容 System One 接口（`/v1/systemone`）。TypeSafe 官方 API、OpenRouter，以及其他兼容这个接口的服务，都可以做上游。

## 开启

```bash
export ORDO_DECIDE_API_KEY=...   # 上游的 API key，只放在服务端

ordo-server \
  --rules-dir ./rulesets \
  --decide-upstream https://api.typesafe.ai \
  --decide-log-dir /var/ordo/decisions
```

走 OpenRouter 时，上游写 `https://openrouter.ai/api`。没有设置 `ORDO_DECIDE_API_KEY` 时，Ordo 会把调用方的 `Authorization` 头原样转发给上游。

应用这边只改 base URL：

```bash
export TYPESAFE_BASE_URL=http://ordo:8080/systemone
```

SDK 会请求 `http://ordo:8080/systemone/v1/systemone`。Ordo 把请求转给上游，再把回答原样返回，401、422、429（连同 `retry-after`）这些错误也一样，所以应用里现有的错误处理不用改。

规则目录可以直接指向规则包的 `rulesets/`。Studio、命令行和规则包写出的规则文件，ordo-server 都能直接读。

## 让规则做决定

在请求里加一个 `ordo` 字段，写上规则集和业务事实：

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

`ordo` 字段不会发给模型。模型回答之后，Ordo 把回答放在 `jev` 下面，和 `facts` 合成规则输入，执行规则集，再把决定附在响应里：

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

几点说明：

- 回答放在规则输入的哪个字段下，默认是 `jev`，可以用 `ordo.answers_field` 改。
- 规则集不存在时，Ordo 直接返回 404，不会去调用模型。
- 规则执行出错时（比如缺了必填字段），响应仍然是 200，模型的回答照常返回，错误放在 `ordo.error` 里。模型已经回答过了，这次调用的费用也已经产生。
- 多租户时，用 `X-Tenant-ID` 指定租户，和其他接口一样。

## 决策日志

开了 `--decide-log-dir` 之后，每次调用往 `decisions-YYYY-MM-DD.jsonl` 写一行（按天轮转）：

```json
{"decision_id":"dec_…","ts":"…","tenant":"default","model":"jev-1.13.0","latency_ms":171,"upstream_status":200,"answers":{…},"ruleset":"refund-routing","rule_version":"1.0.0","input":{…},"code":"AUTO_REFUND","output":{…},"duration_us":55}
```

没有带 `ordo` 字段的调用也会记下来，只是没有规则输入和决定。上游返回错误时，记录里有 `upstream_status`。客户发来的原文（`state`）和问题默认不记，需要时加 `--decide-log-requests`，并把日志当作敏感数据处理。

## 改门槛之前，先用真实请求跑一遍

```bash
ordo impact refund-routing --inputs decisions-2026-10-13.jsonl
```

`impact` 读日志里每一行的规则输入，跳过其他规则集的记录和没有跑规则的调用。这样看到的不只是测试用例和边界探测值，还有真实的退款请求里，有多少个会因为这次改动换一种处理方式。

## 选项

| 选项 | 环境变量 | 说明 |
| --- | --- | --- |
| `--decide-upstream` | `ORDO_DECIDE_UPSTREAM` | 上游的 base URL，设置后网关开启 |
| `--decide-api-key` | `ORDO_DECIDE_API_KEY` | 发给上游的 API key；和 `--api-token` 同时使用时必须设置 |
| `--decide-log-dir` | `ORDO_DECIDE_LOG_DIR` | 决策日志目录，不设置就不写日志 |
| `--decide-log-requests` | `ORDO_DECIDE_LOG_REQUESTS` | 日志里同时记录 `state` 和 `questions`，默认关闭 |

开了 `--api-token` 时，网关和其他数据接口一样需要 Ordo 的 token。这时应用的 SDK key 填 Ordo 的 token，上游的 key 由 `ORDO_DECIDE_API_KEY` 提供。

## 还没有做的

- 重试、限流和备用上游。网关把上游的错误原样返回，由调用方的 SDK 重试。
- 换模型重新提问（比如 Jev 升级前，用新版本把上周的请求再问一遍）。这需要日志里有 `state` 原文，会在后续版本里支持。
