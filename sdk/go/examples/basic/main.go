package main

import (
	"context"
	"encoding/json"
	"fmt"
	"log"

	"github.com/pama-lee/ordo-go/ordo"
)

func main() {
	// Create Ordo client with default configuration
	client, err := ordo.NewClient(
		ordo.WithHTTPAddress("http://localhost:8080"),
		ordo.WithGRPCAddress("localhost:50051"),
	)
	if err != nil {
		log.Fatalf("Failed to create client: %v", err)
	}
	defer client.Close()

	ctx := context.Background()

	// Example 1: Health check
	fmt.Println("=== Health Check ===")
	health, err := client.Health(ctx)
	if err != nil {
		log.Fatalf("Health check failed: %v", err)
	}
	fmt.Printf("Status: %s, Version: %s, Uptime: %ds\n",
		health.Status, health.Version, health.UptimeSeconds)

	// Example 2: Execute a rule
	fmt.Println("\n=== Execute Rule ===")
	input := map[string]any{
		"user": map[string]any{
			"vip": true,
			"age": 25,
		},
		"order": map[string]any{
			"total": 150.0,
		},
	}

	result, err := client.Execute(ctx, "discount-check", input)
	if err != nil {
		log.Fatalf("Execute failed: %v", err)
	}

	fmt.Printf("Code: %s\n", result.Code)
	fmt.Printf("Message: %s\n", result.Message)
	fmt.Printf("Duration: %d µs\n", result.DurationUs)

	var output map[string]any
	if err := json.Unmarshal(result.Output, &output); err == nil {
		fmt.Printf("Output: %v\n", output)
	}

	// Example 3: Execute with trace
	fmt.Println("\n=== Execute with Trace ===")
	result, err = client.Execute(ctx, "discount-check", input, ordo.WithTrace(true))
	if err != nil {
		log.Fatalf("Execute with trace failed: %v", err)
	}

	if result.Trace != nil {
		fmt.Printf("Execution Path: %s\n", result.Trace.Path)
		fmt.Println("Steps:")
		for _, step := range result.Trace.Steps {
			fmt.Printf("  - %s (%s): %d µs\n", step.StepName, step.StepID, step.DurationUs)
		}
	}

	// Example 4: List rulesets
	fmt.Println("\n=== List RuleSets ===")
	rulesets, err := client.ListRuleSets(ctx)
	if err != nil {
		log.Fatalf("List rulesets failed: %v", err)
	}

	for _, rs := range rulesets {
		fmt.Printf("- %s (v%s): %d steps\n", rs.Name, rs.Version, rs.StepCount)
		if rs.Description != nil {
			fmt.Printf("  Description: %s\n", *rs.Description)
		}
	}

	// Example 5: Evaluate expression
	fmt.Println("\n=== Evaluate Expression ===")
	evalResult, err := client.Eval(ctx, "user.age >= 18 && user.vip == true", input)
	if err != nil {
		log.Fatalf("Eval failed: %v", err)
	}

	var evalOutput bool
	if err := json.Unmarshal(evalResult.Result, &evalOutput); err == nil {
		fmt.Printf("Result: %v\n", evalOutput)
	}

	fmt.Println("\n✓ All examples completed successfully!")
}
