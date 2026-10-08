# Studio Editor

Studio is Ordo Platform's visual editor, running in the browser. It offers three views of the same ruleset, kept in sync in real time.

![A loan approval ruleset in the Studio flow view](/screens/en/flow.webp)

## Three Authoring Modes

### Flow

- Blueprint-style canvas built on Vue Flow.
- Node types: Decision (branching), Action (assignments + external calls), Terminal (output), SubRule (sub-rule call).
- Pin shapes: triangles for execution flow, circles for data flow.
- Multi-incoming: multiple upstream nodes can land on the same target input pin; duplicates are auto-deduped.
- Compatible target pins highlight while dragging; trace replay colorizes nodes.

### Form

- Tree-shaped editor for users uncomfortable with flow-graph thinking.
- Each step is its own card; sub-rules and decision branches nest naturally.

### Decision table

- The same ruleset as rows of conditions and outcomes, one row per path through the rules.
- Shows the hit policy, and lets you add input and output columns by hand or import them from the schema.
- Available when the ruleset's branches can be laid out as a table. Studio tells you when they can't.

![The same ruleset in the decision table view](/screens/en/table.webp)

To edit the RuleSet JSON directly, work with the files through the [CLI](./cli).

Any change in one mode shows up in the other views immediately and is pushed onto the undo stack.

## Trace Panel

Before releasing, paste a JSON context into Studio and click **Try run**. The platform calls ordo-server's trace API and shows, for each step:

- Input/output snapshots
- Which branch matched
- Expression evaluation steps
- Sub-rule call stack
- Total + per-step timing

Trace results overlay each flow node via [ExecutionAnnotation](https://github.com/Ordo-Engine/Ordo) tooltips.

## Test Integration

In Studio you can edit each test case and run it on its own or as part of a batch. Results are color-coded:

- Green: actual matches expected
- Red: mismatch, with an inline diff
- Gray: not run yet

![Test cases in Studio with expected and actual results](/screens/en/tests.webp)

See [Test Management](./testing).

## Templates & Sub-Rules

- Templates: clone a complete project (rules + contracts + facts + tests) from Marketplace or the built-in template library in one click.
- Sub-Rule assets: extract common snippets (KYC, risk scoring) at the project level and reuse them across rulesets. An update applies to every ruleset that uses the asset.

## i18n

Studio and the docs are translated into English, Simplified Chinese, and Traditional Chinese. Switch language from the bottom of the sidebar.
