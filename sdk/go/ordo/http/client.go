package http

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"

	"github.com/pama-lee/ordo-go/ordo/types"
)

// Client implements HTTP-based Ordo client
type Client struct {
	baseURL    string
	httpClient *http.Client
}

// NewClient creates a new HTTP client
func NewClient(baseURL string, httpClient *http.Client) *Client {
	if httpClient == nil {
		httpClient = &http.Client{
			Timeout: 30 * time.Second,
		}
	}

	// Ensure baseURL ends with /api/v1
	baseURL = strings.TrimRight(baseURL, "/")
	if !strings.HasSuffix(baseURL, "/api/v1") {
		baseURL += "/api/v1"
	}

	return &Client{
		baseURL:    baseURL,
		httpClient: httpClient,
	}
}

// Execute executes a ruleset via HTTP
func (c *Client) Execute(ctx context.Context, name string, input any, includeTrace bool) (*types.ExecuteResult, error) {
	reqBody := map[string]any{
		"input": input,
		"trace": includeTrace,
	}

	data, err := json.Marshal(reqBody)
	if err != nil {
		return nil, fmt.Errorf("failed to marshal request: %w", err)
	}

	url := fmt.Sprintf("%s/execute/%s", c.baseURL, name)
	req, err := http.NewRequestWithContext(ctx, "POST", url, bytes.NewReader(data))
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read response: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		return nil, parseError(resp.StatusCode, body)
	}

	var result types.ExecuteResult
	if err := json.Unmarshal(body, &result); err != nil {
		return nil, fmt.Errorf("failed to unmarshal response: %w", err)
	}

	return &result, nil
}

// ExecuteBatch executes a ruleset with multiple inputs via HTTP batch API
func (c *Client) ExecuteBatch(ctx context.Context, name string, inputs []any, parallel bool, includeTrace bool) (*types.BatchResult, error) {
	reqBody := map[string]any{
		"inputs": inputs,
		"options": map[string]any{
			"parallel": parallel,
			"trace":    includeTrace,
		},
	}

	data, err := json.Marshal(reqBody)
	if err != nil {
		return nil, fmt.Errorf("failed to marshal request: %w", err)
	}

	url := fmt.Sprintf("%s/execute/%s/batch", c.baseURL, name)
	req, err := http.NewRequestWithContext(ctx, "POST", url, bytes.NewReader(data))
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read response: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		return nil, parseError(resp.StatusCode, body)
	}

	var result types.BatchResult
	if err := json.Unmarshal(body, &result); err != nil {
		return nil, fmt.Errorf("failed to unmarshal response: %w", err)
	}

	return &result, nil
}

// ListRuleSets lists all rulesets via HTTP
func (c *Client) ListRuleSets(ctx context.Context) ([]types.RuleSetSummary, error) {
	url := fmt.Sprintf("%s/rulesets", c.baseURL)
	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read response: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		return nil, parseError(resp.StatusCode, body)
	}

	var summaries []types.RuleSetSummary
	if err := json.Unmarshal(body, &summaries); err != nil {
		return nil, fmt.Errorf("failed to unmarshal response: %w", err)
	}

	return summaries, nil
}

// GetRuleSet retrieves a ruleset by name via HTTP
func (c *Client) GetRuleSet(ctx context.Context, name string) (*types.RuleSet, error) {
	url := fmt.Sprintf("%s/rulesets/%s", c.baseURL, name)
	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read response: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		return nil, parseError(resp.StatusCode, body)
	}

	var ruleset types.RuleSet
	if err := json.Unmarshal(body, &ruleset); err != nil {
		return nil, fmt.Errorf("failed to unmarshal response: %w", err)
	}

	return &ruleset, nil
}

