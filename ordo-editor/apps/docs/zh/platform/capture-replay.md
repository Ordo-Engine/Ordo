# 流量捕获与重放

改规则时，可以先在生产里记录真实决策，再用这些记录重放规则改动，查看哪些决策发生了翻转。Ordo 的测试用例格式是 `{input, expect:{code, output}}`，和一条捕获的 `{input, code, output}` 决策结构相同，所以生产流量几乎不需要额外处理就能变成回归语料。

流程：

> 改规则 → 重放上周的真实决策 → 检查翻转 → 固化成回归测试 → 上线。

## 1. 捕获（ordo-server）

捕获是可选功能，默认关闭。给 ordo-server 指定一个目录：

```bash
ordo-server --rules-dir ./rules --capture-io-path /var/ordo/capture
```

之后每次规则执行都会向 `/var/ordo/capture/capture-YYYY-MM-DD.jsonl` 追加一行（按天轮转）：

```json
{"ts":"…","rule_name":"listing-risk","tenant":"lumate","input":{"amount":5000,"is_vip":true},"code":"REVIEW","output":{…},"duration_us":42,"source_ip":"…"}
```

环境变量：`ORDO_CAPTURE_IO_PATH`、`ORDO_CAPTURE_IO_SAMPLE_RATE`（0–100，默认 100，即启用时全量捕获）。

::: warning 成本与隐私

- 关闭时没有开销。只有捕获开启且该请求被采样时才会克隆输入。
- 捕获的是完整请求 payload，可能包含 PII。所以捕获默认关闭。用采样率控制数据量和暴露面，并把捕获文件当作敏感数据处理。
- v1 只捕获 HTTP execute（单条，暂不支持批量）。gRPC 与批量捕获会在后续版本支持。
  :::

## 2. 重放（ordo CLI）

把捕获文件拉到一台有规则集项目的机器上，执行重放：

```bash
ordo replay capture-2026-07-04.jsonl
```

重放会用当前项目的规则集重新执行每条捕获的 `input`，并把每条记录归入以下分类：

| 桶                  | 含义                                     |
| ------------------- | ---------------------------------------- |
| **consistent**      | 与捕获时决策一致                         |
| **flipped**         | code 或 output 相对捕获基线变了（带 diff） |
| **errored**         | 执行失败                                 |
| **unknown-ruleset** | 记录指向本项目里不存在的规则             |
| **replayed**        | 仅有输入的捕获（无基线可比）               |

```text
FLIP listing-risk  {"amount":25000,…}  REVIEW → ALLOW
     expected code: "REVIEW", got: "ALLOW"

12,401 records: 12,388 consistent · 13 flipped
```

这 13 条翻转就是本次规则改动会改变的决策，上线前应逐条检查。其他选项：

- `--json`：输出完整的分类汇总和每条 diff。
- `--fail-on-flip`：有翻转时以非零状态退出，可用作 CI 检查。
- `--ruleset <name>`：强制指定单个规则。
- source 传 `-`：从 stdin 读取 JSONL。

## 3. 固化成回归测试

把捕获的决策保存为回归测试集：

```bash
ordo replay capture-2026-07-04.jsonl --write-tests
ordo test        # 你的生产流量现在是一套测试
```

`--write-tests` 把每条捕获的 `{input → code, output}` 合并进 `tests/<rule>.json`（按输入去重）。之后 `ordo test` 会检查这些用例，后续改动如果改变了这些真实决策，测试会失败。

## 其他数据来源

`ordo replay` 可以读取任何每行包含 `{rule_name, input, code, output}` 的 JSONL。如果你的应用已经记录了自己的决策（例如调用 Ordo 的服务为每条决策记录 `{input, code}`），可以直接重放这份日志，不需要开启捕获。
