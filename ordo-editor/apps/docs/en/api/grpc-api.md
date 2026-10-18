# gRPC API

Default address: `localhost:50051`. The service is `ordo.v1.OrdoService`, defined in [`crates/ordo-grpc/proto/ordo.proto`](https://github.com/Ordo-Engine/Ordo/blob/main/crates/ordo-grpc/proto/ordo.proto).

| RPC | What it does |
| --- | --- |
| `Execute` | Run a ruleset |
| `BatchExecute` | Run a ruleset on many inputs (up to 1000 per call) |
| `Eval` | Evaluate one expression |
| `ListRuleSets` | List rulesets, optionally by name prefix |
| `GetRuleSet` | Get one ruleset as JSON |
| `Health` | Status, version, ruleset count, uptime |

Inputs and outputs are passed as JSON strings.

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

## Try it with grpcurl

The server has no reflection, so pass the proto file:

```bash
grpcurl -plaintext -proto crates/ordo-grpc/proto/ordo.proto \
  -d '{"ruleset_name":"discount","input_json":"{\"user\":{\"tier\":\"gold\"},\"order\":{\"amount\":1200}}"}' \
  localhost:50051 ordo.v1.OrdoService/Execute
```

## Tenants, auth and TLS

- With multi-tenancy, send the tenant in the `x-tenant-id` metadata key.
- `--api-token` applies to HTTP only.
- TLS: `--grpc-tls-cert` and `--grpc-tls-key`. Client certificates: `--grpc-mtls-enabled` with `--grpc-tls-client-ca`.
- A reader instance answers writes with `FAILED_PRECONDITION`. Input that fails the input schema gets `INVALID_ARGUMENT`.

## Unix socket

`--uds-path /var/run/ordo.sock` serves the same gRPC service on a Unix socket, for a caller on the same host.

## Clients

Client SDKs for Go, Java and Python are in [`sdk/`](https://github.com/Ordo-Engine/Ordo/tree/main/sdk). For other languages, generate a client from the proto file.
