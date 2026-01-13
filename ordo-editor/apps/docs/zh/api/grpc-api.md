# gRPC API

Ordo 提供高性能的 gRPC API 用于规则执行和管理。

## 连接

默认地址：`localhost:50051`

```bash
# 使用 grpcurl
grpcurl -plaintext localhost:50051 list
```

## 协议定义

```protobuf
syntax = "proto3";

package ordo;

service OrdoService {
  // 执行规则
  rpc Execute(ExecuteRequest) returns (ExecuteResponse);

  // 列出所有规则
  rpc ListRuleSets(ListRuleSetsRequest) returns (ListRuleSetsResponse);

  // 根据名称获取规则
  rpc GetRuleSet(GetRuleSetRequest) returns (GetRuleSetResponse);

  // 健康检查
  rpc Health(HealthRequest) returns (HealthResponse);

  // 评估表达式
  rpc Eval(EvalRequest) returns (EvalResponse);
}
```

## 消息

### ExecuteRequest (执行请求)

```protobuf
message ExecuteRequest {
  string name = 1;           // 规则名称
  string input_json = 2;     // 输入数据 (JSON 字符串)
  bool trace = 3;            // 包含执行追踪
}
```

### ExecuteResponse (执行响应)

```protobuf
message ExecuteResponse {
  string code = 1;           // 结果代码
  string message = 2;        // 结果消息
  string output_json = 3;    // 输出数据 (JSON 字符串)
  uint64 duration_us = 4;    // 执行时间 (微秒)
  TraceInfo trace = 5;       // 执行追踪 (如果请求)
}

message TraceInfo {
  string path = 1;           // 执行路径
  repeated StepInfo steps = 2;
}

message StepInfo {
  string id = 1;
  string name = 2;
  uint64 duration_us = 3;
}
```

### ListRuleSetsRequest / Response (列出规则)

```protobuf
message ListRuleSetsRequest {}

message ListRuleSetsResponse {
  repeated RuleSetInfo rulesets = 1;
}

message RuleSetInfo {
  string name = 1;
  string version = 2;
  string description = 3;
}
```

### GetRuleSetRequest / Response (获取规则)

```protobuf
message GetRuleSetRequest {
  string name = 1;
}

message GetRuleSetResponse {
  string ruleset_json = 1;   // 完整的规则定义 (JSON)
}
```

### HealthRequest / Response (健康检查)

```protobuf
message HealthRequest {}

message HealthResponse {
  string status = 1;         // "healthy"
  string version = 2;        // 服务器版本
  uint64 uptime_seconds = 3;
  int32 rules_count = 4;
}
```

### EvalRequest / Response (评估表达式)

```protobuf
message EvalRequest {
  string expression = 1;     // 要评估的表达式
  string context_json = 2;   // 上下文数据 (JSON)
}

message EvalResponse {
  string result_json = 1;    // 结果 (JSON)
  string parsed = 2;         // 解析后的表达式 (调试用)
}
```

## 使用示例

### 执行规则 (grpcurl)

```bash
grpcurl -plaintext \
  -d '{
    "name": "discount-check",
    "input_json": "{\"user\": {\"vip\": true}}",
    "trace": true
  }' \
  localhost:50051 ordo.OrdoService/Execute
```

响应：

```json
{
  "code": "VIP",
  "message": "VIP discount applied",
  "outputJson": "{\"discount\":0.2}",
  "durationUs": "2",
  "trace": {
    "path": "check_vip -> vip_discount",
    "steps": [{ "id": "check_vip", "name": "Check VIP", "durationUs": "1" }]
  }
}
```

### 列出规则

```bash
grpcurl -plaintext localhost:50051 ordo.OrdoService/ListRuleSets
```

### 健康检查

```bash
grpcurl -plaintext localhost:50051 ordo.OrdoService/Health
```

## 客户端库

### Rust

```rust
use ordo_proto::ordo_service_client::OrdoServiceClient;
use ordo_proto::{ExecuteRequest, HealthRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = OrdoServiceClient::connect("http://localhost:50051").await?;

    // 健康检查
    let response = client.health(HealthRequest {}).await?;
    println!("Status: {}", response.into_inner().status);

    // 执行规则
    let response = client.execute(ExecuteRequest {
        name: "discount-check".to_string(),
        input_json: r#"{"user": {"vip": true}}"#.to_string(),
        trace: false,
    }).await?;

    let result = response.into_inner();
    println!("Code: {}, Duration: {}µs", result.code, result.duration_us);

    Ok(())
}
```

### Go

```go
package main

import (
    "context"
    "log"

    pb "github.com/your-org/ordo-proto"
    "google.golang.org/grpc"
)

func main() {
    conn, err := grpc.Dial("localhost:50051", grpc.WithInsecure())
    if err != nil {
        log.Fatal(err)
    }
    defer conn.Close()

    client := pb.NewOrdoServiceClient(conn)

    // 执行规则
    resp, err := client.Execute(context.Background(), &pb.ExecuteRequest{
        Name:      "discount-check",
        InputJson: `{"user": {"vip": true}}`,
        Trace:     false,
    })
    if err != nil {
        log.Fatal(err)
    }

    log.Printf("Code: %s, Duration: %dµs", resp.Code, resp.DurationUs)
}
```

### Python

```python
import grpc
import ordo_pb2
import ordo_pb2_grpc

channel = grpc.insecure_channel('localhost:50051')
stub = ordo_pb2_grpc.OrdoServiceStub(channel)

# 执行规则
response = stub.Execute(ordo_pb2.ExecuteRequest(
    name="discount-check",
    input_json='{"user": {"vip": true}}',
    trace=False
))

print(f"Code: {response.code}, Duration: {response.duration_us}µs")
```

## 性能

在高吞吐量场景下，gRPC 提供比 HTTP 更好的性能：

| 指标       | HTTP        | gRPC            |
| ---------- | ----------- | --------------- |
| 延迟 (p99) | ~500µs      | ~200µs          |
| 吞吐量     | 54K QPS     | 80K+ QPS        |
| 载荷大小   | 较大 (JSON) | 较小 (Protobuf) |

## TLS 配置

生产环境建议启用 TLS：

```bash
ordo-server --grpc-addr 0.0.0.0:50051 --grpc-tls-cert cert.pem --grpc-tls-key key.pem
```

## 禁用 gRPC

如果你只需要 HTTP：

```bash
ordo-server --disable-grpc
```
