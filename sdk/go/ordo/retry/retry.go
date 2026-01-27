package retry

import (
	"context"
	"errors"
	"fmt"
	"net"
	"strings"
	"syscall"
	"time"

	"github.com/pama-lee/ordo-go/ordo/types"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
)

// Config defines retry configuration
type Config struct {
	MaxAttempts     int
	InitialInterval time.Duration
	MaxInterval     time.Duration
	Jitter          bool
}

// DefaultConfig returns default retry configuration
func DefaultConfig() Config {
	return Config{
		MaxAttempts:     3,
		InitialInterval: 100 * time.Millisecond,
		MaxInterval:     5 * time.Second,
		Jitter:          true,
	}
}

// Retrier handles retry logic
type Retrier struct {
	config  Config
	backoff Backoff
}

// NewRetrier creates a new retrier
func NewRetrier(config Config) *Retrier {
	return &Retrier{
		config:  config,
		backoff: NewExponentialBackoff(config.InitialInterval, config.MaxInterval, config.Jitter),
	}
}

// Do executes a function with retry logic
func (r *Retrier) Do(ctx context.Context, fn func() error) error {
	var lastErr error
	r.backoff.Reset()

	for attempt := 0; attempt < r.config.MaxAttempts; attempt++ {
		// Execute the function
		err := fn()
		if err == nil {
			return nil
		}

		lastErr = err

		// Check if error is retryable
		if !isRetryable(err) {
			return err
		}

		// Check if we should retry
		if attempt >= r.config.MaxAttempts-1 {
			break
		}

		// Calculate backoff duration
		backoffDuration := r.backoff.Next()

		// Wait with context cancellation support
		select {
		case <-ctx.Done():
			return fmt.Errorf("retry cancelled: %w", ctx.Err())
		case <-time.After(backoffDuration):
			// Continue to next attempt
		}
	}

	return fmt.Errorf("max retry attempts (%d) exceeded: %w", r.config.MaxAttempts, lastErr)
}

// isRetryable checks if an error is retryable
func isRetryable(err error) bool {
	if err == nil {
		return false
	}

	// Check for API errors with specific status codes
	var apiErr *types.APIError
	if errors.As(err, &apiErr) {
		// Retry on 5xx server errors and rate limiting
		return apiErr.StatusCode >= 500 || apiErr.StatusCode == 429
	}

	// Check for gRPC status errors
	if s, ok := status.FromError(err); ok {
		switch s.Code() {
		case codes.Unavailable, codes.DeadlineExceeded, codes.ResourceExhausted, codes.Aborted:
			return true
		}
		return false
	}

	// Check for network errors
	if isNetworkError(err) {
		return true
	}

	// Check for timeout errors
	if errors.Is(err, context.DeadlineExceeded) {
		return true
	}

	// Check for connection errors
	errStr := err.Error()
	if strings.Contains(errStr, "connection refused") ||
		strings.Contains(errStr, "connection reset") ||
		strings.Contains(errStr, "broken pipe") ||
		strings.Contains(errStr, "no such host") {
		return true
	}

	return false
}

// isNetworkError checks if an error is a network-related error
func isNetworkError(err error) bool {
	var netErr net.Error
	if errors.As(err, &netErr) {
		return netErr.Timeout() || netErr.Temporary()
	}

	// Check for syscall errors
	var opErr *net.OpError
	if errors.As(err, &opErr) {
		var syscallErr *syscall.Errno
		if errors.As(opErr.Err, &syscallErr) {
			switch *syscallErr {
			case syscall.ECONNREFUSED, syscall.ECONNRESET, syscall.ETIMEDOUT:
				return true
			}
		}
		return true
	}

	return false
}
