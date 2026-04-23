# 能力与外部调用

Ordo 用 capability 边界来承接外部副作用和运行时集成。这样规则引擎内部仍然保持确定性执行，而指标、审计、HTTP 外调等运行时行为则通过统一接口接入。

## 什么是 capability

一个 capability provider 暴露一个具名运行时服务，以及该服务上的一个或多个操作。

- provider 名称就是 capability 名，例如 `metrics.prometheus`、`audit.logger`、`network.http`
- operation 是 capability 上调用的方法，例如 `gauge`、`rule_executed`、`post`
- payload 是传给 provider 的对象数据，同时也是返回结果的主要承载体

运行时里，`ExternalCall` action 会被翻译成 capability 请求：

```json
{
  "action": "external_call",
  "service": "demo.echo",
  "method": "echo",
  "params": [["amount", { "Field": "amount" }]],
  "result_variable": "echo_result",
  "timeout_ms": 250
}
```

如果设置了 `result_variable`，Ordo 会把响应存到这个变量下面：

- `$echo_result.capability`
- `$echo_result.operation`
- `$echo_result.payload`
- `$echo_result.metadata`

## Server 内置 capability

当前 server 默认会注册这些 provider：

| Capability           | 分类      | 常见 operation                                             | 用途                              |
| -------------------- | --------- | ---------------------------------------------------------- | --------------------------------- |
| `metrics.prometheus` | `action`  | `gauge`、`counter`                                         | 通过 Prometheus sink 记录规则指标 |
| `audit.logger`       | `action`  | `rule_executed`                                            | 发出结构化执行审计事件            |
| `network.http`       | `network` | `get`、`post`、`put`、`patch`、`delete`、`head`、`options` | 发送出站 HTTP 请求                |

## Studio `externalCalls` 如何映射

Studio 的 action step 可以定义 `externalCalls`。现在 editor adapter 会按下面这套规则把它转成 engine 的 `external_call`。

### HTTP 调用

当目标是 HTTP 端点时，使用 `type: "http"`。

```ts
{
  type: 'http',
  target: 'PATCH https://api.example.com/score',
  params: {
    applicantId: Expr.variable('$.applicant.id'),
    score: Expr.number(720),
  },
  resultVariable: 'http_result',
  timeout: 1500,
}
```

会被转成：

```json
{
  "action": "external_call",
  "service": "network.http",
  "method": "patch",
  "params": [
    ["url", { "Literal": "https://api.example.com/score" }],
    [
      "json_body",
      {
        "Object": [
          ["applicantId", { "Field": "applicant.id" }],
          ["score", { "Literal": 720 }]
        ]
      }
    ]
  ],
  "result_variable": "http_result",
  "timeout_ms": 1500
}
```

规则如下：

- 如果 `target` 以 `METHOD + 空格 + URL` 开头，就使用这个 HTTP method
- 如果没有 method 前缀，默认使用 `POST`
- `params` 会被打包成 `json_body`
- `target` 会变成 `network.http` payload 里的 `url`

### Function 与 gRPC 调用

对于 `type: "function"` 和 `type: "grpc"`，editor 会把 `target` 当成 capability 引用。

支持的 target 形式：

- `demo.echo`
- `demo.echo#echo`
- `demo.echo::echo`

规则如下：

- `service` 是 capability 名称
- 如果 target 里带了 `#` 或 `::`，就把后半段解析成 `method`
- 如果没有显式 method，`function` 默认用 `invoke`，`grpc` 默认用 `call`
- `params` 会原样变成 capability payload

示例：

```ts
{
  type: 'function',
  target: 'demo.echo#echo',
  params: {
    payload: Expr.object({
      amount: Expr.variable('$.amount'),
      approved: Expr.boolean(true),
    }),
  },
  resultVariable: 'echo_result',
}
```

## capability payload 里支持的表达式

现在 editor adapter 可以把下列表达式序列化进 capability payload：

- 字面量
- 字段引用
- 数组
- 对象
- 二元与一元表达式
- 条件表达式
- 函数调用
- 类似 `$.user.profile.id` 这样的简单 member path

## 当前限制

Studio 模型里已经有一些字段，但引擎暂时还没有执行语义：

- `retry`
- `onError`
- `fallbackValue`

这些字段目前仍然只停留在 editor 模型层，还不会被 adapter 翻译成真正的 runtime 行为。

这意味着：

- `ExternalCall` 不会自动重试
- capability 调用失败时不会自动写入 fallback 值
- `onError` 还没有映射成引擎语义

如果你现在就需要这些行为，应该把它们实现在 capability provider 内部，或者在规则里拆成显式步骤处理。

## 示例 provider

仓库里有一个最小示例 [`examples/capability-demo`](https://github.com/Ordo-Engine/Ordo/tree/main/examples/capability-demo)。它注册了 `demo.echo` provider，通过 `ExternalCall` 调用它，并用 `$result.payload` 读取返回值。
