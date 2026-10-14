# Guard (`ordo guard`)

`ordo guard` runs a local Ordo rule on every tool call your coding agent makes. The rule answers allow, deny or ask, and the same call always gets the same answer. The policy is an ordinary Ordo project, so it has tests and traces, and every decision goes to a log.

| Agent       | Hook                   | Config file              | Sees                                      |
| ----------- | ---------------------- | ------------------------ | ----------------------------------------- |
| Claude Code | `PreToolUse`           | `.claude/settings*.json` | every tool (Bash, Read, Write, Edit, WebFetch, …) |
| Codex CLI   | `PreToolUse`           | `.codex/hooks.json`      | Bash only, for now (upstream limit)       |
| Cursor      | `beforeShellExecution` | `.cursor/hooks.json`     | shell commands only                       |

Claude Code and Codex CLI send the same event shape, so a rule behaves the same on both. Cursor only sees shell commands, so rules on `file_path` or `url` never fire there.

## Install

From the repository you want to guard:

```bash
npx @ordo-engine/cli guard init                         # Claude Code
npx @ordo-engine/cli guard init --agent codex            # Codex CLI
npx @ordo-engine/cli guard init --agent claude,codex,cursor
```

This creates `.ordo-guard/` (the policy in `rulesets/policy.json`, its tests, `facts.json`, `AGENTS.md`) and registers the hook for each agent. All agents share the one policy.

Restart the agent (in Claude Code, `/hooks` is enough), then check:

```bash
ordo guard doctor
# ✔ policy evaluates: policy@1.1.0
# ✔ policy tests: 24 passed
# ✔ Claude Code hook (.claude/settings.local.json): answers `rm -rf /` with deny
```

```text
$ (agent tries) rm -r -f ./build
⛔ Denied by policy: Destructive shell command blocked by policy [policy@1.1.0 · DENY]
```

The default policy:

- denies destructive shell commands (recursive `rm`, `find -delete`, `dd`, `mkfs`, `shred`),
- denies reading secrets (`.env`, `.pem`, `id_rsa`, AWS credentials), through file tools, `Grep` or shell arguments,
- asks before `git push`, `npm publish`, `git reset --hard`, `git clean`, and before edits to the guard or the agent's hook config,
- allows a single read-only git command,
- leaves everything else to the agent's normal permission flow.

By default the hook is registered with an absolute path in the git-ignored local settings. When `guard init` runs through `npx`, it copies the binary to `~/.ordo/bin/ordo` first, because agents silently skip a hook whose program is gone. Run `guard init` again after upgrading. To commit a hook the whole team can use, add `--shared`: it registers `npx -y @ordo-engine/cli guard hook` in the shared config instead.

## What the policy sees

The hook turns the agent's event into one input object, the same shape for every agent:

