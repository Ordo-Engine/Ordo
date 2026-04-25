# Organizations & Projects

Every resource in Ordo Platform lives under a two-level hierarchy: organization → project. Organizations are the billing and governance boundary; projects aggregate rulesets, contracts, and test suites.

## Organization

- Any registered user can create organizations.
- An organization owns its members, roles, notifications, and projects.
- **Sub-organizations** allow further isolation by business line or department.
- Org-level API: `/api/v1/orgs`, `/api/v1/orgs/:id`, `/api/v1/orgs/:id/sub-orgs`.

## Members & Roles (RBAC)

Built-in roles:

| Role     | Scope                                                         |
| -------- | ------------------------------------------------------------- |
| `owner`  | Full org control, including member management & billing       |
| `admin`  | Project administration, approval, release                     |
| `editor` | Author drafts and create release requests, no approval rights |
| `viewer` | Read-only                                                     |

Custom roles via `POST /api/v1/orgs/:oid/roles`, with action-level granularity (e.g. `release.approve`, `ruleset.publish`).

Member management:

- `POST /api/v1/orgs/:id/members` — invite
- `PUT /api/v1/orgs/:oid/members/:uid/roles` — adjust roles
- Sub-org members: `/api/v1/orgs/:parent_id/sub-orgs/:sub_id/members`

## Project

Projects are where rulesets and contracts actually live.

- Create: `POST /api/v1/orgs/:oid/projects`
- From template: `POST /api/v1/orgs/:oid/projects/from-template` — instantiate a built-in template (e-commerce coupon, loan approval, …) in one click
- Project structure:
  - **Environments** — defaults `dev` / `staging` / `prod`, customizable
  - **Facts** — project-scoped typed field definitions
  - **Concepts** — composite types reused across rulesets
  - **Contracts** — input/output schemas
  - **RuleSets** — business decision logic
  - **Sub-Rule assets** — logic snippets reusable across rulesets
  - **Test suites**
  - **Release policies**
  - **Bound server** — execution cluster

## Project API Cheatsheet

| Resource     | Endpoint                                       |
| ------------ | ---------------------------------------------- |
| Project      | `/api/v1/orgs/:oid/projects/:pid`              |
| Environments | `/api/v1/orgs/:oid/projects/:pid/environments` |
| Facts        | `/api/v1/projects/:pid/facts`                  |
| Concepts     | `/api/v1/projects/:pid/concepts`               |
| Contracts    | `/api/v1/projects/:pid/contracts`              |
| RuleSets     | `/api/v1/orgs/:oid/projects/:pid/rulesets`     |
| Sub-rules    | `/api/v1/orgs/:oid/projects/:pid/sub-rules`    |
| Tests        | `/api/v1/projects/:pid/rulesets/:name/tests`   |
| Releases     | `/api/v1/orgs/:oid/projects/:pid/releases`     |
| Engine proxy | `/api/v1/engine/:project_id/*path`             |

## Notifications

Each org has a notification queue covering:

- Pending release approvals
- Failed releases / canary anomalies
- Failed test suites
- Member invitations and role changes

API:

- `GET  /api/v1/orgs/:oid/notifications`
- `GET  /api/v1/orgs/:oid/notifications/count`
- `POST /api/v1/orgs/:oid/notifications/:nid/read`
- `POST /api/v1/orgs/:oid/notifications/read-all`