// CreateRuleSet creates or updates a ruleset via HTTP
func (c *Client) CreateRuleSet(ctx context.Context, ruleset *types.RuleSet) error {
	data, err := json.Marshal(ruleset)
	if err != nil {
		return fmt.Errorf("failed to marshal ruleset: %w", err)
	}

	url := fmt.Sprintf("%s/rulesets", c.baseURL)
	req, err := http.NewRequestWithContext(ctx, "POST", url, bytes.NewReader(data))
	if err != nil {
		return fmt.Errorf("failed to create request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return fmt.Errorf("failed to read response: %w", err)
	}

	if resp.StatusCode != http.StatusOK && resp.StatusCode != http.StatusCreated {
		return parseError(resp.StatusCode, body)
	}

	return nil
}

// UpdateRuleSet updates a ruleset via HTTP (alias for CreateRuleSet)
func (c *Client) UpdateRuleSet(ctx context.Context, ruleset *types.RuleSet) error {
	return c.CreateRuleSet(ctx, ruleset)
}

// DeleteRuleSet deletes a ruleset via HTTP
func (c *Client) DeleteRuleSet(ctx context.Context, name string) error {
	url := fmt.Sprintf("%s/rulesets/%s", c.baseURL, name)
	req, err := http.NewRequestWithContext(ctx, "DELETE", url, nil)
	if err != nil {
		return fmt.Errorf("failed to create request: %w", err)
	}

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusNoContent && resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return parseError(resp.StatusCode, body)
	}

	return nil
}

// ListVersions lists all versions of a ruleset via HTTP
func (c *Client) ListVersions(ctx context.Context, name string) (*types.VersionList, error) {
	url := fmt.Sprintf("%s/rulesets/%s/versions", c.baseURL, name)
	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read response: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		return nil, parseError(resp.StatusCode, body)
	}

	var versions types.VersionList
	if err := json.Unmarshal(body, &versions); err != nil {
		return nil, fmt.Errorf("failed to unmarshal response: %w", err)
	}

	return &versions, nil
}

// Rollback rolls back a ruleset to a previous version via HTTP
func (c *Client) Rollback(ctx context.Context, name string, seq int) (*types.RollbackResult, error) {
	reqBody := map[string]any{"seq": seq}
	data, err := json.Marshal(reqBody)
	if err != nil {
		return nil, fmt.Errorf("failed to marshal request: %w", err)
	}

	url := fmt.Sprintf("%s/rulesets/%s/rollback", c.baseURL, name)
	req, err := http.NewRequestWithContext(ctx, "POST", url, bytes.NewReader(data))
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read response: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		return nil, parseError(resp.StatusCode, body)
	}

	var result types.RollbackResult
	if err := json.Unmarshal(body, &result); err != nil {
		return nil, fmt.Errorf("failed to unmarshal response: %w", err)
	}

	return &result, nil
}

// Eval evaluates an expression via HTTP
func (c *Client) Eval(ctx context.Context, expression string, contextData any) (*types.EvalResult, error) {
	reqBody := map[string]any{
		"expression": expression,
		"context":    contextData,
	}

	data, err := json.Marshal(reqBody)
	if err != nil {
		return nil, fmt.Errorf("failed to marshal request: %w", err)
	}

	url := fmt.Sprintf("%s/eval", c.baseURL)
	req, err := http.NewRequestWithContext(ctx, "POST", url, bytes.NewReader(data))
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read response: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		return nil, parseError(resp.StatusCode, body)
	}

	var result types.EvalResult
	if err := json.Unmarshal(body, &result); err != nil {
		return nil, fmt.Errorf("failed to unmarshal response: %w", err)
	}

	return &result, nil
}

// Health checks server health via HTTP
func (c *Client) Health(ctx context.Context) (*types.HealthStatus, error) {
	// Health endpoint is at /health, not /api/v1/health
	baseURL := strings.TrimSuffix(c.baseURL, "/api/v1")
	url := fmt.Sprintf("%s/health", baseURL)

	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read response: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		return nil, parseError(resp.StatusCode, body)
	}

	var status types.HealthStatus
	if err := json.Unmarshal(body, &status); err != nil {
		return nil, fmt.Errorf("failed to unmarshal response: %w", err)
	}

	return &status, nil
}

// Close closes the HTTP client (no-op for standard http.Client)
func (c *Client) Close() error {
	return nil
}

// parseError parses API error responses
func parseError(statusCode int, body []byte) error {
	var apiErr struct {
		Error string `json:"error"`
		Code  string `json:"code"`
	}

	if err := json.Unmarshal(body, &apiErr); err != nil {
		return &types.APIError{
			Code:       "UNKNOWN",
			Message:    string(body),
			StatusCode: statusCode,
		}
	}

	return &types.APIError{
		Code:       apiErr.Code,
		Message:    apiErr.Error,
		StatusCode: statusCode,
	}
}
