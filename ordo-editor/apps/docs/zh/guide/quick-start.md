# 快速上手

这一页带你写一条折扣规则，跑起来，加上测试，最后通过 HTTP 调用它。只需要 Node.js 18 以上，不用装 Rust。

## 1. 安装命令行

```bash
npm install -g @ordo-engine/cli
ordo --version
```

不想全局安装的话，下面的 `ordo` 都可以换成 `npx @ordo-engine/cli`。

## 2. 建一个规则项目

```bash
ordo init my-rules && cd my-rules
```

项目里自带一个示例规则 `loan-approval`。规则放在 `rulesets/`，测试放在 `tests/`。

## 3. 写一条规则

新建 `rulesets/discount.json`。这条规则按会员等级和订单金额决定折扣率，再算出应付金额：

```json
{
  "config": { "name": "discount", "version": "1.0.0", "entry_step": "pick_rate" },
  "steps": {
    "pick_rate": {
      "id": "pick_rate",
      "name": "Pick discount rate",
      "type": "decision_table",
      "inputs": ["user.tier", "order.amount"],
      "outputs": ["rate"],
      "rules": [
        { "when": ["gold", ">= 1000"], "then": [0.15] },
        { "when": ["gold", "*"],       "then": [0.10] },
        { "when": ["*", ">= 1000"],    "then": [0.05] }
      ],
      "default": [0],
      "next_step": "done"
    },
    "done": {
      "id": "done",
      "name": "Done",
      "type": "terminal",
      "result": {
        "code": "OK",
        "output": [
          ["rate", "$rate"],
          ["pay", "order.amount * (1 - $rate)"]
        ]
      }
    }
  }
}
```

`pick_rate` 是一张决策表：从上往下匹配，第一行命中就用它的结果，`*` 表示任意值，都没命中就用 `default`。`done` 是终止步骤，`$rate` 是上一步设置的变量。决策表的完整写法见[决策表](./decision-table)。

## 4. 运行

```bash
ordo validate
ordo trace discount --input '{"user":{"tier":"gold"},"order":{"amount":1200}}'
```

```text
code:    OK
output:  {
  "rate": 0.15,
  "pay": 1020.0
}

path:    pick_rate -> done
```

`validate` 会编译所有规则并报告错误。`trace` 执行规则，并列出经过的每一步。

## 5. 加测试

新建 `tests/discount.json`：

```json
[
  {
    "name": "gold member, large order",
    "input": { "user": { "tier": "gold" }, "order": { "amount": 1200 } },
    "expect": { "code": "OK", "output": { "rate": 0.15, "pay": 1020.0 } }
  },
  {
    "name": "regular member, small order",
    "input": { "user": { "tier": "silver" }, "order": { "amount": 300 } },
    "expect": { "code": "OK", "output": { "rate": 0, "pay": 300 } }
  }
]
```

```bash
ordo test
```

以后改规则，先跑 `ordo test`。把它加进 CI，规则改错了就过不了。

## 6. 作为服务调用

业务系统通过 `ordo-server` 调用规则。用 Docker 启动：

```bash
docker run -p 8080:8080 ghcr.io/ordo-engine/ordo:latest
```

上传规则，然后执行：

```bash
curl -X POST http://localhost:8080/api/v1/rulesets \
  -H 'Content-Type: application/json' \
  -d @rulesets/discount.json

curl -X POST http://localhost:8080/api/v1/execute/discount \
  -H 'Content-Type: application/json' \
  -d '{"input":{"user":{"tier":"gold"},"order":{"amount":1200}}}'
```

```json
{ "code": "OK", "message": "", "output": { "rate": 0.15, "pay": 1020.0 }, "duration_us": 12 }
```

要让服务从目录加载规则、保存版本历史，见[规则持久化](./persistence)。其他启动方式见[安装与运行](./getting-started)。

## 下一步

- [规则结构](./rule-structure)：所有步骤类型和字段
- [表达式语法](./expression-syntax)：条件和输出里能写什么
- [HTTP API](/zh/api/http-api)：完整的接口说明
