# Decision Table

The Decision Table is a spreadsheet-like editing mode for defining rule logic, complementing the existing Flow diagram and Form views.

## Overview

Decision tables provide a compact, tabular representation of business rules. Each row represents a rule with input conditions and output values, making it easy to review and maintain large sets of rules at a glance.

### Three Editing Modes

The visual editor supports three editing modes that can be switched seamlessly:

- **Form** — Traditional form-based editing with fields and dropdowns
- **Flow** — Visual flow diagram showing the step graph
- **Table** — Spreadsheet-like decision table (new)

Data is automatically synchronized when switching between modes.

## Table Structure

A decision table consists of:

| Component      | Description                                                 |
| -------------- | ----------------------------------------------------------- |
| Input Columns  | Schema field paths used as conditions (e.g., `user.age`)    |
| Output Columns | Result fields produced by the rule                          |
| Rows (Rules)   | Each row is one rule with priority, conditions, and outputs |
| Hit Policy     | How matching is resolved: `first`, `all`, or `collect`      |

### Cell Types

Each input cell can use one of five condition types:

| Type         | Example           | Description                  |
| ------------ | ----------------- | ---------------------------- |
| `exact`      | `"premium"`       | Exact value match            |
| `range`      | `[18, 65]`        | Numeric range (inclusive)    |
| `in`         | `["A", "B", "C"]` | Value in set                 |
| `any`        | `*`               | Matches any value (wildcard) |
| `expression` | `age > 18 && vip` | Free-form Ordo expression    |

## Usage

### Creating a Decision Table

1. Open or create a ruleset in the visual editor
2. Switch to **Table** mode using the toolbar icon or status bar
3. Use the toolbar to add input/output columns
4. Add rows and fill in conditions and output values

### Column Operations

- **Add Input Column** — Select a schema field path as a new input condition
- **Add Output Column** — Define a new output field
- **Import from Schema** — Bulk-import columns from the ruleset's schema definition
- **Remove Column** — Click the column header menu to remove

### Row Operations

- **Add Row** — Append a new rule row
- **Duplicate Row** — Copy an existing row
- **Delete Row** — Remove a row
- **Reorder** — Drag rows to change priority order

### Hit Policies

| Policy    | Behavior                                            |
| --------- | --------------------------------------------------- |
| `first`   | Returns the first matching row's output (default)   |
| `all`     | Returns outputs from all matching rows              |
| `collect` | Collects outputs from all matching rows into a list |

::: tip
Only `first` hit policy is currently supported for bidirectional conversion with the Flow diagram.
:::

## Conversion Between Modes

The decision table supports bidirectional conversion with the Step graph model:

- **Table → Flow**: `compileTableToSteps()` converts the table into a Decision step with branches pointing to Terminal steps
- **Flow → Table**: `decompileStepsToTable()` analyzes the step graph and extracts it into table form

This conversion is automatic when switching between Table and Flow modes.

### Conversion Constraints

The automatic decompilation works for rulesets that follow the pattern:

- A single Decision step as the entry point
- Each branch points to a Terminal step
- Conditions use standard comparison operators

Complex step graphs with multiple chained decisions or action steps will fall back to manual editing in Flow mode.

## Export

Use the **Export JSON** button in the toolbar to download the decision table as a standalone JSON file.

## Programmatic API

```typescript
import {
  type DecisionTable,
  createEmptyTable,
  createInputColumn,
  createOutputColumn,
  createEmptyRow,
  compileTableToSteps,
  decompileStepsToTable,
} from '@ordo-engine/editor-core';

// Create a table programmatically
const table = createEmptyTable();
table.inputColumns.push(createInputColumn('user.age', 'number'));
table.outputColumns.push(createOutputColumn('discount', 'number'));

// Convert to steps for execution
const steps = compileTableToSteps(table, 'my-rule');

// Convert steps back to table
const recovered = decompileStepsToTable(steps);
```
