# Server Registry & Multi-Region

Platform and execution nodes are loosely coupled — `ordo-server` instances **register themselves** with the platform on startup, and the platform maintains a directory used for release delivery and proxy routing.

## Registration Flow

```mermaid
sequenceDiagram
  participant Server as ordo-server
  participant Platform as ordo-platform

  Server->>Platform: POST /api/v1/internal/register<br/>{region, capabilities, version}
  Platform-->>Server: server_id + token
  loop every N seconds
    Server->>Platform: POST /api/v1/internal/heartbeat<br/>{metrics, healthy}
  end
```

> `/api/v1/internal/*` are machine-to-machine endpoints authenticated by server tokens — never exposed to browsers or SDKs.

## Server Directory

| Operation  | Endpoint                          |
| ---------- | --------------------------------- |
| List       | `GET /api/v1/servers`             |
| Get        | `GET /api/v1/servers/:id`         |
| Health     | `GET /api/v1/servers/:id/health`  |
| Metrics    | `GET /api/v1/servers/:id/metrics` |
| Deregister | `DELETE /api/v1/servers/:id`      |

Server record fields:

- `region` — deployment region tag
- `capabilities` — enabled features (e.g. `jit`, `signature`)
- `healthy` / `last_heartbeat`
- `current_rulesets` — ruleset digests held now

## Project Binding

Each project binds one or more servers (optionally per-environment). The binding controls:

1. Which ordo-servers receive a release.
2. Where business requests are routed when going through the platform proxy.

```http
PUT /api/v1/orgs/:oid/projects/:pid/server
{ "environment": "prod", "server_ids": ["s_eu", "s_us"] }
```

## Execution Proxy

Apps may not be able to reach regional ordo-servers directly. The platform offers a transparent proxy:

```http
POST /api/v1/engine/:project_id/execute
```

Requests are routed to the ordo-server bound for the project's current environment, preserving original latency metrics (the platform forwards but does not parse).

When to use it:

- Apps can only reach the public platform domain.
- Multi-region failover — health-aware routing falls back to a backup server.
- Canary traffic split — during canary releases, the platform splits traffic by ratio between old and new servers.

## Multi-Region Example

```mermaid
flowchart LR
  subgraph Central["Central Governance"]
    P[ordo-platform]
    DB[(Postgres)]
    P --- DB
  end
  subgraph NorthAmerica
    S1[ordo-server US-East]
    S2[ordo-server US-West]
  end
  subgraph Europe
    S3[ordo-server EU-West]
  end
  subgraph Asia
    S4[ordo-server AP-East]
  end

  S1 -- register/heartbeat --> P
  S2 -- register/heartbeat --> P
  S3 -- register/heartbeat --> P
  S4 -- register/heartbeat --> P

  Biz["Business app"] -- direct or via platform proxy --> S1
  Biz -- direct or via platform proxy --> S3
```
