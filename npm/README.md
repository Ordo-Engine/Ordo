# Ordo CLI

Deterministic guardrails for your AI coding agent, and a local dev loop for
authoring Ordo decision rules as files. Powered by a sub-microsecond Rust rule
engine.

## Guard your coding agent

```bash
npx @ordo-engine/cli guard init      # scaffold a policy + wire the Claude Code hook
```

Every Claude Code tool call now runs through a local rule that decides
**allow / deny / ask**. When the agent tries something destructive, Ordo stops
it with a reason:

```text
⛔ Denied by policy: Destructive shell command blocked by policy [policy@1.0.0 · DENY]
```

The policy lives in `.ordo-guard/` as a normal Ordo project — so your guardrails
have a test suite:

```bash
ordo guard test      # run the policy's own tests
ordo guard log       # every decision, timestamped and auditable
```

Edit `.ordo-guard/rulesets/policy.json` in plain expressions
(`tool == 'Bash' && command contains 'terraform destroy'`), add a test, ship.
The default policy blocks destructive shell + secret access, asks before
`git push` / `npm publish`, and fast-paths read-only git. Fails open on any
internal error (`--fail-closed` to deny instead).

## Author rules as files

```bash
npx @ordo-engine/cli init my-rules
cd my-rules
npx @ordo-engine/cli validate
npx @ordo-engine/cli test
npx @ordo-engine/cli trace loan-approval --input '{"amount":5000}'
```

Or install globally:

```bash
npm i -g @ordo-engine/cli
ordo --help
```

The install step downloads a prebuilt static binary for your platform from the
matching [GitHub Release](https://github.com/Ordo-Engine/Ordo/releases). If none
is available, build from source:

```bash
cargo install --git https://github.com/Ordo-Engine/Ordo ordo-cli
```

## Use it from a coding agent (MCP)

```bash
claude mcp add ordo -- ordo mcp
```

This exposes `list_files`, `read_file`, `grep`, `write_file`, `delete_file`,
`validate`, `run_tests`, `trace`, and `publish` to the agent. Local edits and
checks run offline; `publish` requires `ordo mcp --allow-publish`.

## Commands

| | |
|---|---|
| `ordo guard init` / `hook` / `test` / `log` | deterministic guardrails for a coding agent |
| `ordo init [dir]` | scaffold a project |
| `ordo validate` / `test` / `trace` | check rules offline |
| `ordo replay <captured.jsonl>` | replay recorded decisions; spot flips; `--write-tests` |
| `ordo fmt` / `lint` / `new` | format, lint, scaffold |
| `ordo login` / `link` / `pull` / `push` / `publish` | sync with the platform |
| `ordo mcp` | run as an MCP server (stdio) |

Add `--json` to any command for machine-readable output.
