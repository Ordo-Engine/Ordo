---
name: ordo-editor-component
description: Ordo visual editor component development guide. Includes Vue/React component usage, flow diagram integration, WASM calls, rule import/export. Use for frontend integration of rule editor, custom editor UI, extending editor functionality.
---

# Ordo Editor Component Development

## Installation

```bash
# Vue 3
npm install @ordo-engine/editor-vue

# React
npm install @ordo-engine/editor-react

# Core library (framework-agnostic)
npm install @ordo-engine/editor-core

# WASM module
npm install @ordo-engine/wasm
```

## Vue 3 Integration

### Basic Usage

```vue
<script setup lang="ts">
import { ref } from 'vue'
import { OrdoEditor, type RuleSet } from '@ordo-engine/editor-vue'
import '@ordo-engine/editor-vue/style.css'

const ruleset = ref<RuleSet>({
  config: {
    name: 'my-rule',
    version: '1.0.0',
    entry_step: 'start'
  },
  steps: {}
})

const handleChange = (newRuleset: RuleSet) => {
  ruleset.value = newRuleset
}
</script>

<template>
  <OrdoEditor
    v-model="ruleset"
    @change="handleChange"
    :readonly="false"
  />
</template>
```

### Flow Editor

```vue
<script setup lang="ts">
import { FlowEditor } from '@ordo-engine/editor-vue'

const onNodeClick = (nodeId: string) => {
  console.log('Clicked:', nodeId)
}
</script>

<template>
  <FlowEditor
    v-model="ruleset"
    @node-click="onNodeClick"
    :show-minimap="true"
    :show-controls="true"
  />
</template>
```

### Form Editor

```vue
<script setup lang="ts">
import { FormEditor, StepEditor } from '@ordo-engine/editor-vue'
</script>

<template>
  <FormEditor v-model="ruleset" />
  
  <!-- Or standalone step editor -->
  <StepEditor
    v-model="currentStep"
    :step-id="selectedStepId"
    @update="handleStepUpdate"
  />
</template>
```

### Execution Panel

```vue
<script setup lang="ts">
import { ExecutionPanel } from '@ordo-engine/editor-vue'

const input = ref({ user: { vip: true } })
</script>

<template>
  <ExecutionPanel
    :ruleset="ruleset"
    v-model:input="input"
    :show-trace="true"
  />
</template>
```

## React Integration

### Basic Usage

```tsx
import { useState } from 'react'
import { OrdoEditor, RuleSet } from '@ordo-engine/editor-react'
import '@ordo-engine/editor-react/style.css'

function App() {
  const [ruleset, setRuleset] = useState<RuleSet>({
    config: {
      name: 'my-rule',
      version: '1.0.0',
      entry_step: 'start'
    },
    steps: {}
  })

  return (
    <OrdoEditor
      value={ruleset}
      onChange={setRuleset}
      readonly={false}
    />
  )
}
```

## Core Library API

### Rule Operations

```typescript
import { 
  createRuleSet,
  addStep,
  removeStep,
  validateRuleSet,
  serializeRuleSet,
  deserializeRuleSet
} from '@ordo-engine/editor-core'

// Create ruleset
const ruleset = createRuleSet('my-rule', 'start')

// Add step
addStep(ruleset, {
  id: 'start',
  name: 'Check User',
  type: 'decision',
  branches: [
    { condition: 'user.vip == true', next_step: 'vip' }
  ],
  default_next: 'normal'
})

// Validate
const errors = validateRuleSet(ruleset)

// Serialize
const json = serializeRuleSet(ruleset, 'json')
const yaml = serializeRuleSet(ruleset, 'yaml')
```

### WASM Execution

```typescript
import { OrdoEngine } from '@ordo-engine/wasm'

// Initialize engine
const engine = await OrdoEngine.init()

// Load rule
engine.loadRuleSet(ruleset)

// Execute
const result = engine.execute('my-rule', {
  user: { vip: true, balance: 1000 }
})

console.log(result.code)    // "VIP"
console.log(result.outputs) // { discount: 0.2 }
console.log(result.trace)   // Execution trace
```

### Expression Evaluation

```typescript
import { OrdoEngine } from '@ordo-engine/wasm'

const engine = await OrdoEngine.init()

// Evaluate expression
const result = engine.eval('age >= 18 && status == "active"', {
  age: 25,
  status: 'active'
})
// result: true
```

## File Import/Export

### Export .ordo File

```typescript
import { exportOrdo, exportJSON, exportYAML } from '@ordo-engine/editor-core'

// Export as .ordo binary
const ordoBlob = await exportOrdo(ruleset)
downloadFile(ordoBlob, `${ruleset.config.name}.ordo`)

// Export as JSON
const jsonStr = exportJSON(ruleset)

// Export as YAML
const yamlStr = exportYAML(ruleset)
```

### Import File

```typescript
import { importOrdo, importJSON, importYAML } from '@ordo-engine/editor-core'

// Import .ordo file
const file = await selectFile()
if (file.name.endsWith('.ordo')) {
  const ruleset = await importOrdo(file)
} else if (file.name.endsWith('.json')) {
  const ruleset = await importJSON(await file.text())
} else if (file.name.endsWith('.yaml') || file.name.endsWith('.yml')) {
  const ruleset = await importYAML(await file.text())
}
```

## Component Structure

```
packages/
├── core/                    # @ordo-engine/editor-core
│   ├── src/
│   │   ├── engine/          # Rule engine adapter
│   │   ├── serialization/   # Serialization utilities
│   │   └── validation/      # Validators
│   └── package.json
│
├── vue/                     # @ordo-engine/editor-vue
│   ├── src/components/
│   │   ├── flow/            # Flow diagram components
│   │   ├── form/            # Form components
│   │   ├── step/            # Step editors
│   │   ├── execution/       # Execution panel
│   │   └── debug/           # Debug tools
│   └── package.json
│
├── react/                   # @ordo-engine/editor-react
│   └── src/
│
└── wasm/                    # @ordo-engine/wasm
    ├── index.js             # WASM wrapper
    └── index.d.ts           # Type definitions
```

## Custom Theming

```css
/* Override CSS variables */
:root {
  --ordo-primary-color: #1890ff;
  --ordo-success-color: #52c41a;
  --ordo-warning-color: #faad14;
  --ordo-error-color: #ff4d4f;
  --ordo-border-radius: 4px;
  --ordo-font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto;
}
```

## Internationalization

```typescript
import { setLocale } from '@ordo-engine/editor-vue'

// Set language
setLocale('zh-CN')  // Chinese
setLocale('en-US')  // English
```

## Local Development

```bash
cd ordo-editor

# Install dependencies
pnpm install

# Start playground
pnpm dev

# Build
pnpm build

# Run tests
pnpm test
```

## Key Files

- `ordo-editor/packages/core/` - Core library
- `ordo-editor/packages/vue/src/components/` - Vue components
- `ordo-editor/packages/react/src/` - React components
- `ordo-editor/packages/wasm/` - WASM bindings
- `ordo-editor/apps/playground/` - Demo application
- `crates/ordo-wasm/` - WASM source code
