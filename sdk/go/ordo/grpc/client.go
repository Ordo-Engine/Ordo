package grpc

import (
	"context"
	"encoding/json"
	"fmt"

	"github.com/pama-lee/ordo-go/ordo/types"
	pb "github.com/pama-lee/ordo-go/proto/v1"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/metadata"
)

// Metadata keys for multi-tenancy
const (
	MetadataTenantID = "x-tenant-id"
)

// Client implements gRPC-based Ordo client with multi-tenancy support
type Client struct {
	conn     *grpc.ClientConn
	client   pb.OrdoServiceClient
	tenantID string // Default tenant ID for all requests
}

// NewClient creates a new gRPC client
func NewClient(address string, opts ...grpc.DialOption) (*Client, error) {
	if len(opts) == 0 {
		opts = []grpc.DialOption{grpc.WithTransportCredentials(insecure.NewCredentials())}
	}

	conn, err := grpc.NewClient(address, opts...)
	if err != nil {
		return nil, fmt.Errorf("failed to connect to gRPC server: %w", err)
	}

	return &Client{
		conn:   conn,
		client: pb.NewOrdoServiceClient(conn),
	}, nil
}

// SetTenantID sets the default tenant ID for all requests
func (c *Client) SetTenantID(tenantID string) {
	c.tenantID = tenantID
}

// withTenantContext adds tenant metadata to context if configured
func (c *Client) withTenantContext(ctx context.Context) context.Context {
	if c.tenantID == "" {
		return ctx
	}
	return metadata.AppendToOutgoingContext(ctx, MetadataTenantID, c.tenantID)
}

// Execute executes a ruleset via gRPC
func (c *Client) Execute(ctx context.Context, name string, input any, includeTrace bool) (*types.ExecuteResult, error) {
	inputJSON, err := json.Marshal(input)
	if err != nil {
		return nil, fmt.Errorf("failed to marshal input: %w", err)
	}

	req := &pb.ExecuteRequest{
		RulesetName:  name,
		InputJson:    string(inputJSON),
		IncludeTrace: includeTrace,
	}

	// Add tenant context
	ctx = c.withTenantContext(ctx)

	resp, err := c.client.Execute(ctx, req)
	if err != nil {
		return nil, fmt.Errorf("execute failed: %w", err)
	}

	result := &types.ExecuteResult{
		Code:       resp.Code,
		Message:    resp.Message,
		Output:     json.RawMessage(resp.OutputJson),
		DurationUs: resp.DurationUs,
	}

	if resp.Trace != nil {
		result.Trace = &types.ExecutionTrace{
			Path:  resp.Trace.Path,
			Steps: make([]types.StepTrace, len(resp.Trace.Steps)),
		}
		for i, step := range resp.Trace.Steps {
			result.Trace.Steps[i] = types.StepTrace{
				StepID:     step.StepId,
				StepName:   step.StepName,
				DurationUs: step.DurationUs,
				Result:     step.Result,
			}
		}
	}

	return result, nil
}

// BatchExecuteOptions configures batch execution
type BatchExecuteOptions struct {
	Parallel     bool // Execute in parallel (default: true)
	IncludeTrace bool // Include execution traces
}

// BatchExecute executes a ruleset with multiple inputs via gRPC
func (c *Client) BatchExecute(ctx context.Context, name string, inputs []any, opts *BatchExecuteOptions) (*types.BatchResult, error) {
	if len(inputs) == 0 {
		return nil, fmt.Errorf("inputs array cannot be empty")
	}

	// Marshal all inputs to JSON
	inputsJSON := make([]string, len(inputs))
	for i, input := range inputs {
		inputJSON, err := json.Marshal(input)
		if err != nil {
			return nil, fmt.Errorf("failed to marshal input at index %d: %w", i, err)
		}
		inputsJSON[i] = string(inputJSON)
	}

	// Build options
	pbOpts := &pb.BatchExecuteOptions{
		Parallel:     true, // default
		IncludeTrace: false,
	}
	if opts != nil {
		pbOpts.Parallel = opts.Parallel
		pbOpts.IncludeTrace = opts.IncludeTrace
	}

	req := &pb.BatchExecuteRequest{
		RulesetName: name,
		InputsJson:  inputsJSON,
		Options:     pbOpts,
	}

	// Add tenant context
	ctx = c.withTenantContext(ctx)

	resp, err := c.client.BatchExecute(ctx, req)
	if err != nil {
		return nil, fmt.Errorf("batch execute failed: %w", err)
	}

	// Convert results
	results := make([]types.ExecuteResultItem, len(resp.Results))
	for i, r := range resp.Results {
		item := types.ExecuteResultItem{
			Code:       r.Code,
			Message:    r.Message,
			Output:     json.RawMessage(r.OutputJson),
			DurationUs: r.DurationUs,
		}
		if r.Error != "" {
			item.Error = &r.Error
		}
		if r.Trace != nil {
			item.Trace = &types.ExecutionTrace{
				Path:  r.Trace.Path,
				Steps: make([]types.StepTrace, len(r.Trace.Steps)),
			}
			for j, step := range r.Trace.Steps {
				item.Trace.Steps[j] = types.StepTrace{
					StepID:     step.StepId,
					StepName:   step.StepName,
					DurationUs: step.DurationUs,
					Result:     step.Result,
				}
			}
		}
		results[i] = item
	}

	// Build summary
	var summary types.BatchSummary
	if resp.Summary != nil {
		summary = types.BatchSummary{
			Total:           resp.Summary.Total,
			Success:         resp.Summary.Success,
			Failed:          resp.Summary.Failed,
			TotalDurationUs: resp.Summary.TotalDurationUs,
		}
	}

	return &types.BatchResult{
		Results: results,
		Summary: summary,
	}, nil
}

