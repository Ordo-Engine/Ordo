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
Only `first` hit policy is currently supported for bidirectional conversion with the Flow diagram. Rule files can also contain [decision table steps](#decision-table-steps-in-rule-files) that the engine evaluates directly, including `collect`.
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

## Decision Table Steps in Rule Files

Rule files (JSON or YAML) can contain a decision table directly, as a step of type `decision_table`. The engine evaluates it natively; no editor is needed.

```json
{
  "id": "discount",
  "name": "Discount",
  "type": "decision_table",
  "hit_policy": "first",
  "inputs": ["customer.tier", "order.amount"],
  "outputs": ["discount", "reason"],
  "rules": [
    { "when": ["gold", ">= 1000"], "then": [0.15, "Gold, large order"] },
    { "when": [["silver", "bronze"], "100..999"], "then": [0.05, "= customer.tier + \" tier\""] }
  ],
  "default": [0, "No discount"],
  "next_step": "price"
}
```

`inputs` are expressions, one per column. `outputs` are variable names: after the step runs, later steps read them as `$discount` and `$reason`.

### Input Cells

| Cell                              | Matches when                                      |
| --------------------------------- | ------------------------------------------------- |
| `"*"`, `"-"`, `""` or `null`      | always                                            |
| `"gold"`, `42`, `true`            | the input equals the value                        |
| `["silver", "bronze"]`            | the input is one of the values                    |
| `">= 1000"`, `"< 18"`, `"!= \"x\""` | the comparison holds (right side is an expression) |
| `"in [...]"`, `"not in [...]"`    | membership                                        |
| `"18..65"`                        | inclusive numeric range                           |
| `"= <expression>"`                | the boolean expression is true (any fields)       |

A plain string is compared literally. To compare with a string that starts with an operator or with `in `, write `"== \"in stock\""`.

### Output Cells

A cell is a literal value (`0.15`, `"Gold"`, `null`, an array or an object), or `"= <expression>"` to compute it, e.g. `"= order.amount * 0.1"`.

### Hit Policies

| `hit_policy`      | Result                                                                                                                                                                  |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `first` (default) | The first matching row sets the outputs. With no match, `default` is used; without a `default` the execution fails instead of continuing with missing values.           |
| `collect`         | Every matching row, in order. Each output becomes an array, or one value when `aggregate` is `sum`, `count`, `min` or `max` (e.g. adding up risk scores from all rules). |

With `collect` and no matching row, outputs are `[]`, `sum` is `0`, `count` is `0`, and `min`/`max` are `null`.

Decision table steps cannot be compiled to the binary `.ordo` format yet.
