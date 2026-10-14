# CLI (`ordo`)

The `ordo` CLI keeps rules as files in a folder you put in git. It checks and tests them locally, and syncs them with the platform.

## Install

```bash
npm install -g @ordo-engine/cli    # or run it without installing: npx @ordo-engine/cli
ordo --help
```

The package downloads a prebuilt binary for Linux, macOS or Windows. To build it yourself: `cargo install --git https://github.com/Ordo-Engine/Ordo ordo-cli`.

Every command takes `--json` for machine-readable output.

## A project on disk

`ordo init my-rules` creates:

```text
ordo.yaml              project settings and platform link
rulesets/<name>.json   rulesets
tests/<name>.json      test cases for a ruleset
facts.json             fact catalog
concepts.json          concepts (named expressions over facts)
contracts/<name>.json  decision contracts, when you add them
AGENTS.md              notes for coding agents
```

## Commands

| Command | What it does |
| --- | --- |
| `init [dir]` | Create a project |
| `new ruleset\|fact\|concept <name>` | Add a ruleset (with an empty test file), a fact or a concept |
| `validate [name]` | Compile rulesets and report errors |
| `test [name]` | Run test cases. `--rule` and `--tests` for a single file |
| `trace <name> --input <json>` | Run one input and print the path |
| `impact <name>` | List the decisions an edit changed. See [Testing and Impact](/en/guide/testing) |
| `replay <file>` | Rerun captured decisions on the current rules |
| `decisions <path>` | Summarize a gateway decision log. See [Decision Model Gateway](./decision-gateway) |
| `exec --rule <file> --input <json>` | Run a single rule file outside a project |
| `eval <expr> --input <json>` | Evaluate one expression |
| `fmt`, `lint` | Format rule files; check the graph and style |
| `guard ...` | Agent guardrails. See [Guard](./guard) |
| `mcp` | Run as an MCP server for coding agents. See [MCP Server](./mcp) |
| `login`, `whoami`, `link` | Sign in to the platform and link the folder to a project |
| `pull`, `push`, `diff` | Sync files with the platform's drafts |
| `publish <name> --env <env>`, `deployments` | Release a ruleset and watch its deployments |
| `completions <shell>` | Shell completion for bash, zsh, fish, powershell or elvish |

`validate`, `test`, `trace`, `impact` and `replay` run on the embedded engine, offline. Concepts are computed the same way the platform does it, so local results match production.

## Sync with the platform

```bash
ordo login
ordo link --org <org> --project <project>
ordo pull                                   # rulesets, catalog and tests
# edit files
ordo push                                   # rulesets, facts, concepts, tests and contracts
ordo publish loan-approval --env staging
ordo deployments
ordo diff                                   # local files against the platform's drafts
```

`push` sends everything unless you add `--rulesets-only`. If the platform has newer changes, it asks you to `ordo pull` first.

The token and API URL are kept in `~/.ordo/config.toml` (mode 600). In CI, set `ORDO_TOKEN` and `ORDO_API_URL` instead.
