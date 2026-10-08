# CLI (`ordo`)

The `ordo` CLI manages decision rules as files. A project is a folder you edit
like source code, check locally (offline, in under a second), and sync to the
platform. People and AI coding agents use it the same way.

## Install

```bash
# one-off, no install
npx @ordo-engine/cli --help

# or globally
npm i -g @ordo-engine/cli
ordo --help
```

The install downloads a prebuilt static binary for your platform. Alternatively,
build from source: `cargo install --git https://github.com/Ordo-Engine/Ordo ordo-cli`.

Every command supports `--json` for machine-readable output.

## A decision project on disk

`ordo init` scaffolds a project as a tree of files that mirrors the Studio model:

```text
ordo.yaml              project + link config
rulesets/<name>.json   a ruleset (studio format)
facts.json             fact catalog (external inputs)
concepts.json          concept catalog (derived expressions)
tests/<name>.json      test cases for a ruleset
contracts/<name>.json  decision contract
AGENTS.md              guidance for coding agents
```

Put this folder in git so rules go through PRs, review, and CI like other code.

## The local loop (offline)

```bash
ordo init my-rules && cd my-rules

ordo validate                 # compile every condition, structured errors
ordo test                     # run the ruleset's test cases
ordo trace loan-approval --input '{"amount":5000}'   # show the execution path
ordo impact loan-approval     # which decisions changed since the last commit
ordo fmt                      # canonically format rule files
ordo lint                     # graph + style checks
ordo new ruleset|fact|concept <name>
```

`validate`, `test`, and `trace` run locally against the embedded engine, with
no network or server. Concepts are materialized the same way the platform does
it, so a local run matches production.

`ordo trace` prints the exact path an input takes through the steps. Use it
when a decision isn't what you expected.

```text
$ ordo trace loan-approval --input '{"amount":5000}'
code:    APPROVED
output:  { "approved": true, "amount": 5000 }
path:    check_amount -> approve
```

### See which decisions an edit changed

`ordo test` tells you whether the existing cases still pass. `ordo impact` runs
the last committed version (git `HEAD`) and your edited version side by side
and lists every input whose result changed.

It uses three kinds of input: the test cases, real cases passed with
`--inputs` (JSONL, the same shape `ordo replay` reads), and boundary probes.
For every comparison like `amount <= 10000`, and every decision-table cell, it
tries a value just below, at, and just above the threshold. A change shows up
even when no test sits near it.

```text
$ ordo impact loan-approval        # 10000 changed to 15000; ordo test still passes
impact loan-approval  (HEAD → working tree)
  8 inputs: 2 tests, 0 captured, 6 boundary probes

  REJECTED → APPROVED  3 inputs

CHANGED probe:amount=10001  {"amount":10001}
    code: REJECTED → APPROVED
    output.approved: false → true
...
```

Use `--base <rev>` to compare against another commit, or `--base-file` to
compare against a file. `--json` gives agents structured output, and
`--fail-on-change` exits non-zero when anything changed, for CI.

## Sync with the platform

The platform is a remote you pull from and push/publish to.

```bash
ordo login                                  # authenticate (token in ~/.ordo)
ordo link --org <org> --project <project>   # bind this folder to a project
ordo pull                                   # fetch rulesets + catalog + tests
# ...edit files...
ordo push                                   # upload drafts (facts/concepts/tests too)
ordo publish loan-approval --env staging    # deploy to an environment
ordo deployments                            # watch deployment status
ordo diff                                   # local vs the server's draft
```

`push` is a full sync: rulesets, facts, concepts, per-ruleset tests, and
contracts (`--rulesets-only` limits it). It uses optimistic locking: if the
server has newer changes, it tells you to `ordo pull` first.

### CI

The local commands work offline and return proper exit codes, so you can run
them in CI:

```yaml
- run: npx @ordo-engine/cli validate
- run: npx @ordo-engine/cli test
```

### Config & environment

Auth and API URL live in `~/.ordo/config.toml` (chmod 600). For CI, set
`ORDO_TOKEN` and `ORDO_API_URL` instead; they override the file.

## Drive it from an AI agent

`ordo mcp` exposes these tools over the Model Context Protocol so a coding agent
(Claude Code, Cursor) can author, test, and ship rules for you. See
[MCP](/en/platform/mcp).

## Shell completions

```bash
ordo completions zsh > ~/.zfunc/_ordo    # bash | zsh | fish | powershell | elvish
```

## Command summary

| Group      | Commands                                                                    |
| ---------- | --------------------------------------------------------------------------- |
| Scaffold   | `init`, `new`                                                               |
| Local loop | `validate`, `test`, `trace`, `impact`, `exec`, `eval`, `fmt`, `lint`        |
| Platform   | `login`, `whoami`, `link`, `pull`, `push`, `publish`, `deployments`, `diff` |
| Agent      | `mcp`                                                                       |
| Misc       | `completions`                                                               |
