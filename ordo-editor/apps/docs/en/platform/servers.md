# Engines

The platform does not run rules. `ordo-server` instances (engines) register with it, a project is bound to an engine, and releases are delivered to that engine.

## Connect an engine

In Studio, open Servers and create a connect token for your organization. The dialog prints a command like this:

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

| Variable | Why |
| --- | --- |
| `ORDO_PLATFORM_URL` and `ORDO_SERVER_TOKEN` | Both are needed for the engine to register. It then sends a heartbeat every 30 seconds. |
| `ORDO_CONNECT_TOKEN` | Puts the engine in your organization. Only members of that organization see it. |
| `ORDO_SERVER_URL` | The address the platform uses to reach the engine |
| `ORDO_NATS_URL` | The same NATS the platform uses. Releases arrive through it. |

Revoking a connect token stops new registrations with it. Engines that already registered stay until you delete them. An engine registered before connect tokens existed has no organization: set `ORDO_CONNECT_TOKEN` and restart it.

Studio's server list shows each engine's health and metrics.

## Bind a project

A project is bound to engines in its own organization, per environment if you like. Releases for that environment go to those engines.

## Call a ruleset

Applications call the engine directly. The project id is the tenant:

```bash
curl -X POST https://<engine>/api/v1/execute/loan-approval \
  -H 'x-tenant-id: <project-id>' -H 'Content-Type: application/json' \
  -d '{"input":{"amount":5000,"is_vip":true}}'
```

```json
{ "code": "APPROVED", "message": "Within limit", "output": { "approved": true }, "duration_us": 6 }
```

Branch on `code` and read computed fields from `output`. Studio's Integrate tab fills in this call, with curl, Node, Python and Go versions, for the ruleset you pick.

If an application can only reach the platform, it can go through the platform's proxy with a platform login token: `POST /api/v1/engine/<project-id>/execute/loan-approval` is forwarded to the bound engine's `/api/v1/execute/loan-approval`. Calling the engine directly is faster.

The engine is meant for internal networks. See [Running the Server](/en/guide/server#access) for the API token, TLS and limits.
