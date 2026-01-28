# Ordo Go SDK

Official Go SDK for [Ordo](https://github.com/Pama-Lee/Ordo) - A high-performance rule engine with visual editor.

[![Go Version](https://img.shields.io/badge/go-1.22%2B-blue.svg)](https://golang.org/dl/)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](../LICENSE)

## Features

- ✅ **Dual Protocol Support** - HTTP REST and gRPC with automatic protocol selection
- ✅ **Connection Pooling** - Efficient connection reuse for both HTTP and gRPC
- ✅ **Automatic Retry** - Exponential backoff with configurable retry logic
- ✅ **Batch Execution** - High-throughput batch processing with concurrency control
- ✅ **Type Safe** - Strongly typed API with Go structs
- ✅ **Production Ready** - Timeout, context cancellation, error handling

## Installation

```bash
go get github.com/pama-lee/ordo-go
```

## Quick Start

```go
package main

import (
    "context"
    "fmt"
    "log"

    "github.com/pama-lee/ordo-go/ordo"
)

func main() {
    // Create client
    client, err := ordo.NewClient(
        ordo.WithHTTPAddress("http://localhost:8080"),
        ordo.WithGRPCAddress("localhost:50051"),
    )
    if err != nil {
        log.Fatal(err)
    }
    defer client.Close()

    // Execute rule
    result, err := client.Execute(context.Background(), "discount-check", map[string]any{
        "user": map[string]any{"vip": true, "age": 25},
    })
    if err != nil {
        log.Fatal(err)
    }

    fmt.Printf("Result: %s - %s\n", result.Code, result.Message)
}
```

## Usage

### Client Configuration

```go
// Basic configuration
client, _ := ordo.NewClient(
    ordo.WithHTTPAddress("http://localhost:8080"),
    ordo.WithGRPCAddress("localhost:50051"),
)

// With connection pooling
client, _ := ordo.NewClient(
    ordo.WithHTTPAddress("http://localhost:8080"),
    ordo.WithHTTPTransportConfig(httpClient.TransportConfig{
        MaxIdleConns:        100,
        MaxIdleConnsPerHost: 10,
        IdleConnTimeout:     90 * time.Second,
    }),
    ordo.WithGRPCPoolConfig(grpcClient.PoolConfig{
        MaxConnections:   10,
        KeepAliveTime:    30 * time.Second,
        KeepAliveTimeout: 10 * time.Second,
    }),
)

// With retry enabled
client, _ := ordo.NewClient(
    ordo.WithHTTPAddress("http://localhost:8080"),
    ordo.WithRetry(retry.Config{
        MaxAttempts:     3,
        InitialInterval: 100 * time.Millisecond,
        MaxInterval:     5 * time.Second,
        Jitter:          true,
    }),
)

// HTTP-only or gRPC-only mode
httpClient, _ := ordo.NewClient(
    ordo.WithHTTPAddress("http://localhost:8080"),
    ordo.WithHTTPOnly(),
)

grpcClient, _ := ordo.NewClient(
    ordo.WithGRPCAddress("localhost:50051"),
    ordo.WithGRPCOnly(),
)

// With multi-tenancy support
client, _ := ordo.NewClient(
    ordo.WithHTTPAddress("http://localhost:8080"),
    ordo.WithGRPCAddress("localhost:50051"),
    ordo.WithTenantID("my-tenant"), // Set default tenant ID
)
```

### Multi-Tenancy

The SDK supports multi-tenancy for both HTTP and gRPC protocols:

```go
// Set tenant ID during client creation
client, _ := ordo.NewClient(
    ordo.WithHTTPAddress("http://localhost:8080"),
    ordo.WithGRPCAddress("localhost:50051"),
    ordo.WithTenantID("tenant-a"),
)

// Or set tenant ID at runtime (for gRPC client)
if grpcClient := client.GRPCClient(); grpcClient != nil {
    grpcClient.SetTenantID("tenant-b")
}
```

For HTTP requests, the tenant ID is sent via the `X-Tenant-ID` header.
For gRPC requests, the tenant ID is sent via gRPC metadata with key `x-tenant-id`.

### Execute Rules

```go
ctx := context.Background()

// Simple execution
result, err := client.Execute(ctx, "discount-check", map[string]any{
    "user": map[string]any{"vip": true},
})

// With execution trace
result, err := client.Execute(ctx, "discount-check", input, 
    ordo.WithTrace(true),
)

if result.Trace != nil {
    fmt.Printf("Path: %s\n", result.Trace.Path)
    for _, step := range result.Trace.Steps {
        fmt.Printf("  Step: %s (%d µs)\n", step.StepName, step.DurationUs)
    }
}
```

### Batch Execution

```go
inputs := []any{
    map[string]any{"user": map[string]any{"vip": true}},
    map[string]any{"user": map[string]any{"vip": false}},
    // ... more inputs
}

result, err := client.ExecuteBatch(ctx, "discount-check", inputs,
    ordo.WithParallel(true),
    ordo.WithConcurrency(10),
)

fmt.Printf("Total: %d, Success: %d, Failed: %d\n",
    result.Summary.Total,
    result.Summary.Success,
    result.Summary.Failed,
)
```

### Rule Management

```go
// List all rulesets
rulesets, err := client.ListRuleSets(ctx)

// Get specific ruleset
ruleset, err := client.GetRuleSet(ctx, "discount-check")

// Create or update ruleset
err := client.CreateRuleSet(ctx, &ordo.RuleSet{
    Config: ordo.RuleSetConfig{
        Name:      "new-rule",
        Version:   "1.0.0",
        EntryStep: "start",
    },
    Steps: map[string]json.RawMessage{
        // ... step definitions
    },
})

// Delete ruleset
err := client.DeleteRuleSet(ctx, "old-rule")
```

### Version Management

```go
// List versions
versions, err := client.ListVersions(ctx, "discount-check")
for _, v := range versions.Versions {
    fmt.Printf("v%s (seq: %d) - %s\n", v.Version, v.Seq, v.Timestamp)
}

// Rollback to previous version
result, err := client.Rollback(ctx, "discount-check", 2)
fmt.Printf("Rolled back from %s to %s\n", 
    result.FromVersion, result.ToVersion)
```

### Expression Evaluation

```go
result, err := client.Eval(ctx, 
    "user.age >= 18 && user.vip == true",
    map[string]any{
        "user": map[string]any{"age": 25, "vip": true},
    },
)
```

### Health Check

```go
health, err := client.Health(ctx)
fmt.Printf("Status: %s, Version: %s, Uptime: %ds\n",
    health.Status, health.Version, health.UptimeSeconds)
```

## API Reference

### Client Interface

```go
type Client interface {
    // Rule execution
    Execute(ctx context.Context, name string, input any, opts ...ExecuteOption) (*ExecuteResult, error)
    ExecuteBatch(ctx context.Context, name string, inputs []any, opts ...BatchOption) (*BatchResult, error)
    
    // Rule management (HTTP only)
    ListRuleSets(ctx context.Context) ([]RuleSetSummary, error)
    GetRuleSet(ctx context.Context, name string) (*RuleSet, error)
    CreateRuleSet(ctx context.Context, ruleset *RuleSet) error
    UpdateRuleSet(ctx context.Context, ruleset *RuleSet) error
    DeleteRuleSet(ctx context.Context, name string) error
    
    // Version management (HTTP only)
    ListVersions(ctx context.Context, name string) (*VersionList, error)
    Rollback(ctx context.Context, name string, seq int) (*RollbackResult, error)
    
    // Expression evaluation
    Eval(ctx context.Context, expr string, context any) (*EvalResult, error)
    
    // Health check
    Health(ctx context.Context) (*HealthStatus, error)
    
    // Resource management
    Close() error
}
```

### Client Options

| Option | Description |
|--------|-------------|
| `WithHTTPAddress(address)` | Set HTTP server address |
| `WithGRPCAddress(address)` | Set gRPC server address |
| `WithHTTPClient(client)` | Use custom HTTP client |
| `WithHTTPTransportConfig(config)` | Configure HTTP connection pool |
| `WithGRPCDialOptions(opts...)` | Set gRPC dial options |
| `WithGRPCPoolConfig(config)` | Configure gRPC connection pool |
| `WithRetry(config)` | Enable retry with configuration |
| `WithDefaultRetry()` | Enable retry with defaults |
| `WithBatchConcurrency(n)` | Set batch concurrency limit |
| `WithPreferGRPC()` | Prefer gRPC over HTTP |
| `WithHTTPOnly()` | Force HTTP-only mode |
| `WithGRPCOnly()` | Force gRPC-only mode |
| `Timeout(duration)` | Set request timeout |

### Execute Options

| Option | Description |
|--------|-------------|
| `WithTrace(enabled)` | Enable/disable execution trace |

### Batch Options

| Option | Description |
|--------|-------------|
| `WithParallel(enabled)` | Enable/disable parallel execution |
| `WithBatchTrace(enabled)` | Enable/disable trace for batch |
| `WithConcurrency(n)` | Set concurrency for batch execution |

## Error Handling

```go
result, err := client.Execute(ctx, "my-rule", input)
if err != nil {
    // Check for specific error types
    var apiErr *ordo.APIError
    if errors.As(err, &apiErr) {
        fmt.Printf("API Error: code=%s status=%d message=%s\n",
            apiErr.Code, apiErr.StatusCode, apiErr.Message)
    }
    
    var configErr *ordo.ConfigError
    if errors.As(err, &configErr) {
        fmt.Printf("Config Error: %s\n", configErr.Message)
    }
}
```

## Retry Behavior

The SDK automatically retries on the following errors:

- **Network errors**: connection refused, timeout, connection reset
- **HTTP 5xx**: server errors
- **HTTP 429**: rate limiting
- **gRPC codes**: `UNAVAILABLE`, `DEADLINE_EXCEEDED`, `RESOURCE_EXHAUSTED`, `ABORTED`

Non-retryable errors (fail immediately):

- **HTTP 4xx**: client errors (except 429)
- **gRPC codes**: `INVALID_ARGUMENT`, `NOT_FOUND`, `PERMISSION_DENIED`, etc.

## Performance

### Protocol Selection

- **gRPC**: Lower latency (~200µs), higher throughput, recommended for high-frequency calls
- **HTTP**: Better compatibility, easier debugging, recommended for management operations

The SDK automatically selects the optimal protocol:
- Execution operations: Prefer gRPC (if available)
- Management operations: Use HTTP (gRPC doesn't support CRUD)

### Connection Pooling

```go
// HTTP: Default pool settings
MaxIdleConns:        100
MaxIdleConnsPerHost: 10
IdleConnTimeout:     90s

// gRPC: Automatic connection reuse
// Single connection supports multiplexing
```

### Batch Optimization

```go
// HTTP batch API (when available)
client.ExecuteBatch(ctx, name, inputs) // Single HTTP request

// gRPC parallel execution
// Concurrent gRPC calls with semaphore-based concurrency control
```

## Examples

See the [examples](./examples) directory for complete examples:

- [basic](./examples/basic/main.go) - Basic usage
- [batch](./examples/batch/main.go) - Batch execution
- [retry](./examples/retry/main.go) - Retry configuration

Run examples:

```bash
# Start Ordo server first
cd ../../
cargo run --release --bin ordo-server

# Run examples
cd sdk/go
go run examples/basic/main.go
go run examples/batch/main.go
go run examples/retry/main.go
```

## Contributing

Contributions are welcome! Please see the main [Ordo repository](https://github.com/Pama-Lee/Ordo) for contribution guidelines.

## License

MIT License - see [LICENSE](../../LICENSE) for details.

## Links

- [Ordo Main Repository](https://github.com/Pama-Lee/Ordo)
- [Ordo Documentation](https://pama-lee.github.io/Ordo/)
- [Live Playground](https://pama-lee.github.io/Ordo/)
