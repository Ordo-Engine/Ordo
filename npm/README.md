# Ordo CLI

Deterministic guardrails for your AI coding agent, and a local dev loop for
authoring Ordo decision rules as files. Powered by a sub-microsecond Rust rule
engine.

## Guard your coding agent

```bash
npx @ordo-engine/cli guard init                        # Claude Code (default)
npx @ordo-engine/cli guard init --agent codex,cursor    # …and/or Codex CLI, Cursor
```

Every tool call from the hooked agent(s) now runs through the same local rule
that decides **allow / deny / ask**. When the agent tries something
destructive, Ordo stops it with a reason:

```text
⛔ Denied by policy: Destructive shell command blocked by policy [policy@1.1.0 · DENY]
```

The policy lives in `.ordo-guard/` as a normal Ordo project — so your guardrails
have a test suite:

```bash
ordo guard doctor    # is the hook registered, present, and actually answering?
ordo guard test      # run the policy's own tests
ordo guard log       # every decision, timestamped and auditable
ordo guard upgrade   # move a policy scaffolded by an older CLI to the current default
```

Edit `.ordo-guard/rulesets/policy.json` in plain expressions
(`tool == 'Bash' && 'terraform destroy' in subcommands`), add a test, ship.
Shell commands are parsed, not substring-matched, so `rm -r -f`, `/bin/rm -Rf`
and `bash -c 'rm -rf x'` hit the same rule. The default policy blocks
destructive shell + secret access, asks before `git push` / `npm publish`, and
fast-paths read-only git. Fails open on any
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
`validate`, `run_tests`, `trace`, `impact`, and `publish` to the agent. Local edits and
checks run offline; `publish` requires `ordo mcp --allow-publish`.

## Commands

| | |
|---|---|
| `ordo guard init` / `hook` / `test` / `log` / `doctor` | deterministic guardrails for a coding agent |
| `ordo init [dir]` | scaffold a project |
| `ordo validate` / `test` / `trace` | check rules offline |
| `ordo impact <ruleset>` | which decisions an edit changed vs git `HEAD` (tests + boundary probes) |
| `ordo replay <captured.jsonl>` | replay recorded decisions; spot flips; `--write-tests` |
| `ordo fmt` / `lint` / `new` | format, lint, scaffold |
| `ordo login` / `link` / `pull` / `push` / `publish` | sync with the platform |
| `ordo mcp` | run as an MCP server (stdio) |

Add `--json` to any command for machine-readable output.
