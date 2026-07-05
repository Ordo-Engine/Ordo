# 流量捕获与重放

改规则时的安全网:**在生产里记录真实决策,然后拿它去重放你的规则改动,精确看出哪些决策翻转了。** 因为 Ordo 的测试用例是 `{input, expect:{code, output}}`——和一条捕获的 `{input, code, output}` 决策同一个形状——生产流量几乎零成本地变成回归语料。

闭环:

> 改规则 → 重放上周的真实决策 → 检查翻转 → 固化成回归测试 → 放心上线。

## 1. 捕获(ordo-server)

捕获**可选、默认关闭**。给 ordo-server 指一个目录:

```bash
ordo-server --rules-dir ./rules --capture-io-path /var/ordo/capture
```

此后每次规则执行都会往 `/var/ordo/capture/capture-YYYY-MM-DD.jsonl` 追加一行(按天轮转):

```json
{"ts":"…","rule_name":"listing-risk","tenant":"lumate","input":{"amount":5000,"is_vip":true},"code":"REVIEW","output":{…},"duration_us":42,"source_ip":"…"}
```

环境变量:`ORDO_CAPTURE_IO_PATH`、`ORDO_CAPTURE_IO_SAMPLE_RATE`(0–100,默认 100 = 启用时全量捕获)。

::: warning 成本与隐私

- 关闭时零开销——只有捕获开启**且**该请求被采样时才克隆输入。
- 捕获的是**完整请求 payload**,可能含 PII。捕获刻意设计为可选;用采样率控制量级(和暴露面),并把捕获文件当敏感数据对待。
- v1 捕获 **HTTP execute**(单条;批量暂未)。gRPC 与批量捕获是后续项。
  :::

## 2. 重放(ordo CLI)

把捕获文件拉到一台有你规则集项目的机器上,重放:

```bash
ordo replay capture-2026-07-04.jsonl
```

重放把每条捕获的 `input` 交给**当前**项目规则集重跑,并把每条记录归桶:

| 桶                  | 含义                                     |
| ------------------- | ---------------------------------------- |
| **consistent**      | 与捕获时决策一致                         |
| **flipped**         | code 或 output 相对捕获基线变了(带 diff) |
| **errored**         | 执行失败                                 |
| **unknown-ruleset** | 记录指向本项目里不存在的规则             |
| **replayed**        | 仅有输入的捕获(无基线可比)               |

```text
FLIP listing-risk  {"amount":25000,…}  REVIEW → ALLOW
     expected code: "REVIEW", got: "ALLOW"

12,401 records: 12,388 consistent · 13 flipped
```

那 13 条翻转正是你这次规则改动会改变的决策——上线前逐条审。`--json` 输出完整分桶汇总 + 每条 diff;`--fail-on-flip` 有翻转就非零退出(用于 CI 卡口);`--ruleset <name>` 强制指定单个规则;source 传 `-` 从 stdin 读 JSONL。

## 3. 固化成回归测试

把捕获的决策变成永久回归集:

```bash
ordo replay capture-2026-07-04.jsonl --write-tests
ordo test        # 你的生产流量现在是一套测试
```

`--write-tests` 把每条捕获的 `{input → code, output}` 合并进 `tests/<rule>.json`(按输入去重)。此后 `ordo test` 就会守着:未来的改动不能悄悄改掉这些真实决策。

## 不止 ordo-server

`ordo replay` 能读任何带 `{rule_name, input, code, output}` 行的 JSONL——所以如果你的应用本来就记录了自己的决策(比如一个调用 Ordo 的服务,每条决策记 `{input, code}`),你可以直接重放那份日志,根本不用开捕获。
