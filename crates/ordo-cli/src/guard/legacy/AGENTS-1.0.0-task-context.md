# Ordo guard policy

This folder is the tool-call policy for AI coding agents working in the parent
repo. `ordo guard hook` evaluates `rulesets/policy.json` on every pre-tool-call
event and answers allow / deny / ask; any other terminal code (conventionally
`PASS`) means "no opinion" and the agent's normal permission flow applies.
Decisions are appended to `log.jsonl`.

One policy, enforced identically across whichever agents are hooked up —
`ordo guard init --agent <claude|codex|cursor>` (repeatable/comma-separated;
default `claude`). Claude Code and Codex CLI speak the same PreToolUse
protocol; Cursor's `beforeShellExecution` hook only sees shell commands (no
file-edit visibility), so `file_path`-based rules simply never fire for it.

## Input the policy sees
Flattened from the hook event — reference these directly in conditions:
- `tool` — the tool name (`Bash`, `Read`, `Write`, `Edit`, …; always `Bash` for
  Cursor, since it only hooks shell execution)
- hoisted tool inputs: `command` (Bash), `file_path` (Read/Write/Edit), `url`, …
- `cwd`, `permission_mode`, `session_id`; the full `tool_input` object is nested.
- `task_context` (`active`/`absent`/`mismatch`/`expired`/`invalid`), plus `task`
  and `rel_path` (edited path relative to the task root) when a
  `context.json` task context is active — e.g.
  `task_context == 'active' && tool in ['Write', 'Edit'] && !glob_match(task.touches, rel_path)`.

Missing fields are *lenient*: a condition referencing an absent field is false,
so a `command`-based rule is safely skipped for non-Bash tools. Careful with
negations — `!(command contains 'x')` is also false when `command` is absent.

## Writing rules
Branch conditions are bare expression strings, first match wins:
- `"tool == 'Bash' && command contains 'terraform destroy'"`
- `"tool in ['Write', 'Edit'] && file_path contains 'migrations/'"`
- `regex_match(pattern, s)` — the **pattern comes first**.
Terminal codes: `DENY` / `ASK` / `ALLOW` / `PASS`. The terminal `message` (or a
`reason` output field) is shown to the agent as the decision reason.

## Workflow
1. Edit `rulesets/policy.json` — add a branch + a terminal (or reuse one).
2. Add a case to `tests/policy.json`: `{ "name", "input": { "tool", ... }, "expect": { "code" } }`.
3. `ordo guard test` — the guardrails themselves must be green.
4. Debug a decision: `cd .ordo-guard && ordo trace policy --input '{"tool":"Bash","command":"git push"}'`.
5. `ordo guard log` — recent live decisions.

Guard is defense-in-depth, not a sandbox: it sees tool calls, not their side
effects (e.g. `sed -i` can edit files a `Write` rule would catch).
