# Ordo guard policy

This folder is the tool-call policy for AI coding agents working in the parent
repo. On every Claude Code PreToolUse event, `ordo guard hook` evaluates
`rulesets/policy.json` against the event and answers allow / deny / ask;
any other terminal code (conventionally `PASS`) means "no opinion" and Claude
Code's normal permission flow applies. Decisions are appended to `log.jsonl`.

## Input the policy sees
Flattened from the hook event — reference these directly in conditions:
- `tool` — the tool name (`Bash`, `Read`, `Write`, `Edit`, …)
- hoisted tool inputs: `command` (Bash), `file_path` (Read/Write/Edit), `url`, …
- `cwd`, `permission_mode`, `session_id`; the full `tool_input` object is nested.

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
