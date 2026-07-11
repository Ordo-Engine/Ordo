# Agent Guardrails (`ordo guard`)

An LLM is non-deterministic — ask it the same thing twice and you can get two
answers. That is fine for drafting prose and dangerous the moment an agent runs
a shell command, edits a file, or hits an API. `ordo guard` puts a
**deterministic decision layer** in front of your coding agent: every tool call
is evaluated by a local Ordo rule that answers **allow / deny / ask**.

The difference from an ad-hoc `if`-block or a hand-written allowlist: the policy
is a **normal Ordo project**, so your guardrails have a test suite, are
trace-debuggable, and every decision is written to an audit log.

One policy, enforced identically across whichever agents you hook up:

| Agent           | Hook                    | Config file            | Coverage                                     |
| --------------- | ------------------------ | ----------------------- | --------------------------------------------- |
| **Claude Code** | `PreToolUse`             | `.claude/settings*.json` | every tool (Bash, Read, Write, Edit, WebFetch, …) |
| **Codex CLI**   | `PreToolUse`             | `.codex/hooks.json`      | Bash today (upstream limitation)              |
| **Cursor**      | `beforeShellExecution`   | `.cursor/hooks.json`     | shell commands only                           |

Claude Code and Codex CLI speak the identical envelope, so a rule written for
one behaves the same on the other. Cursor only sees shell commands, so
`file_path`/`url`-based rules simply never fire there — `tool` is always
`"Bash"` for a Cursor event.

## Install (5 minutes)

From the repo you want to guard:

```bash
npx @ordo-engine/cli guard init                        # Claude Code (default)
npx @ordo-engine/cli guard init --agent codex           # Codex CLI
npx @ordo-engine/cli guard init --agent cursor          # Cursor
npx @ordo-engine/cli guard init --agent claude,codex,cursor   # all three at once
```

`--agent` is repeatable (`--agent codex --agent cursor`) or comma-separated;
each selected agent gets its own hook command and config file, all evaluating
the same `.ordo-guard/rulesets/policy.json`. This does two things:

1. Scaffolds `.ordo-guard/` — an Ordo project holding `rulesets/policy.json`,
   `tests/policy.json`, `facts.json`, and an `AGENTS.md`. Scaffolded once,
   shared by every agent.
2. Registers the hook for each selected agent (see the table above for
   which file).

Restart the agent (or, for Claude Code, run `/hooks`) to pick it up. From now on
every tool call runs through your policy:

```text
$ (agent tries) rm -rf ./build
⛔ Denied by policy: Destructive shell command blocked by policy [policy@1.0.0 · DENY]
```

The default policy blocks destructive shell (`rm -rf`, `dd`, `mkfs`) and secret
access (`.env`, `.pem`, `id_rsa`, aws credentials), asks before `git push` /
`npm publish` / edits to the guardrails themselves, fast-paths read-only git, and
lets everything else through to the agent's normal permission flow.

::: tip Sharing across a team
The default registration uses an absolute binary path in the git-ignored,
agent-local settings file. To commit a portable hook for the whole team, add
`--shared` — it registers `npx -y @ordo-engine/cli guard hook` (with an
`--agent` suffix for Codex/Cursor) in the agent's shared/committed config
instead. Claude Code is the only agent that distinguishes shared vs. local
files (`.claude/settings.json` vs. `.claude/settings.local.json`); Codex CLI
and Cursor each have one project-local file, but `--shared` still swaps in the
portable command.
:::

::: warning Cursor's decision envelope
Cursor's `beforeShellExecution` schema documents `allow` / `deny` / `ask`
outcomes but no explicit "no opinion" — so unlike Claude Code / Codex CLI
(where a rule that doesn't match prints nothing and defers to the agent's own
flow), a Cursor hook call with no matching policy rule explicitly emits
`allow` instead of staying silent.
:::

## The input your policy sees

The hook flattens the agent's event into one canonical input object — the same
shape regardless of which agent triggered it, so a rule you write once behaves
the same everywhere it's reachable. Reference these fields directly in
conditions:

