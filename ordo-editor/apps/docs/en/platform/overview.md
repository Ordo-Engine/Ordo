# Studio and Platform

The engine works on its own. Add the platform when a team needs a browser editor, reviews before release, and several environments.

| Part            | What it does                                                              |
| --------------- | ------------------------------------------------------------------------- |
| `ordo-platform` | Organizations, projects, members, drafts, tests, releases, audit. Serves Studio. |
| Studio          | The web editor, in the browser                                            |
| `ordo-server`   | Runs the rules. The platform never runs rules itself; it sends them to engines. |

## How it fits together

1. Model the inputs: [facts, concepts and contracts](./modeling).
2. Write rules in [Studio](./studio), or as files with the [CLI](./cli). Both work on the same project.
3. Try them on sample input and run their tests.
4. Open a [release](./releases). Reviewers approve it, and the platform sends the rules to the engines of that environment.
5. Applications call the engine. See [Engines](./servers).

## Organizations and projects

Everything lives in a project, and projects live in an organization.

- An organization holds members, roles, notifications and projects. It can have sub-organizations, for example one per business line.
- A project holds facts, concepts, contracts, rulesets, sub-rules, tests, environments (`dev`, `staging`, `prod` by default, more if you add them), release policies and the engine it is bound to.
- A new project can start from a template, such as loan approval or an e-commerce coupon.

Built-in roles:

| Role     | Can                                                   |
| -------- | ----------------------------------------------------- |
| `owner`  | Everything, including members                         |
| `admin`  | Manage projects, approve and run releases             |
| `editor` | Edit drafts and open release requests                 |
| `viewer` | Read only                                             |

Custom roles can be made from finer permissions, such as approving releases or publishing rulesets.

Members get notifications for releases waiting on them, failed releases, and invitations.

## First rule in Studio

1. Sign in, create an organization, then a project.
2. Click New ruleset and pick a template. You get a working decision graph, and the fact catalog is filled in for it.
3. Open the trace panel, paste an input such as `{ "amount": 5000, "is_vip": true }` and click Run. You see the path, each step and the result.
4. Open a release to `staging`. Once it is approved, the platform sends the rule to that environment's engine.
5. Call it. Studio's Integrate tab shows the call for your project:

```bash
curl -X POST https://<engine>/api/v1/execute/loan-approval \
  -H 'x-tenant-id: <project-id>' -H 'Content-Type: application/json' \
  -d '{"input":{"amount":5000,"is_vip":true}}'
```

The project id is the tenant on the engine.

## From files instead

```bash
ordo init my-rules && cd my-rules
ordo login
ordo link --org <org> --project <project>
ordo push
ordo publish loan-approval --env staging
```

See [CLI](./cli) for the sync commands. A coding agent can drive the same steps through the [MCP server](./mcp).
