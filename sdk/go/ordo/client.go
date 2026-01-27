package ordo

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"

	"github.com/pama-lee/ordo-go/ordo/batch"
	grpcClient "github.com/pama-lee/ordo-go/ordo/grpc"
	httpClient "github.com/pama-lee/ordo-go/ordo/http"
	"github.com/pama-lee/ordo-go/ordo/retry"
	"github.com/pama-lee/ordo-go/ordo/types"
)

// Client is the unified Ordo client interface
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

// client is the concrete implementation of Client
type client struct {
	httpClient *httpClient.Client
	grpcClient *grpcClient.Client
	retrier    *retry.Retrier
	batchExec  *batch.Executor
	options    ClientOptions
}

// NewClient creates a new Ordo client with the given options
func NewClient(opts ...ClientOption) (Client, error) {
	options := DefaultClientOptions()
	for _, opt := range opts {
		opt(&options)
	}

	// Validate configuration
	if options.HTTPOnly && options.GRPCOnly {
		return nil, &types.ConfigError{Message: "cannot set both HTTPOnly and GRPCOnly"}
	}

	if !options.GRPCOnly && options.HTTPAddress == "" {
		return nil, &types.ConfigError{Message: "HTTP address is required when not in gRPC-only mode"}
	}

	if options.GRPCOnly && options.GRPCAddress == "" {
		return nil, &types.ConfigError{Message: "gRPC address is required when in gRPC-only mode"}
	}

	c := &client{
		options: options,
	}

	// Initialize HTTP client
	if !options.GRPCOnly {
		var httpCli *http.Client
		if options.HTTPClient != nil {
			httpCli = options.HTTPClient
		} else if options.HTTPTransport != nil {
			httpCli = &http.Client{Transport: options.HTTPTransport}
		}
		c.httpClient = httpClient.NewClient(options.HTTPAddress, httpCli)
	}

	// Initialize gRPC client
	if !options.HTTPOnly && options.GRPCAddress != "" {
		grpcCli, err := grpcClient.NewClient(options.GRPCAddress, options.GRPCDialOpts...)
		if err != nil {
			return nil, fmt.Errorf("failed to create gRPC client: %w", err)
		}
		c.grpcClient = grpcCli
	}

	// Initialize retrier
	if options.RetryConfig != nil {
		c.retrier = retry.NewRetrier(*options.RetryConfig)
	}

	// Initialize batch executor
	c.batchExec = batch.NewExecutor(options.BatchConcurrency)

	return c, nil
}

// Execute executes a ruleset
func (c *client) Execute(ctx context.Context, name string, input any, opts ...ExecuteOption) (*ExecuteResult, error) {
	execOpts := DefaultExecuteOptions()
	for _, opt := range opts {
		opt(&execOpts)
	}

	var result *types.ExecuteResult
	var execErr error

	fn := func() error {
		var err error

		// Choose protocol
		if c.shouldUseGRPC() {
			result, err = c.grpcClient.Execute(ctx, name, input, execOpts.IncludeTrace)
		} else {
			result, err = c.httpClient.Execute(ctx, name, input, execOpts.IncludeTrace)
		}

		if err != nil {
			execErr = err
			return err
		}

		return nil
	}

	if c.retrier != nil {
		if err := c.retrier.Do(ctx, fn); err != nil {
			return nil, err
		}
	} else {
		if err := fn(); err != nil {
			return nil, execErr
		}
	}

	return result, nil
}

// ExecuteBatch executes a ruleset with multiple inputs
func (c *client) ExecuteBatch(ctx context.Context, name string, inputs []any, opts ...BatchOption) (*BatchResult, error) {
	batchOpts := DefaultBatchOptions()
	for _, opt := range opts {
		opt(&batchOpts)
	}

	if batchOpts.Concurrency > 0 {
		c.batchExec = batch.NewExecutor(batchOpts.Concurrency)
	}

	// Use HTTP batch API if available and not in gRPC-only mode
	if c.httpClient != nil && batchOpts.Parallel && !c.options.GRPCOnly {
		return c.httpClient.ExecuteBatch(ctx, name, inputs, batchOpts.Parallel, batchOpts.IncludeTrace)
	}

	// Fall back to gRPC parallel execution
	if c.grpcClient != nil {
		execFn := func(ctx context.Context, input any, includeTrace bool) (*batch.Result, error) {
			r, err := c.grpcClient.Execute(ctx, name, input, includeTrace)
			if err != nil {
				return nil, err
			}
			return &batch.Result{
				Code:       r.Code,
				Message:    r.Message,
				Output:     r.Output,
				DurationUs: r.DurationUs,
				Trace:      r.Trace,
			}, nil
		}
		batchRes, err := c.batchExec.ExecuteParallel(ctx, inputs, batchOpts.IncludeTrace, execFn)
		if err != nil {
			return nil, err
		}
		return convertBatchResult(batchRes), nil
	}

	// Fall back to HTTP parallel execution
	if c.httpClient != nil {
		execFn := func(ctx context.Context, input any, includeTrace bool) (*batch.Result, error) {
			r, err := c.httpClient.Execute(ctx, name, input, includeTrace)
			if err != nil {
				return nil, err
			}
			return &batch.Result{
				Code:       r.Code,
				Message:    r.Message,
				Output:     r.Output,
				DurationUs: r.DurationUs,
				Trace:      r.Trace,
			}, nil
		}
		batchRes, err := c.batchExec.ExecuteParallel(ctx, inputs, batchOpts.IncludeTrace, execFn)
		if err != nil {
			return nil, err
		}
		return convertBatchResult(batchRes), nil
	}

	return nil, &types.ConfigError{Message: "no client available for batch execution"}
}

