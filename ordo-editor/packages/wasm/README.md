# @ordo-engine/wasm

WebAssembly bindings for Ordo Rule Engine.

## Building

### Prerequisites

- Rust toolchain (1.75+)
- wasm-pack: `cargo install wasm-pack`

### Build Command

```bash
npm run build
```

This will compile the Rust code to WebAssembly and generate TypeScript bindings in the `dist/` directory.

## Usage

```typescript
import init, { execute_ruleset, validate_ruleset, eval_expression } from '@ordo-engine/wasm';

// Initialize WASM module
await init();

// Execute a ruleset
const result = execute_ruleset(
  rulesetJson, // RuleSet as JSON string
  inputJson, // Input data as JSON string
  true // Include trace
);

const executionResult = JSON.parse(result);
console.log(executionResult.code, executionResult.output);
```

## API

### `execute_ruleset(ruleset_json: string, input_json: string, include_trace: boolean): string`

Execute a ruleset with given input data.

**Returns**: JSON string containing execution result

### `validate_ruleset(ruleset_json: string): string`

Validate a ruleset definition.

**Returns**: JSON string with validation result: `{"valid": true}` or `{"valid": false, "errors": [...]}`

### `eval_expression(expression: string, context_json: string): string`

Evaluate an expression with given context.

**Returns**: JSON string with result and parsed expression
