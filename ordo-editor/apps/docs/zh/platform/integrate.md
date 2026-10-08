# 运行时接入

规则发布后，应用在运行时调用**引擎**获取决策：一次请求输入，返回一个决策，执行时间在亚微秒级。应用对接的是引擎（热路径），不是控制面。

> Studio 里每个项目都有一个 **接入（Integrate）** 页面，会生成这些调用所需的内容：endpoint、项目的 tenant id，以及按所选规则集预填的 curl / Node / Python / Go 代码片段，可以直接复制。

## 决策调用

按名称指定规则，用 tenant 头把调用限定到你的项目（项目 id 就是执行时的 tenant）。

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

业务逻辑按 `code` 分支，从 `output` 读取计算出的字段。一次计算多条输入时，用 `POST /api/v1/execute/<name>/batch`。

## SDK

官方 SDK 封装了 REST/gRPC，内置重试，返回类型化的结果。

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

`sdk/go` 和 `sdk/java` 使用 gRPC（`OrdoService.Execute`），带 `x-tenant-id` 元数据。具体 API 见各 SDK 的 README。

## 传输方式

引擎通过三种传输提供同一套执行接口，按延迟和部署环境选择：

| 传输                     | 适用                         |
| ------------------------ | ---------------------------- |
| **HTTP REST**（`:8080`） | 默认选项，任何语言和服务都能接入 |
| **gRPC**（`:50051`）     | 高吞吐服务；Go/Java SDK 使用它  |
| **Unix 域套接字**        | 同一主机上的调用方，延迟最低 |

完整的请求/响应 schema 见 [HTTP API](/zh/api/http-api) 和 [gRPC API](/zh/api/grpc-api)。

## 引擎部署位置

- **托管**：平台运行引擎，发布的规则不需要自建服务即可调用。
- **自建**：在自己的网络里运行 `ordo-server`，用[接入令牌](/zh/platform/server-registry)连接平台。Ordo 引擎面向内网可信环境设计（auth/TLS 可选，不强制），决策数据可以只留在你的基础设施内。

## 事实与输入

规则的条件可以引用输入字段、**事实**（fact）和**概念**（concept）。概念是派生值，由引擎计算。事实是外部输入，在运行时调用的 `input` 对象中提供（未提供的事实按缺失或 null 处理，不会报错）。每条规则的输入/输出在它的[决策契约](/zh/platform/contracts)里建模。