| Field             | Example             | Notes                                                    |
| ----------------- | -------------------- | --------------------------------------------------------- |
| `tool`            | `"Bash"`, `"Edit"`   | the tool name — always `"Bash"` for a Cursor event        |
| `command`         | `"git push origin"`  | Bash — hoisted from `tool_input`                          |
| `file_path`       | `"src/main.rs"`      | Read/Write/Edit — hoisted (Claude Code / Codex CLI only)  |
| `url`             | `"https://…"`        | WebFetch — hoisted (Claude Code / Codex CLI only)         |
| `cwd`             | `"/repo"`            | working directory                                          |
| `permission_mode` | `"default"`          | Claude Code / Codex CLI permission mode; absent on Cursor |
| `session_id`      | `"c1a2…"`             | Cursor's `conversation_id` maps into this field too       |
| `tool_input`      | `{ … }`              | the full, nested tool input                                |

Any other key inside `tool_input` is hoisted to the top level too, so a new tool
is usable in conditions without a code change.

::: warning Missing fields are lenient
A condition referencing an **absent** field is `false`, so a `command`-based
rule is safely skipped for non-Bash tools. Be careful with negation:
`!(command contains 'x')` is _also_ false when `command` is absent. Prefer
guarding with the tool first: `tool == 'Bash' && !(command contains 'x')`.
:::

## Writing rules

Branch conditions are plain expression strings, evaluated top to bottom — first
match wins. Terminal codes map to decisions: `DENY`, `ASK`, `ALLOW`, and `PASS`
(or any other code) = no opinion.

```json
{
  "id": "gate-b0",
  "label": "block terraform destroy",
  "condition": "tool == 'Bash' && command contains 'terraform destroy'",
  "nextStepId": "deny_infra"
}
```

The expression language has `== != > >= < <=`, `&&` `||` `!`, `in`, `contains`,
and functions like `starts_with(s, prefix)`, `ends_with(s, suffix)`, and
`regex_match(pattern, s)`.

::: warning `regex_match` argument order
The **pattern comes first**: `regex_match('rm\\s+-rf', command)`, not the other
way around.
:::

The decision reason shown to the agent comes from the matched terminal's
`message` (or a `reason` output field, if you set one).

## Test your guardrails

Because the policy is a real Ordo project, add a case to `tests/policy.json`:

```json
{
  "name": "blocks terraform destroy",
  "input": { "tool": "Bash", "command": "terraform destroy" },
  "expect": { "code": "DENY" }
}
```

and run:

```bash
ordo guard test
# --- PASS: blocks rm -rf (0.10ms)
# --- PASS: asks before git push (0.09ms)
# …
```

Debug what a specific event does, step by step:

```bash
cd .ordo-guard
ordo trace policy --input '{"tool":"Bash","command":"git push"}'
```

## Audit log

Every decision is appended to `.ordo-guard/log.jsonl` (git-ignored):

```bash
ordo guard log --tail 20
ordo guard log --json | jq 'select(.decision=="deny")'
```

Each entry records the timestamp, session id, tool, decision, reason, duration,
and a one-line summary of what the call was about.

## Fail-open by design

If the guard itself fails — the policy is missing, a rule doesn't compile, the
event is malformed — the hook **fails open**: it warns on stderr, stays silent
on stdout, and the tool call proceeds under the agent's normal flow. (On
Cursor, "silent" means an explicit `allow` rather than empty stdout — see the
note above.) A broken guard should never wedge your agent. Pass
`--fail-closed` (in the registered command) to invert this and deny on
internal error instead.

## Limitations

Guard is **defense-in-depth, not a sandbox**. It sees tool _calls_, not their
side effects: a rule that asks before `Edit` to `.ordo-guard/` won't catch a
`bash sed -i` doing the same edit. Layer it with the agent's own permission
system; don't treat it as a security boundary against an adversarial process.

Per-agent gaps to know about, both upstream limitations rather than anything
`ordo guard` can work around: Codex CLI's `PreToolUse` currently only fires for
Bash (Read/Write/Edit/MCP calls don't trigger it yet), and Cursor's
`beforeShellExecution` only ever sees shell commands (no file-edit hook at
all), so file-path rules are unreachable there.
