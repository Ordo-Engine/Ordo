# MCP Server (`ordo mcp`)

`ordo mcp` runs Ordo as a [Model Context Protocol](https://modelcontextprotocol.io)
server over stdio, so a coding agent (Claude Code, Cursor, Windsurf, …) gets
Ordo's tools natively — it can read, write, validate, test, and ship decision
rules for you without leaving your editor.

## Register

```bash
# Claude Code
claude mcp add ordo -- ordo mcp
```

For other clients, add a stdio MCP server whose command is `ordo mcp`, run from
inside a decision project (the folder created by [`ordo init`](/en/platform/cli)).

## Tools

The server exposes nine tools. Read/edit/check tools operate on the local
project files and the embedded engine — offline and instant; only `publish`
reaches the platform.

| Tool          | What it does                                      |
| ------------- | ------------------------------------------------- |
| `list_files`  | List the project's files                          |
| `read_file`   | Read a file                                       |
| `grep`        | Search files for a substring                      |
| `write_file`  | Create/overwrite a file                           |
| `delete_file` | Delete a ruleset/tests/contracts file             |
| `validate`    | Compile a ruleset, structured errors              |
| `run_tests`   | Run a ruleset's test cases                        |
| `trace`       | Execute an input and return the step-by-step path |
| `publish`     | Deploy a ruleset to an environment                |

## Safety

The server is local-first and git-backed, so file edits are reversible and
allowed by default. High-risk actions are gated by flags:

```bash
ordo mcp --allow-publish     # permit the publish tool
ordo mcp --allow-delete      # permit deleting ruleset files
```

Without `--allow-publish`, the `publish` tool returns a blocked result rather
than deploying — so an agent can propose a release but a human stays in control.

## Typical flow

1. You tell your agent: _"add a rule: approve if amount ≤ 10000, else reject."_
2. The agent uses `list_files` / `read_file` to understand the project, then
   `write_file` to add `rulesets/loan-approval.json`.
3. It calls `validate` and `run_tests`, fixing anything that fails.
4. It calls `trace` to confirm a sample input takes the expected path.
5. With `--allow-publish`, it can `publish` — otherwise it hands off to you.

Because `validate`/`test`/`trace` are offline and sub-second, the agent's
edit → check loop is tight, and its results match what the platform would
produce (concepts are materialized identically).