// convertBatchResult converts batch.BatchResult to types.BatchResult
func convertBatchResult(br *batch.BatchResult) *types.BatchResult {
	items := make([]types.BatchItem, len(br.Results))
	for i, r := range br.Results {
		var trace *types.ExecutionTrace
		if r.Trace != nil {
			if t, ok := r.Trace.(*types.ExecutionTrace); ok {
				trace = t
			}
		}
		items[i] = types.BatchItem{
			Code:       r.Code,
			Message:    r.Message,
			Output:     json.RawMessage(r.Output),
			DurationUs: r.DurationUs,
			Trace:      trace,
			Error:      r.Error,
		}
	}
	return &types.BatchResult{
		Results: items,
		Summary: types.BatchSummary{
			Total:           br.Summary.Total,
			Success:         br.Summary.Success,
			Failed:          br.Summary.Failed,
			TotalDurationUs: br.Summary.TotalDurationUs,
		},
	}
}

// ListRuleSets lists all rulesets (HTTP only)
func (c *client) ListRuleSets(ctx context.Context) ([]RuleSetSummary, error) {
	if c.httpClient == nil {
		return nil, &types.ConfigError{Message: "HTTP client required for ListRuleSets"}
	}
	return c.httpClient.ListRuleSets(ctx)
}

// GetRuleSet retrieves a ruleset by name
func (c *client) GetRuleSet(ctx context.Context, name string) (*RuleSet, error) {
	if c.shouldUseGRPC() && c.grpcClient != nil {
		return c.grpcClient.GetRuleSet(ctx, name)
	}
	if c.httpClient != nil {
		return c.httpClient.GetRuleSet(ctx, name)
	}
	return nil, &types.ConfigError{Message: "no client available"}
}

// CreateRuleSet creates a ruleset (HTTP only)
func (c *client) CreateRuleSet(ctx context.Context, ruleset *RuleSet) error {
	if c.httpClient == nil {
		return &types.ConfigError{Message: "HTTP client required for CreateRuleSet"}
	}
	return c.httpClient.CreateRuleSet(ctx, ruleset)
}

// UpdateRuleSet updates a ruleset (HTTP only)
func (c *client) UpdateRuleSet(ctx context.Context, ruleset *RuleSet) error {
	if c.httpClient == nil {
		return &types.ConfigError{Message: "HTTP client required for UpdateRuleSet"}
	}
	return c.httpClient.UpdateRuleSet(ctx, ruleset)
}

// DeleteRuleSet deletes a ruleset (HTTP only)
func (c *client) DeleteRuleSet(ctx context.Context, name string) error {
	if c.httpClient == nil {
		return &types.ConfigError{Message: "HTTP client required for DeleteRuleSet"}
	}
	return c.httpClient.DeleteRuleSet(ctx, name)
}

// ListVersions lists all versions of a ruleset (HTTP only)
func (c *client) ListVersions(ctx context.Context, name string) (*VersionList, error) {
	if c.httpClient == nil {
		return nil, &types.ConfigError{Message: "HTTP client required for ListVersions"}
	}
	return c.httpClient.ListVersions(ctx, name)
}

// Rollback rolls back a ruleset to a previous version (HTTP only)
func (c *client) Rollback(ctx context.Context, name string, seq int) (*RollbackResult, error) {
	if c.httpClient == nil {
		return nil, &types.ConfigError{Message: "HTTP client required for Rollback"}
	}
	return c.httpClient.Rollback(ctx, name, seq)
}

// Eval evaluates an expression
func (c *client) Eval(ctx context.Context, expr string, contextData any) (*EvalResult, error) {
	if c.shouldUseGRPC() && c.grpcClient != nil {
		return c.grpcClient.Eval(ctx, expr, contextData)
	}
	if c.httpClient != nil {
		return c.httpClient.Eval(ctx, expr, contextData)
	}
	return nil, &types.ConfigError{Message: "no client available"}
}

// Health checks server health
func (c *client) Health(ctx context.Context) (*HealthStatus, error) {
	if c.shouldUseGRPC() && c.grpcClient != nil {
		return c.grpcClient.Health(ctx)
	}
	if c.httpClient != nil {
		return c.httpClient.Health(ctx)
	}
	return nil, &types.ConfigError{Message: "no client available"}
}

// Close closes all connections
func (c *client) Close() error {
	if c.grpcClient != nil {
		if err := c.grpcClient.Close(); err != nil {
			return err
		}
	}
	if c.httpClient != nil {
		if err := c.httpClient.Close(); err != nil {
			return err
		}
	}
	return nil
}

// shouldUseGRPC determines whether to use gRPC based on configuration
func (c *client) shouldUseGRPC() bool {
	if c.options.GRPCOnly {
		return true
	}
	if c.options.HTTPOnly {
		return false
	}
	return c.options.PreferGRPC && c.grpcClient != nil
}
