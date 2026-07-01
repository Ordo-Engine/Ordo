# 运行时接入

规则一旦发布,你的应用在运行时调用**引擎**拿决策——一次请求进、一个决策出,执行时间在亚微秒级。你的应用对接的是引擎(热路径),不是控制面。

## 决策调用

按名字寻址规则;用 tenant 头把它限定到你的项目(你的项目 id 就是执行 tenant)。

```bash
POST https://<engine>/api/v1/execute/loan-approval
Header: x-tenant-id: <项目id>
Body:   { "input": { "amount": 5000, "is_vip": true } }
```

```json
{
  "code": "APPROVED",
  "message": "Within limit",
  "output": { "approved": true, "amount": 5000 },
  "duration_us": 6
}
```

按 `code` 分支你的业务逻辑(从 `output` 读计算出的字段)。要一次算很多输入,用 `POST /api/v1/execute/<name>/batch`。

## SDK

官方 SDK 封装了 REST/gRPC,自带重试和类型化结果。

### Python

```python
from ordo import OrdoClient

client = OrdoClient(http_address="https://<engine>", tenant_id="<项目id>")

result = client.execute("loan-approval", {"amount": 5000, "is_vip": True})
if result.code == "APPROVED":
    ...
print(result.code, result.output, f"{result.duration_us}µs")
```

### Go / Java

`sdk/go` 和 `sdk/java` 走 gRPC(`OrdoService.Execute`),带 `x-tenant-id` 元数据。具体 API 见各 SDK 的 README。

## 传输方式

引擎用三种传输暴露同一套执行,按延迟和环境选:

| 传输                     | 适用                         |
| ------------------------ | ---------------------------- |
| **HTTP REST**（`:8080`） | 默认——任何语言/服务都好接    |
| **gRPC**（`:50051`）     | 高吞吐服务;Go/Java SDK 用它  |
| **Unix 域套接字**        | 同机同主机的调用方——延迟最低 |

完整请求/响应 schema 见 [HTTP API](/zh/api/http-api) 和 [gRPC API](/zh/api/grpc-api)。

## 引擎跑在哪

- **托管**——平台跑引擎,你发布的规则无需自建即可调用。
- **自建**——在你自己的网络里跑 `ordo-server`,用[接入令牌](/zh/platform/server-registry)连到平台。Ordo 引擎为内网可信环境设计(auth/TLS 可选、不强制),决策可以完全留在你的基础设施内。

## 事实 vs 输入

规则的条件会引用**输入字段**、**事实(fact)**、**概念(concept)**。概念是派生的、由引擎计算。事实是外部输入——运行时调用里由你在 `input` 对象中提供(没提供的事实按缺失/null 处理,不报错)。在每条规则的[决策契约](/zh/platform/contracts)里建模它的输入/输出。
