# Studio Editor

Studio is Ordo Platform's visual editor — a browser-native app. It offers three equivalent views over the same ruleset, kept in sync in real time.

## Three Authoring Modes

### Flow

- Blueprint-style canvas built on Vue Flow.
- Node types: **Decision** (branching), **Action** (assignments + external calls), **Terminal** (output), **SubRule** (sub-rule call).
- Pin shapes: triangles for execution flow, circles for data flow.
- Multi-incoming: multiple upstream nodes can land on the same target input pin; duplicates are auto-deduped.
- Compatible target pins highlight while dragging; trace replay colorizes nodes.

### Form

- Tree-shaped editor for users uncomfortable with flow-graph thinking.
- Each step is its own card; sub-rules and decision branches nest naturally.

### JSON

- Direct edit of the RuleSet JSON.
- Schema validation and expression highlighting built in.
- Useful for Git/CI imports or bulk find-and-replace.

> All three modes synchronize through a shared [editor-store](/en/guide/editor-store) (Pinia / framework-agnostic). Any change is reflected in the other views immediately and pushed onto the undo stack.

## Trace Panel

Before releasing, paste a JSON context into Studio and click "Try run" → the platform calls ordo-server's trace API → for each step you get:

- Input/output snapshots
- Which branch matched
- Expression evaluation steps
- Sub-rule call stack
- Total + per-step timing

Trace results overlay each flow node via [ExecutionAnnotation](https://github.com/Ordo-Engine/Ordo) tooltips.

## Test Integration

Each test case can be edited, run individually, or run as a batch — all from inside Studio. Color-coded results:

- Green: actual matches expected
- Red: mismatch, with an inline diff
- Gray: not run yet

See [Test Management](./testing).

## Templates & Sub-Rules

- **Templates** — clone a complete project (rules + contracts + facts + tests) from Marketplace or the built-in template library in one click.
- **Sub-Rule assets** — extract common snippets (KYC, risk scoring) at the project level and reuse them across rulesets; updating once propagates everywhere.

## i18n

Studio and the docs are fully translated into English, Simplified Chinese, and Traditional Chinese. Switch language from the top-right corner.
