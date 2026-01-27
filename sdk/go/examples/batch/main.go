package main

import (
	"context"
	"fmt"
	"log"

	"github.com/pama-lee/ordo-go/ordo"
)

func main() {
	// Create Ordo client
	client, err := ordo.NewClient(
		ordo.WithHTTPAddress("http://localhost:8080"),
		ordo.WithGRPCAddress("localhost:50051"),
		ordo.WithBatchConcurrency(5),
	)
	if err != nil {
		log.Fatalf("Failed to create client: %v", err)
	}
	defer client.Close()

	ctx := context.Background()

	// Prepare multiple inputs for batch execution
	inputs := []any{
		map[string]any{
			"user": map[string]any{"vip": true, "age": 25},
			"order": map[string]any{"total": 150.0},
		},
		map[string]any{
			"user": map[string]any{"vip": false, "age": 30},
			"order": map[string]any{"total": 80.0},
		},
		map[string]any{
			"user": map[string]any{"vip": true, "age": 22},
			"order": map[string]any{"total": 200.0},
		},
		map[string]any{
			"user": map[string]any{"vip": false, "age": 18},
			"order": map[string]any{"total": 50.0},
		},
		map[string]any{
			"user": map[string]any{"vip": true, "age": 35},
			"order": map[string]any{"total": 300.0},
		},
	}

	fmt.Println("=== Batch Execution ===")
	fmt.Printf("Processing %d inputs...\n\n", len(inputs))

	// Execute batch
	result, err := client.ExecuteBatch(
		ctx,
		"discount-check",
		inputs,
		ordo.WithParallel(true),
		ordo.WithConcurrency(5),
	)
	if err != nil {
		log.Fatalf("Batch execution failed: %v", err)
	}

	// Display summary
	fmt.Println("Summary:")
	fmt.Printf("  Total: %d\n", result.Summary.Total)
	fmt.Printf("  Success: %d\n", result.Summary.Success)
	fmt.Printf("  Failed: %d\n", result.Summary.Failed)
	fmt.Printf("  Total Duration: %d µs\n", result.Summary.TotalDurationUs)
	fmt.Printf("  Average Duration: %d µs\n",
		result.Summary.TotalDurationUs/uint64(result.Summary.Success))

	// Display individual results
	fmt.Println("\nResults:")
	for i, item := range result.Results {
		if item.Error != nil {
			fmt.Printf("[%d] ✗ Error: %s\n", i+1, *item.Error)
		} else {
			fmt.Printf("[%d] ✓ Code: %s, Message: %s, Duration: %d µs\n",
				i+1, item.Code, item.Message, item.DurationUs)
		}
	}

	fmt.Println("\n✓ Batch execution completed!")
}
