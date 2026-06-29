# Ordo Demo Deployment — Studio on Vercel + Platform on Nomad

This wires up a public demo:

```
 Browser ──► app.ordoengine.com (Vercel, Studio SPA + static/CDN/TLS)
                 │  vercel.json rewrites /api/v1/* (edge proxy)
                 ▼
            api.ordoengine.com (Traefik on the Nomad cluster)
                 ▼
            ordo-platform :3001 ──► ordo-server (engine) ─┐
                 ▲                                         │
            ordo-platform-worker :8090                     │
                 ▼                                         ▼
            Postgres                                    NATS JetStream
```

**Why this split:** Studio's build compiles a Rust→WASM package that Vercel's
cloud build can't produce, and Studio calls a same-origin `/api/v1`. Building in
CI (Rust available) + a Vercel rewrite to the Nomad-hosted platform solves both
with no app code changes and no CORS.

---

## 0. Prerequisites (you provide)

| Thing | Value to decide |
|------|------|
| Studio domain | `app.ordoengine.com` → CNAME to Vercel |
| Platform domain | `api.ordoengine.com` → A record to the cluster's Traefik node IP |
| Traefik TLS | a cert resolver for `websecure` (Let's Encrypt). If none, use the HTTP fallback below. |
| NATS URL | the running cluster value, incl. token: `nats://ordo-nats-<token>@<ip>:4222` |
| Engine URL | an `ordo-server` base URL reachable from the platform (node IP:port) |
| JWT secret | 32+ char random string |
| DB password | Postgres password for the `ordo` user |
| Vercel project | linked to repo, **root dir = `ordo-editor/apps/studio`** |

> **Demo hardening (do not skip):** keep auth ON (the platform requires JWT login),
> never expose Postgres/NATS publicly (no Traefik tags on them — they stay
> cluster-internal), and add the daily reset in §6. Ordo's defaults assume a
> trusted network, so the only public surfaces should be `app.` and `api.`.

---

## 1. Publish the platform image

The control plane had no published image. Trigger the new workflow:

```bash
gh workflow run build-platform-image.yml
# → ghcr.io/pama-lee/ordo-platform:latest  (ordo-platform + ordo-platform-worker)
```

(If the package is private, `docker login ghcr.io` on the Nomad nodes or make it public.)

## 2. Deploy Postgres (if not already up)

```bash
nomad job run -var='postgres_password=<PASS>' deploy/nomad/ordo-postgres.nomad
```

## 3. Deploy the platform + worker

Pass secrets via `NOMAD_VAR_*` so they stay out of shell history:

```bash
export NOMAD_VAR_database_url='postgresql://ordo:<PASS>@<PG_IP>:5432/ordo_platform'
export NOMAD_VAR_nats_url='nats://ordo-nats-<token>@<NATS_IP>:4222'
export NOMAD_VAR_engine_url='http://<ENGINE_IP>:<PORT>'
export NOMAD_VAR_jwt_secret='<32+ char random>'

nomad job run deploy/nomad/ordo-platform.nomad
nomad job run deploy/nomad/ordo-platform-worker.nomad
```

Verify:

```bash
curl -fsS https://api.ordoengine.com/health        # platform readiness
curl -fsS http://<WORKER_NODE>:8090/health/live    # worker liveness (C9)
curl -fsS http://<WORKER_NODE>:8090/metrics        # worker metrics
```

### TLS fallback (no cert resolver on Traefik yet)

`ordo-platform.nomad` routes via `websecure` + `tls=true`. If Traefik has no cert
resolver, either add one (`traefik.http.routers.ordo-platform.tls.certresolver=<name>`)
or switch the router to HTTP by replacing those two tags with:

```
"traefik.http.routers.ordo-platform.entrypoints=web",
```

and set the Vercel rewrite destination (step 4) to `http://api.ordoengine.com`.
HTTPS is strongly recommended for anything public.

## 4. Deploy Studio to Vercel

1. Create the Vercel project, **Root Directory = `ordo-editor/apps/studio`**, then
   `vercel link` locally and copy `orgId`/`projectId` from `.vercel/project.json`.
2. Add repo secrets: `VERCEL_TOKEN`, `VERCEL_ORG_ID`, `VERCEL_PROJECT_ID`.
3. Confirm the rewrite target in `ordo-editor/apps/studio/vercel.json` matches the
   platform host (`https://api.ordoengine.com`).
4. Deploy:

```bash
gh workflow run deploy-studio.yml
```

5. Point `app.ordoengine.com` at Vercel (add the domain in the Vercel dashboard).

## 5. Smoke test

Open `https://app.ordoengine.com`, register/login, create a project, instantiate
the `loan-approval` or `ecommerce-coupon` template, run a test, and publish to an
environment — confirm the deployment shows `success` (a bound engine confirmed it)
or `dispatched` (no bound server yet).

## 6. Demo reset (recommended)

Add a Nomad periodic batch job that truncates demo data (or `DROP/CREATE` the
schema) nightly so the public sandbox stays clean. Keep a seed script that
re-creates a demo org/project + the sample templates after reset.

---

## Notes

- **Worker count:** release executions use Postgres advisory locks, so multiple
  workers are safe; the demo uses `count = 1`.
- **Engine sync:** publishing from the platform pushes rules to engines over NATS;
  register the engine in the platform's server registry to get confirmed
  `success` deployments instead of `dispatched`.
- **Images:** `ordo-server` comes from `release.yml`; `ordo-platform` from
  `build-platform-image.yml` (added with this change).
