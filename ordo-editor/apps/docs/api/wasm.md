# WebAssembly API

Ordo can run directly in the browser via WebAssembly, enabling client-side rule execution with the same performance as the server.

## Installation

### NPM Package

```bash
npm install @ordo/wasm
# or
pnpm add @ordo/wasm
```

### CDN

```html
<script type="module">
  import init, {
    execute,
    validate,
    parse_expression,
  } from 'https://unpkg.com/@ordo/wasm/ordo_wasm.js';

  await init();
</script>
```

## API Reference

### initialize()

Initialize the WASM module. Must be called before using other functions.

```typescript
import init from '@ordo/wasm';

await init();
```

### execute(ruleset, input)

Execute a rule with input data.

```typescript
import { execute } from '@ordo/wasm';

const ruleset = {
  config: {
    name: 'discount-check',
    version: '1.0.0',
    entry_step: 'check_vip',
  },
  steps: {
    check_vip: {
      id: 'check_vip',
      type: 'decision',
      branches: [{ condition: 'user.vip == true', next_step: 'vip' }],
      default_next: 'normal',
    },
    vip: {
      id: 'vip',
      type: 'terminal',
      result: { code: 'VIP', message: 'VIP discount' },
    },
    normal: {
      id: 'normal',
      type: 'terminal',
      result: { code: 'NORMAL', message: 'Normal discount' },
    },
  },
};

const input = {
  user: { vip: true },
};

const result = execute(JSON.stringify(ruleset), JSON.stringify(input));
console.log(JSON.parse(result));
// { code: "VIP", message: "VIP discount", output: {}, duration_us: 2 }
```

### execute_with_trace(ruleset, input)

Execute with execution trace for debugging.

```typescript
import { execute_with_trace } from '@ordo/wasm';

const result = execute_with_trace(JSON.stringify(ruleset), JSON.stringify(input));

const parsed = JSON.parse(result);
console.log(parsed.trace);
// { path: "check_vip -> vip", steps: [...] }
```

### validate(ruleset)

Validate a rule definition without executing.

```typescript
import { validate } from '@ordo/wasm';

const result = validate(JSON.stringify(ruleset));
const validation = JSON.parse(result);

if (validation.valid) {
  console.log('Rule is valid');
} else {
  console.log('Errors:', validation.errors);
}
```

### parse_expression(expression)

Parse and validate an expression.

```typescript
import { parse_expression } from '@ordo/wasm';

const result = parse_expression('user.age >= 18 && user.vip == true');
const parsed = JSON.parse(result);

if (parsed.valid) {
  console.log('Expression is valid');
  console.log('AST:', parsed.ast);
} else {
  console.log('Parse error:', parsed.error);
}
```

### eval_expression(expression, context)

Evaluate an expression with context.

```typescript
import { eval_expression } from '@ordo/wasm';

const result = eval_expression('user.age >= 18', JSON.stringify({ user: { age: 25 } }));

console.log(JSON.parse(result)); // true
```

## Vue Integration

### Using with @ordo/editor-vue

```vue
<script setup lang="ts">
import { ref, onMounted } from 'vue';
import init, { execute } from '@ordo/wasm';

const result = ref(null);
const loading = ref(true);

onMounted(async () => {
  await init();
  loading.value = false;
});

const runRule = async (ruleset, input) => {
  const response = execute(JSON.stringify(ruleset), JSON.stringify(input));
  result.value = JSON.parse(response);
};
</script>
```

### Vite Configuration

```typescript
// vite.config.ts
export default defineConfig({
  optimizeDeps: {
    exclude: ['@ordo/wasm'],
  },
  build: {
    target: 'esnext',
  },
});
```

## React Integration

```tsx
import { useState, useEffect } from 'react';
import init, { execute } from '@ordo/wasm';

function RuleExecutor({ ruleset, input }) {
  const [result, setResult] = useState(null);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    init().then(() => setReady(true));
  }, []);

  const run = () => {
    if (!ready) return;
    const response = execute(JSON.stringify(ruleset), JSON.stringify(input));
    setResult(JSON.parse(response));
  };

  return (
    <div>
      <button onClick={run} disabled={!ready}>
        Execute Rule
      </button>
      {result && <pre>{JSON.stringify(result, null, 2)}</pre>}
    </div>
  );
}
```

## Performance

WASM execution is nearly as fast as native Rust:

| Environment    | Execution Time |
| -------------- | -------------- |
| Native (Rust)  | 1.63 µs        |
| WASM (Chrome)  | 2-3 µs         |
| WASM (Firefox) | 2-4 µs         |
| WASM (Safari)  | 3-5 µs         |

## Browser Support

| Browser | Version | Status          |
| ------- | ------- | --------------- |
| Chrome  | 57+     | ✅ Full support |
| Firefox | 52+     | ✅ Full support |
| Safari  | 11+     | ✅ Full support |
| Edge    | 16+     | ✅ Full support |

## Error Handling

```typescript
try {
  const result = execute(rulesetJson, inputJson);
  return JSON.parse(result);
} catch (error) {
  if (error instanceof Error) {
    console.error('Execution error:', error.message);
  }
}
```

## Memory Management

WASM memory is automatically managed. For large-scale usage:

```typescript
// Process in batches to avoid memory pressure
const results = [];
for (const input of inputs) {
  results.push(execute(rulesetJson, JSON.stringify(input)));

  // Optional: yield to event loop periodically
  if (results.length % 1000 === 0) {
    await new Promise((r) => setTimeout(r, 0));
  }
}
```

## Building from Source

```bash
cd crates/ordo-wasm

# Install wasm-pack
cargo install wasm-pack

# Build for web
wasm-pack build --target web --out-dir ../../ordo-editor/packages/wasm/dist
```
