# Ordo Engine Integration Guide

This guide explains how the Ordo Rust engine is integrated with the TypeScript Playground for rule execution.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                      Ordo Playground (Vue)                       │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │  Form/Flow Editor  →  Execute Button  →  Execution Modal │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────┬──────────────────────────────────────────────┘
                  │ RuleSet (TypeScript format)
                  │
                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Format Adapter (@ordo/core)                    │
│  • Converts: steps[] → steps: HashMap<id, Step>                 │
│  • Converts: startStepId → config.entry_step                    │
│  • Validates compatibility                                       │
└─────────────────┬──────────────────────────────────────────────┘
                  │ RuleSet (Engine format)
                  │
         ┌────────┴────────┐
         │                 │
         ▼                 ▼
┌──────────────────┐  ┌──────────────────┐
│  WASM (Local)    │  │  HTTP (Remote)   │
│  @ordo/wasm      │  │  ordo-server     │
│                  │  │                  │
│  • Fast          │  │  • Centralized   │
│  • No network    │  │  • Scalable      │
│  • Client-side   │  │  • Server logs   │
└────────┬─────────┘  └────────┬─────────┘
         │                     │
         └──────────┬──────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Rust Engine (ordo-core)                      │
│  • Step Flow Execution                                           │
│  • Expression Evaluation                                         │
│  • Execution Tracing                                             │
│  • Validation                                                    │
└─────────────────────────────────────────────────────────────────┘
```

## Components

### 1. WASM Bindings (`crates/ordo-wasm`)

Rust crate that compiles to WebAssembly, providing JavaScript bindings for the engine.

**Files:**
- `crates/ordo-wasm/src/lib.rs` - WASM interface implementation
- `crates/ordo-wasm/Cargo.toml` - Dependencies and build configuration
- `crates/ordo-wasm/build.sh` - Build script

**Key Functions:**
- `execute_ruleset()` - Execute a ruleset
- `validate_ruleset()` - Validate ruleset structure
- `eval_expression()` - Evaluate expressions

### 2. Format Adapter (`packages/core/src/engine/adapter.ts`)

Converts between TypeScript editor format and Rust engine format.

**Key Differences:**

| Editor Format | Engine Format |
|---------------|---------------|
| `startStepId: string` | `config.entry_step: string` |
| `steps: Step[]` | `steps: HashMap<string, Step>` |
| Array-based | Map-based |

### 3. Execution Client (`packages/core/src/engine/executor.ts`)

Dual-mode executor supporting both WASM and HTTP execution.

**Features:**
- Lazy WASM module loading
- Automatic format conversion
- Compatibility validation
- Error handling
- Timeout support

### 4. UI Components (`packages/vue/src/components/execution`)

Vue component for execution debugging.

**Features:**
- JSON input editor
- Execution mode selector (WASM/HTTP)
- Result visualization
- Trace visualization
- Error display

## Usage

### In Playground

1. Create or load a ruleset
2. Click "Execute" button in the status bar
3. Enter input data (JSON format)
4. Choose execution mode:
   - **Local (WASM)**: Fast, no network, runs in browser
   - **Remote (HTTP)**: Connects to ordo-server
5. Click "Execute" button
6. View results and execution trace

### Programmatic Usage

```typescript
import { RuleExecutor } from '@ordo/editor-core';
import type { RuleSet } from '@ordo/editor-core';

const executor = new RuleExecutor();

// Define ruleset
const ruleset: RuleSet = {
  config: { name: 'my-rule', version: '1.0.0' },
  startStepId: 'start',
  steps: [
    {
      id: 'start',
      name: 'Start Step',
      type: 'terminal',
      code: 'SUCCESS',
    },
  ],
};

// Execute (WASM by default)
const result = await executor.execute(
  ruleset,
  { user: 'test' },
  { includeTrace: true }
);

console.log(result.code);        // 'SUCCESS'
console.log(result.output);      // {}
console.log(result.duration_us); // 100
console.log(result.trace);       // Execution trace
```

### HTTP Mode

```typescript
const result = await executor.execute(
  ruleset,
  input,
  {
    mode: 'http',
    httpEndpoint: 'http://localhost:8080',
    includeTrace: true,
  }
);
```

## Building

### Build WASM Module

```bash
cd crates/ordo-wasm
./build.sh
```

This will:
1. Compile Rust to WebAssembly
2. Generate TypeScript bindings
3. Output to `ordo-editor/packages/wasm/dist/`

### Build TypeScript Packages

```bash
cd ordo-editor
pnpm install
pnpm build
```

## Testing

### Unit Tests

```bash
cd ordo-editor/packages/core
pnpm test
```

Tests are located in `src/engine/__tests__/`:
- `adapter.test.ts` - Format conversion tests
- `executor.test.ts` - Execution client tests

### Integration Tests

Start the ordo-server:

```bash
cd ../../
cargo run --bin ordo-server
```

Then test HTTP execution in Playground.

## Troubleshooting

### WASM Module Not Loading

**Problem**: "Failed to initialize WASM module"

**Solution**:
1. Ensure WASM module is built: `cd crates/ordo-wasm && ./build.sh`
2. Check that `packages/wasm/dist/` contains the WASM files
3. Check browser console for CORS errors

### Compatibility Errors

**Problem**: "Compatibility errors: Missing startStepId"

**Solution**: Ensure your RuleSet has:
- `startStepId` defined
- All steps have unique `id` fields
- All step references point to existing steps

### HTTP Execution Fails

**Problem**: "HTTP execution failed: 404"

**Solution**:
1. Ensure ordo-server is running: `cargo run --bin ordo-server`
2. Check the HTTP endpoint URL
3. Enable CORS in ordo-server if needed

## Performance

### WASM vs HTTP

| Metric | WASM | HTTP |
|--------|------|------|
| Latency | ~1-10ms | ~50-200ms |
| Throughput | High | Medium |
| Network | No | Yes |
| Scalability | Per-client | Centralized |

**Recommendation**: Use WASM for:
- Development and testing
- Single-user scenarios
- Offline usage

Use HTTP for:
- Production deployments
- Multi-user scenarios
- Centralized logging
- Resource pooling

## Future Enhancements

- [ ] Add expression syntax highlighting in input editor
- [ ] Add history/recent executions
- [ ] Add performance profiling visualization
- [ ] Add step-by-step debugging
- [ ] Add breakpoint support
- [ ] Export execution results
- [ ] Add execution comparison tool

## References

- [Ordo Core Documentation](../../crates/ordo-core/README.md)
- [WASM Package](packages/wasm/README.md)
- [Vue Components](packages/vue/README.md)

