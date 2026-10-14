# 测试与影响

改规则时用到三个命令：

- `ordo test`：检查你写好的用例是否还通过。
- `ordo trace`：看一条输入走了哪条路径。
- `ordo impact`：列出这次改动影响的每一个决策，包括没有测试覆盖到的。

三个命令都在本地用内置引擎运行，不需要服务，也不需要网络。它们要在 `ordo init` 建出的项目里用（见[快速上手](./quick-start)）。

## 测试

`rulesets/<name>.json` 的测试放在 `tests/<name>.json`：

```json
[
  {
    "name": "金卡会员，大额订单",
    "input": { "user": { "tier": "gold" }, "order": { "amount": 1200 } },
    "expect": { "code": "OK", "output": { "rate": 0.15, "pay": 1020.0 } }
  }
]
```

```bash
ordo test             # 所有带测试的规则集
ordo test discount    # 只测一个
```

项目之外的单个规则文件：`ordo test --rule rule.json --tests cases.json`。

## 追踪

```text
$ ordo trace discount --input '{"user":{"tier":"gold"},"order":{"amount":1200}}'
code:    OK
output:  {
  "rate": 0.15,
  "pay": 1020.0
}

path:    pick_rate -> done
```

决策结果和预期不一样时用它。

## 影响分析

`ordo impact` 把规则集上一次提交的版本（git `HEAD`）和你改过的版本放在一起跑，列出结果变了的每一条输入。输入有三类：

- 测试用例，
- 用 `--inputs` 传入的真实输入（JSONL），
- 边界探测值：每个像 `amount >= 1000` 这样的比较、决策表的每个单元格，都取阈值下方、等于和上方三个值。

所以即使没有测试落在阈值附近，改动也能被看到。把快速上手里金卡的门槛从 1000 降到 800：

```text
$ ordo impact discount
impact discount  (HEAD → working tree)
  17 inputs: 2 tests, 0 captured, 15 boundary probes

  OK → OK  3 inputs

CHANGED probe:order.amount=999  {"order":{"amount":999},"user":{"tier":"gold"}}
    output.pay: 899.1 → 849.15
    output.rate: 0.1 → 0.15
...
3 changed · 14 unchanged
```

两个测试都还通过，impact 照样列出了变了的地方。

| 选项               | 作用                                     |
| ------------------ | ---------------------------------------- |
| `--base <rev>`     | 和另一个提交比较，而不是 `HEAD`          |
| `--base-file <f>`  | 和一个文件比较                           |
| `--inputs <f>`     | 加入真实输入：捕获文件、决策日志，或每行一个 JSON 输入 |
| `--no-probes`      | 只用测试和 `--inputs`                    |
| `--fail-on-change` | 有任何变化就以非零退出，用于 CI          |

## 用真实流量

ordo-server 可以记录每一次执行。给它一个目录就会开启：

```bash
ordo-server --rules-dir ./rules --capture-io-path /var/ordo/capture
```

每次执行往 `capture-YYYY-MM-DD.jsonl` 写一行，包括输入、结果码和输出。`--capture-io-sample-rate`（0 到 100，默认 100）控制记录多少。捕获的输入是完整请求，可能含有个人信息，所以默认关闭。目前只记录单次 HTTP 执行，不包括批量和 gRPC。

然后在项目里：

```bash
ordo impact discount --inputs capture-2026-10-14.jsonl   # 一次改动会影响多少真实请求
ordo replay capture-2026-10-14.jsonl                     # 用当前规则重跑
ordo replay capture-2026-10-14.jsonl --write-tests       # 存成测试用例
```

`replay` 把每条记录归为一致、翻转（附差异）、出错，或规则集不在这个项目里。`--fail-on-flip` 在有决策翻转时以非零退出。`--write-tests` 把捕获的决策合并进 `tests/<name>.json`，之后改动一旦改变这些决策，`ordo test` 就会失败。

`replay` 能读任何每行带 `rule_name`、`input`、`code`、`output` 的 JSONL，你自己服务写的日志也可以。

通过[决策模型网关](/zh/platform/decision-gateway)做出的决策，决策日志也可以同样传给 `--inputs`。

## 在 CI 里

这些命令都会返回正确的退出码：

```yaml
- run: npx @ordo-engine/cli validate
- run: npx @ordo-engine/cli test
- run: npx @ordo-engine/cli impact discount --fail-on-change
```

每个命令都支持 `--json`，输出机器可读的结果。
