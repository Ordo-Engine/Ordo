# WebAssembly API

Ordo 可以通过 WebAssembly 直接在浏览器中运行，实现与服务器相同的性能的客户端规则执行。

## 安装

### NPM 包

```bash
npm install @ordo-engine/wasm
# or
pnpm add @ordo-engine/wasm
```

### CDN

```html
<script type="module">
  import init, {
    execute,
    validate,
    parse_expression,
  } from 'https://unpkg.com/@ordo-engine/wasm/ordo_wasm.js';

  await init();
</script>
```

## API 参考

### initialize()

初始化 WASM 模块。必须在使用其他函数之前调用。

```typescript
import init from '@ordo-engine/wasm';

await init();
```

### execute(ruleset, input)

使用输入数据执行规则。

```typescript
import { execute } from '@ordo-engine/wasm';

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

执行并带回执行追踪以进行调试。

```typescript
import { execute_with_trace } from '@ordo-engine/wasm';

const result = execute_with_trace(JSON.stringify(ruleset), JSON.stringify(input));

const parsed = JSON.parse(result);
console.log(parsed.trace);
// { path: "check_vip -> vip", steps: [...] }
```

### validate(ruleset)

验证规则定义而不执行。

```typescript
import { validate } from '@ordo-engine/wasm';

const result = validate(JSON.stringify(ruleset));
const validation = JSON.parse(result);

if (validation.valid) {
  console.log('Rule is valid');
} else {
  console.log('Errors:', validation.errors);
}
```

### parse_expression(expression)

解析并验证表达式。

```typescript
import { parse_expression } from '@ordo-engine/wasm';

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

使用上下文评估表达式。

```typescript
import { eval_expression } from '@ordo-engine/wasm';

const result = eval_expression('user.age >= 18', JSON.stringify({ user: { age: 25 } }));

console.log(JSON.parse(result)); // true
```

## Vue 集成

### 使用 @ordo-engine/editor-vue

```vue
<script setup lang="ts">
import { ref, onMounted } from 'vue';
import init, { execute } from '@ordo-engine/wasm';

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

### Vite 配置

```typescript
// vite.config.ts
export default defineConfig({
  optimizeDeps: {
    exclude: ['@ordo-engine/wasm'],
  },
  build: {
    target: 'esnext',
  },
});
```

## React 集成

```tsx
import { useState, useEffect } from 'react';
import init, { execute } from '@ordo-engine/wasm';

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

## 性能

WASM 执行几乎与原生 Rust 一样快：

| 环境           | 执行时间 |
| -------------- | -------- |
| Native (Rust)  | 1.63 µs  |
| WASM (Chrome)  | 2-3 µs   |
| WASM (Firefox) | 2-4 µs   |
| WASM (Safari)  | 3-5 µs   |

## 浏览器支持

| 浏览器  | 版本 | 状态        |
| ------- | ---- | ----------- |
| Chrome  | 57+  | ✅ 完全支持 |
| Firefox | 52+  | ✅ 完全支持 |
| Safari  | 11+  | ✅ 完全支持 |
| Edge    | 16+  | ✅ 完全支持 |

## 错误处理

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

## 内存管理

WASM 内存是自动管理的。对于大规模使用：

```typescript
// 分批处理以避免内存压力
const results = [];
for (const input of inputs) {
  results.push(execute(rulesetJson, JSON.stringify(input)));

  // 可选：定期让出事件循环
  if (results.length % 1000 === 0) {
    await new Promise((r) => setTimeout(r, 0));
  }
}
```

## 从源码构建

```bash
cd crates/ordo-wasm

# 安装 wasm-pack
cargo install wasm-pack

# 构建 web 目标
wasm-pack build --target web --out-dir ../../ordo-editor/packages/wasm/dist
```
