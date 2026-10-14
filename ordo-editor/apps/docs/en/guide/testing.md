# Testing and Impact

Three commands cover the work around a rule change:

- `ordo test` checks that the cases you wrote still pass.
- `ordo trace` shows the path one input takes.
- `ordo impact` lists every decision an edit changes, including ones no test covers.

All three run locally on the embedded engine, with no server and no network. They need a project made with `ordo init` (see [Quick Start](./quick-start)).

## Tests

Tests for `rulesets/<name>.json` live in `tests/<name>.json`:

```json
[
  {
    "name": "gold member, large order",
    "input": { "user": { "tier": "gold" }, "order": { "amount": 1200 } },
    "expect": { "code": "OK", "output": { "rate": 0.15, "pay": 1020.0 } }
  }
]
```

```bash
ordo test             # every ruleset that has tests
ordo test discount    # one ruleset
```

For a single rule file outside a project: `ordo test --rule rule.json --tests cases.json`.

## Trace

```text
$ ordo trace discount --input '{"user":{"tier":"gold"},"order":{"amount":1200}}'
code:    OK
output:  {
  "rate": 0.15,
  "pay": 1020.0
}

path:    pick_rate -> done
```

Use it when a decision is not what you expected.

## Impact

`ordo impact` runs the last committed version of a ruleset (git `HEAD`) and your edited version side by side, and lists every input whose result changed. It uses three kinds of input:

- the test cases,
- real inputs passed with `--inputs` (JSONL),
- boundary probes: for every comparison such as `amount >= 1000`, and every decision table cell, a value just below, at and just above the threshold.

So a change shows up even when no test sits near it. Lowering the gold threshold in the quick start rule from 1000 to 800:

```text
$ ordo impact discount
impact discount  (HEAD → working tree)
  17 inputs: 2 tests, 0 captured, 15 boundary probes

  OK → OK  3 inputs

CHANGED probe:order.amount=999  {"order":{"amount":999},"user":{"tier":"gold"}}
    output.pay: 899.1 → 849.15
    output.rate: 0.1 → 0.15
...
3 changed · 14 unchanged
```

Both tests still pass, and impact still shows what moved.

| Option             | What it does                                     |
| ------------------ | ------------------------------------------------ |
| `--base <rev>`     | Compare against another commit instead of `HEAD` |
| `--base-file <f>`  | Compare against a file                           |
| `--inputs <f>`     | Add real inputs: a capture file, a decision log, or one JSON input per line |
| `--no-probes`      | Only tests and `--inputs`                        |
| `--fail-on-change` | Exit non-zero if anything changed, for CI        |

## Use real traffic

ordo-server can record every execution. Turn it on with a directory:

```bash
ordo-server --rules-dir ./rules --capture-io-path /var/ordo/capture
```

Each execution adds one line to `capture-YYYY-MM-DD.jsonl` with the input, code and output. `--capture-io-sample-rate` (0 to 100, default 100) limits how much is kept. Captured inputs are full requests and may hold personal data, so capture is off by default. Only single HTTP executions are captured, not batch or gRPC.

Then, in the project:

```bash
ordo impact discount --inputs capture-2026-10-14.jsonl   # how many real requests a change moves
ordo replay capture-2026-10-14.jsonl                     # rerun them on the current rules
ordo replay capture-2026-10-14.jsonl --write-tests       # keep them as test cases
```

`replay` reports each record as consistent, flipped (with a diff), errored, or for a ruleset not in the project. `--fail-on-flip` exits non-zero when a decision flipped. `--write-tests` merges the captured decisions into `tests/<name>.json`, so from then on `ordo test` fails if a change alters them.

`replay` reads any JSONL with `rule_name`, `input`, `code` and `output` on each line, so a log your own service writes works too.

For decisions made through the [decision model gateway](/en/platform/decision-gateway), the decision log works the same way with `--inputs`.

## In CI

The commands return proper exit codes:

```yaml
- run: npx @ordo-engine/cli validate
- run: npx @ordo-engine/cli test
- run: npx @ordo-engine/cli impact discount --fail-on-change
```

Every command takes `--json` for machine-readable output.
