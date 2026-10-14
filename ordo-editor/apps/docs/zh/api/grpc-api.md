# gRPC API

默认地址：`localhost:50051`。服务名是 `ordo.v1.OrdoService`，定义在 [`crates/ordo-grpc/proto/ordo.proto`](https://github.com/Ordo-Engine/Ordo/blob/main/crates/ordo-grpc/proto/ordo.proto)。

| RPC | 作用 |
| --- | --- |
| `Execute` | 执行规则集 |
| `BatchExecute` | 对多条输入执行规则集（每次最多 1000 条） |
| `Eval` | 计算一个表达式 |
| `ListRuleSets` | 列出规则集，可按名称前缀过滤 |
| `GetRuleSet` | 以 JSON 获取一个规则集 |
| `Health` | 状态、版本、规则集数量、运行时长 |

输入和输出都以 JSON 字符串传递。

```protobuf
message ExecuteRequest {
  string ruleset_name = 1;
  string input_json = 2;
  bool include_trace = 3;
}

message ExecuteResponse {
  string code = 1;
  string message = 2;
  string output_json = 3;
  uint64 duration_us = 4;
  ExecutionTrace trace = 5;
}

message BatchExecuteRequest {
  string ruleset_name = 1;
  repeated string inputs_json = 2;
  BatchExecuteOptions options = 3;   // parallel, include_trace
}
```

## 用 grpcurl 试

服务没有开启反射，所以要指定 proto 文件：

```bash
grpcurl -plaintext -proto crates/ordo-grpc/proto/ordo.proto \
  -d '{"ruleset_name":"discount","input_json":"{\"user\":{\"tier\":\"gold\"},\"order\":{\"amount\":1200}}"}' \
  localhost:50051 ordo.v1.OrdoService/Execute
```

## 租户、认证和 TLS

- 多租户时，在 `x-tenant-id` 元数据里传租户。
- `--api-token` 只作用于 HTTP。
- TLS：`--grpc-tls-cert` 和 `--grpc-tls-key`。客户端证书：`--grpc-mtls-enabled` 加 `--grpc-tls-client-ca`。
- 读节点对写请求返回 `FAILED_PRECONDITION`。没通过输入校验的请求返回 `INVALID_ARGUMENT`。

## Unix Socket

`--uds-path /var/run/ordo.sock` 在 Unix Socket 上提供同样的 gRPC 服务，适合同一台机器上的调用方。

## 客户端

Go、Java、Python 的客户端 SDK 在 [`sdk/`](https://github.com/Ordo-Engine/Ordo/tree/main/sdk)。其他语言可以用 proto 文件生成客户端。
