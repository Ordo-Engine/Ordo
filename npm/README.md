# Ordo CLI

Author, test, trace, and ship Ordo decision rules — rules-as-files, with a local
dev loop and an MCP server so your coding agent can drive it.

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
| `ordo init [dir]` | scaffold a project |
| `ordo validate` / `test` / `trace` | check rules offline |
| `ordo fmt` / `lint` / `new` | format, lint, scaffold |
| `ordo login` / `link` / `pull` / `push` / `publish` | sync with the platform |
| `ordo mcp` | run as an MCP server (stdio) |

Add `--json` to any command for machine-readable output.
