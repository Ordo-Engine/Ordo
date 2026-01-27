package ordo

import (
	"net/http"
	"time"

	grpcClient "github.com/pama-lee/ordo-go/ordo/grpc"
	httpClient "github.com/pama-lee/ordo-go/ordo/http"
	"github.com/pama-lee/ordo-go/ordo/retry"
	"google.golang.org/grpc"
)

// ClientOptions defines configuration for the Ordo client
type ClientOptions struct {
	// HTTP configuration
	HTTPAddress   string
	HTTPClient    *http.Client
	HTTPTransport *http.Transport

	// gRPC configuration
	GRPCAddress  string
	GRPCDialOpts []grpc.DialOption

	// Retry configuration
	RetryConfig *retry.Config

	// Batch configuration
	BatchConcurrency int

	// Protocol preference
	PreferGRPC bool
	HTTPOnly   bool
	GRPCOnly   bool
}

// ClientOption is a function that configures ClientOptions
type ClientOption func(*ClientOptions)

// WithHTTPAddress sets the HTTP server address
func WithHTTPAddress(address string) ClientOption {
	return func(o *ClientOptions) {
		o.HTTPAddress = address
	}
}

// WithGRPCAddress sets the gRPC server address
func WithGRPCAddress(address string) ClientOption {
	return func(o *ClientOptions) {
		o.GRPCAddress = address
	}
}

// WithHTTPClient sets a custom HTTP client
func WithHTTPClient(client *http.Client) ClientOption {
	return func(o *ClientOptions) {
		o.HTTPClient = client
	}
}

// WithHTTPTransport sets a custom HTTP transport (for connection pooling)
func WithHTTPTransport(transport *http.Transport) ClientOption {
	return func(o *ClientOptions) {
		o.HTTPTransport = transport
	}
}

// WithHTTPTransportConfig sets HTTP transport using configuration
func WithHTTPTransportConfig(config httpClient.TransportConfig) ClientOption {
	return func(o *ClientOptions) {
		o.HTTPTransport = httpClient.NewTransport(config)
	}
}

// WithGRPCDialOptions sets custom gRPC dial options
func WithGRPCDialOptions(opts ...grpc.DialOption) ClientOption {
	return func(o *ClientOptions) {
		o.GRPCDialOpts = opts
	}
}

// WithGRPCPoolConfig sets gRPC connection pool configuration
func WithGRPCPoolConfig(config grpcClient.PoolConfig) ClientOption {
	return func(o *ClientOptions) {
		o.GRPCDialOpts = grpcClient.NewDialOptions(config)
	}
}

// WithRetry enables retry with the specified configuration
func WithRetry(config retry.Config) ClientOption {
	return func(o *ClientOptions) {
		o.RetryConfig = &config
	}
}

// WithDefaultRetry enables retry with default configuration
func WithDefaultRetry() ClientOption {
	return func(o *ClientOptions) {
		config := retry.DefaultConfig()
		o.RetryConfig = &config
	}
}

// WithBatchConcurrency sets the concurrency limit for batch operations
func WithBatchConcurrency(concurrency int) ClientOption {
	return func(o *ClientOptions) {
		o.BatchConcurrency = concurrency
	}
}

// WithPreferGRPC sets gRPC as the preferred protocol
func WithPreferGRPC() ClientOption {
	return func(o *ClientOptions) {
		o.PreferGRPC = true
	}
}

// WithHTTPOnly forces HTTP-only mode
func WithHTTPOnly() ClientOption {
	return func(o *ClientOptions) {
		o.HTTPOnly = true
		o.GRPCOnly = false
	}
}

// WithGRPCOnly forces gRPC-only mode
func WithGRPCOnly() ClientOption {
	return func(o *ClientOptions) {
		o.GRPCOnly = true
		o.HTTPOnly = false
	}
}

// ExecuteOptions defines options for Execute calls
type ExecuteOptions struct {
	IncludeTrace bool
}

// ExecuteOption is a function that configures ExecuteOptions
type ExecuteOption func(*ExecuteOptions)

// WithTrace enables execution trace
func WithTrace(enabled bool) ExecuteOption {
	return func(o *ExecuteOptions) {
		o.IncludeTrace = enabled
	}
}

// BatchOptions defines options for batch Execute calls
type BatchOptions struct {
	Parallel     bool
	IncludeTrace bool
	Concurrency  int
}

// BatchOption is a function that configures BatchOptions
type BatchOption func(*BatchOptions)

// WithParallel enables parallel execution for batch operations
func WithParallel(enabled bool) BatchOption {
	return func(o *BatchOptions) {
		o.Parallel = enabled
	}
}

// WithBatchTrace enables trace for batch operations
func WithBatchTrace(enabled bool) BatchOption {
	return func(o *BatchOptions) {
		o.IncludeTrace = enabled
	}
}

// WithConcurrency sets the concurrency for batch operations
func WithConcurrency(concurrency int) BatchOption {
	return func(o *BatchOptions) {
		o.Concurrency = concurrency
	}
}

// DefaultClientOptions returns default client options
func DefaultClientOptions() ClientOptions {
	return ClientOptions{
		HTTPAddress:      "http://localhost:8080",
		GRPCAddress:      "localhost:50051",
		RetryConfig:      nil, // Disabled by default
		BatchConcurrency: 10,
		PreferGRPC:       true,
		HTTPOnly:         false,
		GRPCOnly:         false,
	}
}

// DefaultExecuteOptions returns default execute options
func DefaultExecuteOptions() ExecuteOptions {
	return ExecuteOptions{
		IncludeTrace: false,
	}
}

// DefaultBatchOptions returns default batch options
func DefaultBatchOptions() BatchOptions {
	return BatchOptions{
		Parallel:     true,
		IncludeTrace: false,
		Concurrency:  10,
	}
}

// Timeout sets the timeout for operations
func Timeout(d time.Duration) ClientOption {
	return func(o *ClientOptions) {
		if o.HTTPClient == nil {
			o.HTTPClient = &http.Client{Timeout: d}
		} else {
			o.HTTPClient.Timeout = d
		}
	}
}
