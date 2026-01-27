package batch

import (
	"context"
	"encoding/json"
	"fmt"
	"sync"
)

// Executor handles batch execution logic
type Executor struct {
	concurrency int
}

// NewExecutor creates a new batch executor
func NewExecutor(concurrency int) *Executor {
	if concurrency <= 0 {
		concurrency = 10 // Default concurrency
	}
	return &Executor{
		concurrency: concurrency,
	}
}

// Result represents a single execution result
type Result struct {
	Code       string
	Message    string
	Output     json.RawMessage
	DurationUs uint64
	Trace      any
	Error      *string
}

// BatchResult represents batch execution results
type BatchResult struct {
	Results []Result
	Summary Summary
}

// Summary represents batch execution summary
type Summary struct {
	Total           int
	Success         int
	Failed          int
	TotalDurationUs uint64
}

// ExecuteFunc is the function signature for single execution
type ExecuteFunc func(ctx context.Context, input any, includeTrace bool) (*Result, error)

// ExecuteParallel executes multiple inputs in parallel
func (e *Executor) ExecuteParallel(ctx context.Context, inputs []any, includeTrace bool, execFn ExecuteFunc) (*BatchResult, error) {
	total := len(inputs)
	results := make([]Result, total)

	// Use a semaphore to limit concurrency
	sem := make(chan struct{}, e.concurrency)
	var wg sync.WaitGroup
	var mu sync.Mutex

	successCount := 0
	failedCount := 0
	var totalDuration uint64

	for i, input := range inputs {
		wg.Add(1)
		go func(idx int, inp any) {
			defer wg.Done()

			// Acquire semaphore
			sem <- struct{}{}
			defer func() { <-sem }()

			// Execute single request
			result, err := execFn(ctx, inp, includeTrace)

			mu.Lock()
			defer mu.Unlock()

			if err != nil {
				errMsg := err.Error()
				results[idx] = Result{
					Error: &errMsg,
				}
				failedCount++
			} else {
				results[idx] = Result{
					Code:       result.Code,
					Message:    result.Message,
					Output:     result.Output,
					DurationUs: result.DurationUs,
					Trace:      result.Trace,
				}
				totalDuration += result.DurationUs
				successCount++
			}
		}(i, input)
	}

	wg.Wait()

	return &BatchResult{
		Results: results,
		Summary: Summary{
			Total:           total,
			Success:         successCount,
			Failed:          failedCount,
			TotalDurationUs: totalDuration,
		},
	}, nil
}

// MergeBatchResults merges multiple batch results (useful for chunked processing)
func MergeBatchResults(results ...*BatchResult) *BatchResult {
	if len(results) == 0 {
		return &BatchResult{
			Results: []Result{},
			Summary: Summary{},
		}
	}

	merged := &BatchResult{
		Results: []Result{},
		Summary: Summary{},
	}

	for _, r := range results {
		merged.Results = append(merged.Results, r.Results...)
		merged.Summary.Total += r.Summary.Total
		merged.Summary.Success += r.Summary.Success
		merged.Summary.Failed += r.Summary.Failed
		merged.Summary.TotalDurationUs += r.Summary.TotalDurationUs
	}

	return merged
}

// ConvertToAny converts a JSON-marshalable value to any
func ConvertToAny(v any) (any, error) {
	data, err := json.Marshal(v)
	if err != nil {
		return nil, fmt.Errorf("failed to marshal input: %w", err)
	}

	var result any
	if err := json.Unmarshal(data, &result); err != nil {
		return nil, fmt.Errorf("failed to unmarshal input: %w", err)
	}

	return result, nil
}
