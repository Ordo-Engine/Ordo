# Editor Store & Undo/Redo

The Editor Store holds the state of the visual rule editor. Every edit is a command, which is how undo and redo work.

## Overview

Each open ruleset file gets its own `EditorStore` instance, so undo/redo history is kept per file.

### Key Features

- **Command Pattern**: all mutations go through commands, enabling undo/redo
- **Per-file History**: each file maintains its own undo/redo stack
- **Schema-aware Editing**: condition builders and value inputs adapt to the ruleset's schema
- **Keyboard Shortcuts**: `Cmd/Ctrl+Z` for undo, `Cmd/Ctrl+Shift+Z` for redo

## Commands

All rule changes are expressed as commands:

| Command               | Description                          |
| --------------------- | ------------------------------------ |
| `AddStep`             | Add a new step to the ruleset        |
| `RemoveStep`          | Remove a step                        |
| `UpdateStep`          | Modify step properties               |
| `MoveStep`            | Change step position                 |
| `AddBranch`           | Add a branch to a Decision step      |
| `RemoveBranch`        | Remove a branch                      |
| `UpdateBranch`        | Modify branch conditions or target   |
| `ReorderBranch`       | Change branch evaluation order       |
| `ConnectSteps`        | Create a connection between steps    |
| `DisconnectSteps`     | Remove a connection                  |
| `SetStartStep`        | Set the entry step                   |
| `UpdateConfig`        | Update ruleset configuration         |
| `SetSchema`           | Set or update the ruleset schema     |
| `Batch`               | Execute multiple commands atomically |
| `PasteStep`           | Paste a copied step                  |
| `ImportDecisionTable` | Import a decision table as steps     |

## Schema-Aware Editing

When a ruleset defines a schema, the editor uses it as follows:

### Smart Condition Builder

The `OrdoSmartConditionBuilder` component replaces the basic condition editor when a schema is available:

- **Field Picker**: browse and search schema fields by group, with type labels
- **Operator Selection**: operators are filtered by field type (e.g., numeric fields show `>`, `<`, `>=`, `<=`)
- **Type-Aware Values**: input widgets adapt to the field type:
  - String → text input
  - Number → numeric input
  - Boolean → toggle switch
  - Enum → dropdown selector
- **Mode Switching**: toggle between Simple, Group (AND/OR), and Expression modes

### Schema Field Picker

The `OrdoSchemaFieldPicker` provides:

- Grouped display by top-level objects
- Fuzzy search across all fields
- Keyboard navigation (arrow keys, Enter, Escape)
- Type badges for quick identification

### Enriched Suggestions

Action and Terminal editors automatically provide:

- Variable name autocompletion from schema fields
- Expression input suggestions combining schema fields and existing variables

## Vue Integration

Use the `useEditorStore` composable in Vue 3:

```vue
<script setup lang="ts">
import { useEditorStore } from '@ordo-engine/editor-vue';

const store = useEditorStore(fileId, initialRuleset);
const { state, dispatch, undo, redo, canUndo, canRedo, schemaContext } = store;
</script>
```

## Programmatic API

```typescript
import { EditorStore } from '@ordo-engine/editor-core';

const store = new EditorStore(initialRuleset, { maxHistory: 80 });

// Dispatch commands
store.dispatch({ type: 'AddStep', payload: { step: newStep } });

// Undo/Redo
store.undo();
store.redo();

// Access state
console.log(store.state.steps);
console.log(store.canUndo);
```
