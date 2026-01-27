package grpc

import (
	"context"
	"encoding/json"
	"fmt"

	"github.com/pama-lee/ordo-go/ordo/types"
	pb "github.com/pama-lee/ordo-go/proto/v1"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
)

// Client implements gRPC-based Ordo client
type Client struct {
	conn   *grpc.ClientConn
	client pb.OrdoServiceClient
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

// GetRuleSet retrieves a ruleset by name via gRPC
func (c *Client) GetRuleSet(ctx context.Context, name string) (*types.RuleSet, error) {
	req := &pb.GetRuleSetRequest{Name: name}
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
