# Quickstart

Ship your first decision in five minutes — create a project, author a rule from a
template, try it on sample input, publish it, and call it from your app. There's
no engine to install; the platform runs one for you.

> New to Ordo? Skim the [Platform Overview](./overview) for the mental model
> (model → author → test → release → run). This page is the hands-on path.

There are two ways in. Pick one — they produce the same result and you can switch
anytime: the [CLI](./cli) pulls what you build in [Studio](./studio), and Studio
shows what you push from the CLI.

- **Studio (web)** — click-to-build in the browser. Best for a first look.
- **CLI (local)** — rules-as-files in your git repo, driven by you or an AI coding agent.

## Path A — Studio (web)

### 1. Create a project

Sign in to Studio, create an **organization**, then a **project** inside it. A
project is the unit that owns your facts, concepts, rulesets, environments, and
the engine it runs on. See [Organizations & Projects](./organizations).

### 2. Start from a template

New ruleset → pick **Loan Approval** (or Ecommerce Coupon). You get a working
decision graph — a decision step on `amount`, terminals for approve/reject — and
the [Fact Catalog](./catalog) is pre-filled with the inputs it reads.

### 3. Try it

Open the trace panel, paste a sample input, and **Try run**:

```json
{ "amount": 5000, "is_vip": true }
```

You'll see the matched branch, the full path, per-step timing, and the terminal
`code` / `output`. This is the same engine that serves production —
[Studio Editor](./studio) covers the three views and the trace panel.

### 4. Publish

Open a release to an environment (start with **staging**). Tests and a diff run
automatically; once approved, the platform delivers the rule to that
environment's engine. See [Release Pipeline](./releases).

### 5. Call it

Your app calls the engine at runtime — see [Runtime Integration](./integrate):

```bash
POST https://<engine>/api/v1/execute/loan-approval
Header: x-tenant-id: <project-id>
Body:   { "input": { "amount": 5000, "is_vip": true } }
```

```json
{ "code": "APPROVED", "output": { "approved": true }, "duration_us": 6 }
```

## Path B — CLI (local, git-native)

Everything above, as files in your repo. Nothing to install — `npx` fetches a
prebuilt binary.

### 1. Scaffold + local loop (offline)

```bash
npx @ordo-engine/cli init my-rules && cd my-rules

ordo validate     # compile every condition, structured errors
ordo test         # run the ruleset's test cases
ordo trace loan-approval --input '{"amount":5000,"is_vip":true}'
```

`validate` / `test` / `trace` run on an embedded engine — offline, sub-second, and
concept-identical to production. See [CLI](./cli).

### 2. Connect to the platform

```bash
ordo login
ordo link --org <org> --project <project>
ordo push                                # rulesets + facts + concepts + tests
ordo publish loan-approval --env staging
```

### 3. Let an AI agent drive it

```bash
claude mcp add ordo -- ordo mcp
```

Now your coding agent has Ordo as native tools — it reads, writes, validates,
tests, and traces rules on the local project, and proposes releases you approve.
See [MCP Server](./mcp).

### 4. Call it

Same runtime call as Path A → [Runtime Integration](./integrate).

## What you built

| Piece           | What it is                                                    |
| --------------- | ------------------------------------------------------------- |
| **Project**     | Owns facts, rulesets, environments, and the bound engine      |
| **Ruleset**     | The decision graph you authored and tested                    |
| **Environment** | Where a published version runs (staging → prod)               |
| **Engine call** | `POST /api/v1/execute/<name>` with your project as the tenant |

## Next

- [Fact Catalog](./catalog) · [Decision Contracts](./contracts) — model typed inputs and I/O
- [Release Pipeline](./releases) — review, canary, rollback
- [Test Management](./testing) — cases, suites, CI
- [Runtime Integration](./integrate) — REST, gRPC, and the official SDKs
