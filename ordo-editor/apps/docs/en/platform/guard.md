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

Restart the agent (or, for Claude Code, run `/hooks`) to pick it up, then
confirm the whole chain works:

```bash
ordo guard doctor
# ✔ policy evaluates: policy@1.1.0
# ✔ policy tests: 24 passed
# ✔ Claude Code hook (.claude/settings.local.json): answers `rm -rf /` with deny
```

From now on every tool call runs through your policy:

```text
$ (agent tries) rm -r -f ./build
⛔ Denied by policy: Destructive shell command blocked by policy [policy@1.1.0 · DENY]
```

The default policy blocks destructive shell (recursive `rm`, `find -delete`,
`dd`, `mkfs`, `shred`) and secret access (`.env`, `.pem`, `id_rsa`, aws
credentials — through file tools, `Grep`, and shell arguments alike), asks
before `git push` / `npm publish` / `git reset --hard` / `git clean` and before
edits to the guardrails or the agent's hook config, fast-paths a single
read-only git command, and lets everything else through to the agent's normal
permission flow. Shell rules match the *parsed* command (see
[Shell commands](#shell-commands)), so `rm -rf`, `rm -r -f`, `/bin/rm -Rf`,
`sudo env X=1 rm --recursive` and `bash -c 'rm -rf x'` are all the same rule.

::: tip Running through npx
`npx` runs the binary out of its package cache, which `npm cache clean` can
delete — and agents silently skip a hook whose program is missing. So when
`guard init` runs from the npx cache it copies the binary to `~/.ordo/bin/ordo`
and registers that path. Re-run `guard init` after upgrading to refresh it.
:::

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
| `task_context`    | `"active"`           | always set — see [Task context](#task-context)              |
| `task`, `rel_path` | `{ … }`, `"src/a.ts"` | only when a task context is active                        |
| `programs`, `subcommands`, `argv`, `commands`, `words`, `shell_parse` | | Bash only — see [Shell commands](#shell-commands) |

Any other key inside `tool_input` is hoisted to the top level too, so a new tool
is usable in conditions without a code change.

::: warning Missing fields are lenient
A condition referencing an **absent** field is `false`, so a `command`-based
rule is safely skipped for non-Bash tools. Be careful with negation:
`!(command contains 'x')` is _also_ false when `command` is absent. Prefer
guarding with the tool first: `tool == 'Bash' && !(command contains 'x')`.
:::

## Shell commands

Matching substrings of `command` is easy to bypass: `command contains 'rm -rf'`
misses `rm -r -f`, `rm  -rf` (two spaces), and `rm -Rf`. So for every `Bash`
call the hook also parses `command` the way a POSIX shell would — quotes and
escapes, `&&` `||` `;` `|` `&`, subshells, `$(…)` and backticks, `bash -c '…'`
and `eval` payloads, heredocs (their bodies are data, not commands) — unwraps
`sudo`, `doas`, `env`, `nohup`, `nice`, `timeout`, `xargs`, `command`, …, and
adds these fields:

| Field         | Example for `sudo git -C web push && rm -rf /tmp/x` | Use it as                         |
| ------------- | ---------------------------------------------------- | --------------------------------- |
| `programs`    | `["sudo", "git", "rm"]`                              | `'rm' in programs`                |
| `subcommands` | `["git push", "rm /tmp/x"]` (program + first non-flag argument, skipping options like `git -C dir`) | `'git push' in subcommands` |
| `argv`        | `{"git": ["-C", "web", "push"], "rm": ["-rf", "-r", "-f", "/tmp/x"], …}` — short-flag clusters are also split | `'-r' in argv.rm` |
| `commands`    | `["sudo git -C web push", "rm -rf /tmp/x"]`          | `len(commands) == 1`              |
| `words`       | every single-token argument and redirect target (free text such as a commit message is left out) | `regex_match('[.]env', join(words, ' '))` |
| `shell_parse` | `"ok"`, or `"error"` for unbalanced quoting / too-deep nesting | `shell_parse == 'error'` |

```json
{
  "id": "gate-tf",
  "label": "block terraform destroy",
  "condition": "tool == 'Bash' && 'terraform destroy' in subcommands",
  "nextStepId": "deny_infra"
}
```

This is analysis, not execution: variables, aliases and shell functions are
invisible to it, so it raises the bar rather than closing every door.

::: warning One missing field makes the whole condition false
`'-r' in argv.rm` is fine when `rm` ran — but if it didn't, `argv.rm` is
missing and the *entire* condition is false, even an `||` alternative that
would have matched. Guard each lookup (`'rm' in programs && '-r' in argv.rm`)
and put alternatives that read different fields in separate branches.
:::

## Writing rules

Branch conditions are plain expression strings, evaluated top to bottom — first
match wins. Terminal codes map to decisions: `DENY`, `ASK`, `ALLOW`, and `PASS`
(or any other code) = no opinion.

```json
{
  "id": "gate-migrations",
  "label": "confirm migration edits",
  "condition": "tool in ['Write', 'Edit'] && file_path contains 'migrations/'",
  "nextStepId": "ask_migration"
}
```

The expression language has `== != > >= < <=`, `&&` `||` `!`, `in`, `contains`,
and functions like `starts_with(s, prefix)`, `ends_with(s, suffix)`, and
`regex_match(pattern, s)`, and `glob_match(pattern_or_patterns, s)`.

::: warning `regex_match` argument order and backslashes
The **pattern comes first**: `regex_match('[.]pem$', file_path)`, not the other
way around. A backslash inside an expression string is an escape (`'\s'`
reaches the regex as plain `s`), so prefer `[.]` and a literal space over
`\.` and `\s`.
:::

The decision reason shown to the agent comes from the matched terminal's
`message` (or a `reason` output field, if you set one).

## Task context

A tool-call event says nothing about *the task* the agent is working on, so a
policy can say "never run `terraform destroy`" but not "for this task, only
touch `src/auth/`". To scope a task, write `.ordo-guard/context.json` (by
hand, or from a task-planning tool) and keep one hand-written policy that
reads it:

```json
{
  "root": "/abs/path/to/repo",
  "session_id": "optional: only this agent session",
  "expires_at": "2026-10-08T00:00:00Z",
  "task": { "id": "login-signup", "touches": ["src/auth/**", "db/schema.sql"] }
}
```

`task` is passed through as-is: put whatever your policy needs in it. The
context only applies when it is bound to the event:

| Check        | Rule                                                                     |
| ------------ | ------------------------------------------------------------------------ |
| `root`       | the event's `cwd` must be inside it (default: the repo holding `.ordo-guard/`) |
| `session_id` | if set, must equal the event's session id                                |
| `expires_at` | if set (RFC 3339), must be in the future                                 |

When it applies, the input gains `task` and `rel_path`: the edited file
(`file_path` / `notebook_path`) relative to `root`, `/`-separated, with `.`
and `..` resolved. Paths outside the root come back as `../…`. The input
*always* carries `task_context`, so the policy can decide what "no usable
task" means:

| `task_context` | Meaning                                                    |
| -------------- | ----------------------------------------------------------- |
| `active`       | context applied: `task` and `rel_path` are set              |
| `absent`       | no `context.json`                                           |
| `mismatch`     | wrong `cwd` (outside `root`) or wrong session               |
| `expired`      | past `expires_at`                                           |
| `invalid`      | unreadable or malformed (a warning goes to stderr)          |

Put the scope rule *after* your base rules, so `rm -rf` and secret access are
still denied inside the task:

```json
{
  "id": "gate-scope",
  "label": "stay inside the task scope",
  "condition": "task_context == 'active' && tool in ['Write', 'Edit'] && !glob_match(task.touches, rel_path)",
  "nextStepId": "ask_scope"
}
```

`glob_match` takes one pattern or an array of patterns (true if any
matches). Its `*` also matches `/`, so `src/auth/*` covers subdirectories
too; avoid patterns that start with a wildcard (`**`, `*/…`), since those
also match `../` paths outside the root.

Every audit-log entry made under a context records `task_context`,
`task_id`, and `context_hash` (`sha256:` of the exact `context.json`), so a
decision can be traced back to the task definition that was active. Like the
rest of guard, the check is on tool *calls*: it compares path strings and
does not resolve symlinks, and a `Bash` command can still write anywhere.
Pair a scope rule with an `ASK` on `Bash` if that matters for the task.

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

`ordo guard test` derives the [shell fields](#shell-commands) from `command`
exactly like the live hook, so a case only needs `tool` and `command`.

See what the live hook answers for a specific event:

```bash
echo '{"tool_name":"Bash","tool_input":{"command":"git -C web push"}}' | ordo guard hook
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

Failing open — or a hook whose program has gone missing, which every agent
skips silently — means you may not notice guard is off. `ordo guard doctor`
checks each link: the policy evaluates, its tests pass, a hook is registered,
its program exists, and each registered hook answers a sample `rm -rf /` the
way the agent would run it. It exits non-zero when guard isn't protecting the
repo, so it also works as a CI or pre-commit check.

## Limitations

Guard is **defense-in-depth, not a sandbox**. It sees tool _calls_, not their
side effects, and shell parsing can't see through variables or scripts: the
default policy asks before `sed -i … .ordo-guard/…`, but not before a script
that edits the same file. Layer it with the agent's own permission
system; don't treat it as a security boundary against an adversarial process.

Per-agent gaps to know about, both upstream limitations rather than anything
`ordo guard` can work around: Codex CLI's `PreToolUse` currently only fires for
Bash (Read/Write/Edit/MCP calls don't trigger it yet), and Cursor's
`beforeShellExecution` only ever sees shell commands (no file-edit hook at
all), so file-path rules are unreachable there.
