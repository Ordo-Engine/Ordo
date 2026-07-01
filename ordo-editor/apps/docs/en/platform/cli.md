# CLI (`ordo`)

The `ordo` CLI brings decision rules into your development workflow. A project is
a folder of files you edit like source code, check locally (offline, sub-second),
and sync to the platform. It's designed to be equally usable by a person and by
an AI coding agent.

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

`ordo init` scaffolds a project — a tree of files that mirrors the Studio model:

```text
ordo.yaml              project + link config
rulesets/<name>.json   a ruleset (studio format)
facts.json             fact catalog (external inputs)
concepts.json          concept catalog (derived expressions)
tests/<name>.json      test cases for a ruleset
contracts/<name>.json  decision contract
AGENTS.md              guidance for coding agents
```

Put this folder in git — rules now get PRs, review, and CI like any code.

## The local loop (offline)

```bash
ordo init my-rules && cd my-rules

ordo validate                 # compile every condition, structured errors
ordo test                     # run the ruleset's test cases
ordo trace loan-approval --input '{"amount":5000}'   # show the execution path
ordo fmt                      # canonically format rule files
ordo lint                     # graph + style checks
ordo new ruleset|fact|concept <name>
```

`validate`, `test`, and `trace` run entirely locally against the embedded
engine — no network, no server. Concepts are materialized the same way the
platform does, so a local run matches production.

`ordo trace` is the debugging tool: it prints the exact path an input takes
through the steps, which is invaluable when a decision isn't what you expected.

```text
$ ordo trace loan-approval --input '{"amount":5000}'
code:    APPROVED
output:  { "approved": true, "amount": 5000 }
path:    check_amount -> approve
```

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
contracts (`--rulesets-only` limits it). It uses optimistic locking — if the
server has newer changes you'll be told to `ordo pull` first.

### CI

Because the local commands are offline and return proper exit codes, they drop
straight into CI:

```yaml
- run: npx @ordo-engine/cli validate
- run: npx @ordo-engine/cli test
```

### Config & environment

Auth and API URL live in `~/.ordo/config.toml` (chmod 600). For CI, set
`ORDO_TOKEN` and `ORDO_API_URL` instead — they override the file.

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
| Local loop | `validate`, `test`, `trace`, `exec`, `eval`, `fmt`, `lint`                  |
| Platform   | `login`, `whoami`, `link`, `pull`, `push`, `publish`, `deployments`, `diff` |
| Agent      | `mcp`                                                                       |
| Misc       | `completions`                                                               |
