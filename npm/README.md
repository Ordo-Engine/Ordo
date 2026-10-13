# Ordo CLI

Turn judgments into decisions. Ordo is an open-source decision engine: models
and scores tell you what is going on, and Ordo decides what to do about it.
Write those decisions as rule files with tests, see which decisions a change
moves before you ship it, and run the rules anywhere the Ordo engine runs.
Works offline, and coding agents can drive it over MCP.

## Write and test rules as files

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

## See what a change affects

Every app that uses a decision model like Jev has a line such as
`if jev.confidence > 0.85: refund(order)`. Nobody reviews it, and it means
something else after the next model update. In the
[jev-routing](https://github.com/Ordo-Engine/Ordo/tree/main/examples/rule-packs/jev-routing)
rule pack that threshold is a rule with tests. Lower it to 0.80 and all 12
tests still pass. `impact` shows what they missed:

```text
$ ordo impact refund-routing
impact refund-routing  (HEAD → working tree)
  309 inputs: 12 tests, 0 captured, 297 boundary probes

  LLM_DRAFT → AUTO_REFUND  18 inputs
  LLM_DRAFT → HUMAN_REVIEW  3 inputs
…
21 changed · 288 unchanged
```

18 requests that used to wait for a person now get their money back without
anyone reading the message.

`impact` runs your working copy next to the last commit in git, over the
ruleset's tests, real inputs you provide (`--inputs`) and probe inputs on both
sides of every threshold, and lists each decision that changed. Add
`--fail-on-change` to stop surprise changes in CI.

Ready-made projects to try it on (refund routing after Jev, credit approval,
promo stacking, fraud scoring) are in
[examples/rule-packs](https://github.com/Ordo-Engine/Ordo/tree/main/examples/rule-packs).

## Use it from a coding agent (MCP)

```bash
claude mcp add ordo -- ordo mcp
```

This exposes `list_files`, `read_file`, `grep`, `write_file`, `delete_file`,
`validate`, `run_tests`, `trace`, `impact`, and `publish` to the agent. Local edits and
checks run offline; `publish` requires `ordo mcp --allow-publish`.

## Ordo Guard: keep coding agents in bounds

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
