# gRPC API

Ordo provides a high-performance gRPC API for rule execution and management.

## Connection

Default address: `localhost:50051`

```bash
# Using grpcurl
grpcurl -plaintext localhost:50051 list
```

## Protocol Definition

```protobuf
syntax = "proto3";

package ordo;

service OrdoService {
  // Execute a rule
  rpc Execute(ExecuteRequest) returns (ExecuteResponse);
  
  // List all rules
  rpc ListRuleSets(ListRuleSetsRequest) returns (ListRuleSetsResponse);
  
  // Get a rule by name
  rpc GetRuleSet(GetRuleSetRequest) returns (GetRuleSetResponse);
  
  // Health check
  rpc Health(HealthRequest) returns (HealthResponse);
  
  // Evaluate expression
  rpc Eval(EvalRequest) returns (EvalResponse);
}
```

## Messages

### ExecuteRequest

```protobuf
message ExecuteRequest {
  string name = 1;           // Rule name
  string input_json = 2;     // Input data as JSON string
  bool trace = 3;            // Include execution trace
}
```

### ExecuteResponse

```protobuf
message ExecuteResponse {
  string code = 1;           // Result code
  string message = 2;        // Result message
  string output_json = 3;    // Output data as JSON string
  uint64 duration_us = 4;    // Execution time in microseconds
  TraceInfo trace = 5;       // Execution trace (if requested)
}

message TraceInfo {
  string path = 1;           // Execution path
  repeated StepInfo steps = 2;
}

message StepInfo {
  string id = 1;
  string name = 2;
  uint64 duration_us = 3;
}
```

### ListRuleSetsRequest / Response

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

### GetRuleSetRequest / Response

```protobuf
message GetRuleSetRequest {
  string name = 1;
}

message GetRuleSetResponse {
  string ruleset_json = 1;   // Full rule definition as JSON
}
```

### HealthRequest / Response

```protobuf
message HealthRequest {}

message HealthResponse {
  string status = 1;         // "healthy"
  string version = 2;        // Server version
  uint64 uptime_seconds = 3;
  int32 rules_count = 4;
}
```

### EvalRequest / Response

```protobuf
message EvalRequest {
  string expression = 1;     // Expression to evaluate
  string context_json = 2;   // Context data as JSON
}

message EvalResponse {
  string result_json = 1;    // Result as JSON
  string parsed = 2;         // Parsed expression (debug)
}
```

## Usage Examples

### Execute Rule (grpcurl)

```bash
grpcurl -plaintext \
  -d '{
    "name": "discount-check",
    "input_json": "{\"user\": {\"vip\": true}}",
    "trace": true
  }' \
  localhost:50051 ordo.OrdoService/Execute
```

Response:

```json
{
  "code": "VIP",
  "message": "VIP discount applied",
  "outputJson": "{\"discount\":0.2}",
  "durationUs": "2",
  "trace": {
    "path": "check_vip -> vip_discount",
    "steps": [
      {"id": "check_vip", "name": "Check VIP", "durationUs": "1"}
    ]
  }
}
```

### List Rules

```bash
grpcurl -plaintext localhost:50051 ordo.OrdoService/ListRuleSets
```

### Health Check

```bash
grpcurl -plaintext localhost:50051 ordo.OrdoService/Health
```

## Client Libraries

### Rust

```rust
use ordo_proto::ordo_service_client::OrdoServiceClient;
use ordo_proto::{ExecuteRequest, HealthRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = OrdoServiceClient::connect("http://localhost:50051").await?;
    
    // Health check
    let response = client.health(HealthRequest {}).await?;
    println!("Status: {}", response.into_inner().status);
    
    // Execute rule
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
    
    // Execute rule
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

# Execute rule
response = stub.Execute(ordo_pb2.ExecuteRequest(
    name="discount-check",
    input_json='{"user": {"vip": true}}',
    trace=False
))

print(f"Code: {response.code}, Duration: {response.duration_us}µs")
```

## Performance

gRPC provides better performance than HTTP for high-throughput scenarios:

| Metric | HTTP | gRPC |
|--------|------|------|
| Latency (p99) | ~500µs | ~200µs |
| Throughput | 54K QPS | 80K+ QPS |
| Payload size | Larger (JSON) | Smaller (Protobuf) |

## TLS Configuration

For production, enable TLS:

```bash
ordo-server --grpc-addr 0.0.0.0:50051 --grpc-tls-cert cert.pem --grpc-tls-key key.pem
```

## Disable gRPC

If you only need HTTP:

```bash
ordo-server --disable-grpc
```
