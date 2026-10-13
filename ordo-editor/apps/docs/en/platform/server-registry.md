# Server Registry & Multi-Region

Platform and execution nodes are loosely coupled. `ordo-server` instances register themselves with the platform on startup, and the platform keeps a directory used for release delivery and proxy routing.

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

> `/api/v1/internal/*` are machine-to-machine endpoints authenticated by server tokens. They are never exposed to browsers or SDKs.

## Org Connect Tokens

An engine is scoped to an organization by the token it registers with. Mint a
**connect token** for an org and give it to that org's engines. They register
into that org, not into a global pool visible to everyone.

```http
POST   /api/v1/orgs/:oid/connect-tokens      # mint (returns the raw token ONCE)
GET    /api/v1/orgs/:oid/connect-tokens      # list metadata (never the raw token)
DELETE /api/v1/orgs/:oid/connect-tokens/:id  # revoke
```

Start an engine with the token. Studio's **Servers → Create token** dialog prints this command with the values filled in:

```bash
docker run -d --name ordo-engine -p 8080:8080 \
  -e ORDO_PLATFORM_URL=https://studio.example.com \
  -e ORDO_CONNECT_TOKEN=ordo_connect_xxxx \
  -e ORDO_SERVER_TOKEN=<unique per engine> \
  -e ORDO_SERVER_NAME=engine-1 \
  -e ORDO_SERVER_URL=http://ENGINE_HOST:8080 \
  -e ORDO_NATS_URL=nats://NATS_HOST:4222 \
  ghcr.io/ordo-engine/ordo:latest
```

The connect token alone doesn't register anything: the engine only registers when `ORDO_PLATFORM_URL` and `ORDO_SERVER_TOKEN` are also set, and it only receives releases when `ORDO_NATS_URL` points at the same NATS the platform uses.

The engine sends the token as an `x-connect-token` header on registration; the
platform derives the owning org from it and records the server under that org.
The token both authorizes registration and scopes the engine. It
replaces the global `--platform-registration-secret` for org-scoped setups.

Consequences:

- The [server directory](#server-directory) and the [project binding](#project-binding)
  picker only show engines registered to an org you belong to.
- A project may bind only to an engine in its own organization; binding to a
  server from another org is rejected.
- Revoking a token stops new registrations with it; already-registered servers
  keep their assigned org (delete the server to remove it).

> Upgrading an existing engine: an engine registered before connect tokens
> (or without one) has no org and is no longer listed under any org. Mint a token,
> set `ORDO_CONNECT_TOKEN`, and restart the engine. It re-registers into the org.

## Server Directory

| Operation  | Endpoint                          |
| ---------- | --------------------------------- |
| List       | `GET /api/v1/servers`             |
| Get        | `GET /api/v1/servers/:id`         |
| Health     | `GET /api/v1/servers/:id/health`  |
| Metrics    | `GET /api/v1/servers/:id/metrics` |
| Deregister | `DELETE /api/v1/servers/:id`      |

Server record fields:

- `region`: deployment region tag
- `capabilities`: enabled features (e.g. `jit`, `signature`)
- `healthy` / `last_heartbeat`
- `current_rulesets`: ruleset digests currently held

## Project Binding

Each project binds one or more servers (optionally per-environment). The binding controls:

1. Which ordo-servers receive a release.
2. Where business requests are routed when going through the platform proxy.

```http
PUT /api/v1/orgs/:oid/projects/:pid/server
{ "environment": "prod", "server_ids": ["s_eu", "s_us"] }
```

## Execution Proxy

If apps can't reach regional ordo-servers directly, they can go through the platform's transparent proxy:

```http
POST /api/v1/engine/:project_id/execute
```

Requests are routed to the ordo-server bound for the project's current environment, preserving original latency metrics (the platform forwards but does not parse).

When to use it:

- Apps can only reach the public platform domain.
- Multi-region failover: health-aware routing falls back to a backup server.
- Canary traffic split: during canary releases, the platform splits traffic by ratio between old and new servers.

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
