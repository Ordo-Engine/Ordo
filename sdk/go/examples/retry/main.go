package main

import (
	"context"
	"fmt"
	"log"
	"time"

	"github.com/pama-lee/ordo-go/ordo"
	"github.com/pama-lee/ordo-go/ordo/retry"
)

func main() {
	// Create Ordo client with retry enabled
	client, err := ordo.NewClient(
		ordo.WithHTTPAddress("http://localhost:8080"),
		ordo.WithGRPCAddress("localhost:50051"),
		ordo.WithRetry(retry.Config{
			MaxAttempts:     5,
			InitialInterval: 100 * time.Millisecond,
			MaxInterval:     2 * time.Second,
			Jitter:          true,
		}),
	)
	if err != nil {
		log.Fatalf("Failed to create client: %v", err)
	}
	defer client.Close()

	ctx := context.Background()

	fmt.Println("=== Execute with Retry ===")
	fmt.Println("This example will retry automatically on transient failures")
	fmt.Println("(e.g., network errors, 5xx server errors, rate limiting)\n")

	input := map[string]any{
		"user": map[string]any{
			"vip": true,
			"age": 25,
		},
		"order": map[string]any{
			"total": 150.0,
		},
	}

	// Example 1: Normal execution with retry protection
	fmt.Println("Example 1: Normal execution")
	start := time.Now()
	result, err := client.Execute(ctx, "discount-check", input)
	elapsed := time.Since(start)

	if err != nil {
		log.Printf("Execute failed after retries: %v", err)
	} else {
		fmt.Printf("✓ Success: %s (took %v)\n", result.Code, elapsed)
		fmt.Printf("  Duration: %d µs\n", result.DurationUs)
	}

	// Example 2: Execute with timeout context
	fmt.Println("\nExample 2: Execution with timeout")
	timeoutCtx, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()

	start = time.Now()
	result, err = client.Execute(timeoutCtx, "discount-check", input)
	elapsed = time.Since(start)

	if err != nil {
		log.Printf("Execute failed: %v (took %v)", err, elapsed)
	} else {
		fmt.Printf("✓ Success: %s (took %v)\n", result.Code, elapsed)
	}

	// Example 3: Demonstrate retry behavior with invalid ruleset
	fmt.Println("\nExample 3: Non-retryable error (404 Not Found)")
	start = time.Now()
	_, err = client.Execute(ctx, "non-existent-rule", input)
	elapsed = time.Since(start)

	if err != nil {
		fmt.Printf("✓ Failed immediately (non-retryable): %v (took %v)\n", err, elapsed)
		fmt.Println("  Note: 404 errors are not retried")
	}

	// Example 4: Health check with retry
	fmt.Println("\nExample 4: Health check with retry")
	start = time.Now()
	health, err := client.Health(ctx)
	elapsed = time.Since(start)

	if err != nil {
		log.Printf("Health check failed after retries: %v", err)
	} else {
		fmt.Printf("✓ Server healthy: %s v%s (took %v)\n",
			health.Status, health.Version, elapsed)
	}

	fmt.Println("\n=== Retry Configuration ===")
	fmt.Println("Current retry settings:")
	fmt.Println("  - Max Attempts: 5")
	fmt.Println("  - Initial Interval: 100ms")
	fmt.Println("  - Max Interval: 2s")
	fmt.Println("  - Jitter: enabled")
	fmt.Println("\nRetryable errors:")
	fmt.Println("  - Network errors (connection refused, timeout, etc.)")
	fmt.Println("  - 5xx server errors")
	fmt.Println("  - 429 Too Many Requests")
	fmt.Println("  - gRPC: UNAVAILABLE, DEADLINE_EXCEEDED, RESOURCE_EXHAUSTED")

	fmt.Println("\n✓ Retry examples completed!")
}