| Field             | Example             | Notes |
| ----------------- | ------------------- | ----- |
| `tool`            | `"Bash"`, `"Edit"`  | always `"Bash"` on Cursor |
| `command`         | `"git push origin"` | Bash |
| `file_path`       | `"src/main.rs"`     | Read, Write, Edit (not on Cursor) |
| `url`             | `"https://…"`       | WebFetch (not on Cursor) |
| `cwd`             | `"/repo"`           | |
| `permission_mode` | `"default"`         | not on Cursor |
| `session_id`      | `"c1a2…"`           | |
| `tool_input`      | `{ … }`             | the full tool input. Its other keys are copied to the top level too |
| `task_context`, `task`, `rel_path` | | see [Task scope](#task-scope) |
| `programs`, `subcommands`, `argv`, `commands`, `words`, `shell_parse` | | Bash, see below |

A condition on a missing field is false. So `command`-based rules are skipped for other tools, but `!(command contains 'x')` is also false when there is no `command`. Check the tool first: `tool == 'Bash' && !(command contains 'x')`.

## Shell commands

Matching text in `command` is easy to get around: `rm -r -f`, `rm  -rf` and `rm -Rf` all miss `'rm -rf'`. So the hook parses each Bash command the way a shell would: quotes, `&&` `||` `;` `|`, subshells, `$(…)`, `bash -c '…'`, `eval`, heredocs. It looks through `sudo`, `env`, `nohup`, `timeout`, `xargs` and similar wrappers, and adds:

| Field         | For `sudo git -C web push && rm -rf /tmp/x` | Use it as |
| ------------- | ------------------------------------------- | --------- |
| `programs`    | `["sudo", "git", "rm"]`                     | `'rm' in programs` |
| `subcommands` | `["git push", "rm /tmp/x"]`                 | `'git push' in subcommands` |
| `argv`        | `{"git": ["-C", "web", "push"], "rm": ["-rf", "-r", "-f", "/tmp/x"]}` | `'-r' in argv.rm` |
| `commands`    | `["sudo git -C web push", "rm -rf /tmp/x"]` | `len(commands) == 1` |
| `words`       | single-token arguments and redirect targets | `regex_match('[.]env', join(words, ' '))` |
| `shell_parse` | `"ok"`, or `"error"` for broken quoting     | `shell_parse == 'error'` |

The command is analyzed, not run. Variables, aliases and functions are invisible to the parser, so this makes bypasses harder but does not rule them out.

`argv.rm` is missing when `rm` did not run, and then the whole condition is false, including an `||` alternative. Write `'rm' in programs && '-r' in argv.rm`, and put alternatives on different fields in separate branches.

## Writing rules

The policy is a ruleset in the Studio file format. Branches are checked top to bottom and the first match wins. A terminal's code is the decision: `DENY`, `ASK`, `ALLOW`, and `PASS` (or any other code) for no opinion. The reason shown to the agent is the terminal's `message`, or a `reason` output.

```json
{
  "id": "gate-migrations",
  "label": "confirm migration edits",
  "condition": "tool in ['Write', 'Edit'] && file_path contains 'migrations/'",
  "nextStepId": "ask_migration"
}
```

Conditions are normal [expressions](/en/guide/expression-syntax). Useful functions here are `starts_with`, `ends_with`, `regex_match(pattern, s)` and `glob_match(patterns, s)`. In `regex_match` the pattern comes first. A backslash in an expression string is an escape (`'\s'` reaches the regex as `s`), so write `[.]` and a plain space instead of `\.` and `\s`.

## Task scope

A tool call says nothing about the task the agent is on. To limit a task to some files, write `.ordo-guard/context.json`:

```json
{
  "root": "/abs/path/to/repo",
  "session_id": "optional: only this agent session",
  "expires_at": "2026-10-08T00:00:00Z",
  "task": { "id": "login-signup", "touches": ["src/auth/**", "db/schema.sql"] }
}
```

The context applies only when the event's `cwd` is inside `root`, the session matches (if set) and it has not expired. Then the input gets `task` (passed through as written) and `rel_path` (the edited file, relative to `root`). `task_context` is always set: `active`, `absent`, `mismatch`, `expired` or `invalid`.

Put the scope rule after your base rules, so `rm -rf` and secrets stay denied inside the task:

```json
{
  "id": "gate-scope",
  "label": "stay inside the task scope",
  "condition": "task_context == 'active' && tool in ['Write', 'Edit'] && !glob_match(task.touches, rel_path)",
  "nextStepId": "ask_scope"
}
```

`glob_match`'s `*` also matches `/`. Avoid patterns that start with a wildcard, since they also match `../` paths outside the root. The check compares path strings and does not follow symlinks, and a Bash command can still write anywhere. Add an `ASK` on Bash if that matters for the task.

## Test the policy

Add a case to `.ordo-guard/tests/policy.json`:

```json
{
  "name": "blocks terraform destroy",
  "input": { "tool": "Bash", "command": "terraform destroy" },
  "expect": { "code": "DENY" }
}
```

```bash
ordo guard test
```

`guard test` fills in the shell fields from `command` the same way the hook does, so a case only needs `tool` and `command`. To see what the hook answers for one event:

```bash
echo '{"tool_name":"Bash","tool_input":{"command":"git -C web push"}}' | ordo guard hook
```

## Log

Every decision is appended to `.ordo-guard/log.jsonl` (git-ignored), with the time, session, tool, decision, reason and a one-line summary. Entries made under a task context also record the task id and a hash of `context.json`.

```bash
ordo guard log --tail 20
ordo guard log --json | jq 'select(.decision=="deny")'
```

## When guard itself fails

If the policy is missing, does not compile, or the event is malformed, the hook warns on stderr and lets the call go through the agent's normal flow. A broken guard should not block your agent. Register the hook with `--fail-closed` to deny instead.

Because of that, guard can be off without you noticing. `ordo guard doctor` checks every link: the policy runs, its tests pass, each hook is registered, its program exists, and it answers a sample `rm -rf /`. It exits non-zero when guard is not protecting the repo, so it works in CI or a pre-commit hook.

`guard init` never overwrites an existing `.ordo-guard/`. To move an older default policy to the current one, run `ordo guard upgrade` (`--dry-run` to preview). Files you edited are kept unless you pass `--force`, which saves the old file as `<file>.bak`.

## Limits

Guard is an extra layer, not a sandbox. It sees tool calls, not what they do, and it cannot see into variables or scripts. Use it together with the agent's own permissions, and do not rely on it against a process that is trying to get around it.
