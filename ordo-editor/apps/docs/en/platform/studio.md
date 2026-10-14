# Studio

Studio is the platform's editor, in the browser. It shows the same ruleset in three views, and a change in one shows up in the others right away.

![A loan approval ruleset in the Studio flow view](/screens/en/flow.webp)

## Views

- Flow: a canvas of steps and connections. Decision, action, terminal and sub-rule steps are nodes.
- Form: each step as a card, with branches and sub-rules nested inside. Easier if you don't think in graphs.
- Table: the ruleset as rows of conditions and outcomes, one row per path. It is available when the branches can be laid out as a table, and Studio tells you when they can't.

![The same ruleset in the table view](/screens/en/table.webp)

To edit the JSON directly, work with the files through the [CLI](./cli).

## Try a run

Paste an input and click Run. Studio runs the draft on the engine and shows the path taken, which branch matched at each step, sub-rule calls, the result and the time per step. The path is also drawn on the flow view.

## Tests

Each ruleset has test cases: an input and the expected code and output. Run one or all of them. Passing cases are green, failing ones are red with a diff, and cases not run yet are gray.

![Test cases in Studio with expected and actual results](/screens/en/tests.webp)

The cases are the same `tests/<name>.json` files the CLI uses, so `ordo pull` brings them into your repository and `ordo test` runs them in CI. See [Testing and Impact](/en/guide/testing).

## Languages

Studio is in English, Simplified Chinese and Traditional Chinese. Switch at the bottom of the sidebar.
