# Traffic Capture & Replay

Record real decisions in production, then replay them against a rule change to
see which ones flip. An Ordo test case is `{input, expect:{code, output}}`, the
same shape as a captured `{input, code, output}` decision, so captured traffic
converts directly into regression tests.

The loop:

> change a rule → replay last week's real decisions → inspect the flips → fixate
> them as regression tests → ship.

## 1. Capture (ordo-server)

Capture is off by default. To enable it, point ordo-server at a directory:

```bash
ordo-server --rules-dir ./rules --capture-io-path /var/ordo/capture
```

Every rule execution then appends one JSON line to
`/var/ordo/capture/capture-YYYY-MM-DD.jsonl` (daily rotation):

```json
{"ts":"…","rule_name":"listing-risk","tenant":"lumate","input":{"amount":5000,"is_vip":true},"code":"REVIEW","output":{…},"duration_us":42,"source_ip":"…"}
```

Env vars: `ORDO_CAPTURE_IO_PATH`, `ORDO_CAPTURE_IO_SAMPLE_RATE` (0–100, default
100 = capture all when enabled).

::: warning Cost & privacy

- No overhead when disabled. The input is only cloned when capture is on and
  the request is sampled.
- Captured inputs are the full request payload and may contain PII. That is why
  capture is opt-in. Limit the volume (and exposure) with the sample rate, and
  treat the capture files as sensitive.
- v1 captures the HTTP execute path (single requests, not yet batch). gRPC and
  batch capture are follow-ups.
  :::

## 2. Replay (ordo CLI)

Pull the capture file to a machine that has your ruleset as a project, and replay:

```bash
ordo replay capture-2026-07-04.jsonl
```

Replay re-runs every captured `input` through the current project ruleset and
puts each record in a bucket:

| Bucket              | Meaning                                                        |
| ------------------- | -------------------------------------------------------------- |
| **consistent**      | same decision as captured                                      |
| **flipped**         | code or output changed vs. the captured baseline (with a diff) |
| **errored**         | execution failed                                               |
| **unknown-ruleset** | the record named a rule not in this project                    |
| **replayed**        | input-only capture (no baseline to compare)                    |

```text
FLIP listing-risk  {"amount":25000,…}  REVIEW → ALLOW
     expected code: "REVIEW", got: "ALLOW"

12,401 records: 12,388 consistent · 13 flipped
```

Those 13 flips are the decisions your rule change alters. Review them before
you ship. `--json` emits the full bucketed summary + per-record diffs;
`--fail-on-flip` exits non-zero (for a CI gate); `--ruleset <name>` forces one
rule; a source of `-` reads the JSONL from stdin.

## 3. Fixate as regression tests

Turn the captured decisions into a permanent regression suite:

```bash
ordo replay capture-2026-07-04.jsonl --write-tests
ordo test        # your production traffic is now a test suite
```

`--write-tests` merges each captured `{input → code, output}` into
`tests/<rule>.json` (deduped by input). From then on, `ordo test` fails if a
change alters those decisions.

## Replaying other logs

`ordo replay` reads any JSONL with `{rule_name, input, code, output}` lines. If
your application already logs its decisions (e.g. a service that calls Ordo and
records `{input, code}` per decision), you can replay that log directly without
running capture at all.