// GetRuleSet retrieves a ruleset by name via gRPC
func (c *Client) GetRuleSet(ctx context.Context, name string) (*types.RuleSet, error) {
	req := &pb.GetRuleSetRequest{Name: name}

	// Add tenant context
	ctx = c.withTenantContext(ctx)

	resp, err := c.client.GetRuleSet(ctx, req)
	if err != nil {
		return nil, fmt.Errorf("get ruleset failed: %w", err)
	}

	var ruleset types.RuleSet
	if err := json.Unmarshal([]byte(resp.RulesetJson), &ruleset); err != nil {
		return nil, fmt.Errorf("failed to unmarshal ruleset: %w", err)
	}

	return &ruleset, nil
}

// ListRuleSets lists all rulesets via gRPC
func (c *Client) ListRuleSets(ctx context.Context, namePrefix string, limit uint32) ([]types.RuleSetSummary, error) {
	req := &pb.ListRuleSetsRequest{
		NamePrefix: namePrefix,
		Limit:      limit,
	}

	// Add tenant context
	ctx = c.withTenantContext(ctx)

	resp, err := c.client.ListRuleSets(ctx, req)
	if err != nil {
		return nil, fmt.Errorf("list rulesets failed: %w", err)
	}

	summaries := make([]types.RuleSetSummary, len(resp.Rulesets))
	for i, rs := range resp.Rulesets {
		summaries[i] = types.RuleSetSummary{
			Name:        rs.Name,
			Version:     rs.Version,
			Description: &rs.Description,
			StepCount:   rs.StepCount,
		}
	}

	return summaries, nil
}

// Eval evaluates an expression via gRPC
func (c *Client) Eval(ctx context.Context, expression string, contextData any) (*types.EvalResult, error) {
	contextJSON, err := json.Marshal(contextData)
	if err != nil {
		return nil, fmt.Errorf("failed to marshal context: %w", err)
	}

	req := &pb.EvalRequest{
		Expression:  expression,
		ContextJson: string(contextJSON),
	}

	// Add tenant context
	ctx = c.withTenantContext(ctx)

	resp, err := c.client.Eval(ctx, req)
	if err != nil {
		return nil, fmt.Errorf("eval failed: %w", err)
	}

	return &types.EvalResult{
		Result: json.RawMessage(resp.ResultJson),
		Parsed: resp.ParsedExpression,
	}, nil
}

// Health checks server health via gRPC
func (c *Client) Health(ctx context.Context) (*types.HealthStatus, error) {
	req := &pb.HealthRequest{}

	// Add tenant context (optional for health)
	ctx = c.withTenantContext(ctx)

	resp, err := c.client.Health(ctx, req)
	if err != nil {
		return nil, fmt.Errorf("health check failed: %w", err)
	}

	status := "unknown"
	switch resp.Status {
	case pb.HealthResponse_SERVING:
		status = "serving"
	case pb.HealthResponse_NOT_SERVING:
		status = "not_serving"
	}

	return &types.HealthStatus{
		Status:        status,
		Version:       resp.Version,
		RulesetCount:  resp.RulesetCount,
		UptimeSeconds: resp.UptimeSeconds,
	}, nil
}

// Close closes the gRPC connection
func (c *Client) Close() error {
	if c.conn != nil {
		return c.conn.Close()
	}
	return nil
}
