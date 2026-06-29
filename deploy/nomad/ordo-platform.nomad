# Ordo Platform (control plane) — Nomad job
#
# Serves the platform HTTP API on :3001 and is exposed publicly through Traefik
# at https://api.ordoengine.com. The Studio frontend (on Vercel) rewrites its
# /api/v1/* calls to this host, so this is the single public entrypoint for the
# control plane.
#
# Depends on: ordo-postgres (ordo-postgres.nomad), ordo-nats, and an ordo-server
# engine (ordo-server-cluster). Deploy those first.
#
# Deploy:
#   nomad job run \
#     -var='database_url=postgresql://ordo:PASS@HOST:5432/ordo_platform' \
#     -var='nats_url=nats://ordo-nats-TOKEN@HOST:4222' \
#     -var='engine_url=http://HOST:PORT' \
#     -var='jwt_secret=<32+ char random>' \
#     deploy/nomad/ordo-platform.nomad
# (Prefer NOMAD_VAR_jwt_secret / NOMAD_VAR_database_url env vars so secrets stay out of shell history.)

variable "image" {
  type        = string
  description = "Platform image (built by build-platform-image.yml)"
  default     = "ghcr.io/pama-lee/ordo-platform:latest"
}

variable "http_port" {
  type    = number
  default = 3001
}

variable "database_url" {
  type        = string
  description = "Postgres DSN for the platform database"
}

variable "nats_url" {
  type        = string
  description = "NATS URL incl. token, e.g. nats://ordo-nats-<token>@<ip>:4222"
}

variable "nats_subject_prefix" {
  type    = string
  default = "ordo.rules"
}

variable "engine_url" {
  type        = string
  description = "ordo-server engine base URL the platform proxies to"
}

variable "jwt_secret" {
  type        = string
  description = "JWT signing secret (min 32 chars). Pass via NOMAD_VAR_jwt_secret."
}

variable "cors_origins" {
  type        = string
  description = "Allowed CORS origins for the Studio frontend"
  default     = "https://app.ordoengine.com"
}

variable "public_host" {
  type    = string
  default = "api.ordoengine.com"
}

job "ordo-platform" {
  datacenters = ["dc1"]
  type        = "service"

  group "platform" {
    count = 1

    network {
      mode = "host"
      port "http" {
        static = var.http_port
      }
    }

    service {
      name     = "ordo-platform"
      port     = "http"
      provider = "nomad"
      tags = [
        "ordo",
        "platform",
        "traefik.enable=true",
        "traefik.http.routers.ordo-platform.rule=Host(`${var.public_host}`)",
        "traefik.http.routers.ordo-platform.entrypoints=websecure",
        "traefik.http.routers.ordo-platform.tls=true",
        "traefik.http.services.ordo-platform.loadbalancer.passhostheader=true",
      ]

      check {
        name     = "platform-health"
        type     = "http"
        path     = "/health"
        interval = "10s"
        timeout  = "2s"
      }
    }

    restart {
      attempts = 3
      interval = "5m"
      delay    = "15s"
      mode     = "delay"
    }

    task "platform" {
      driver = "docker"

      config {
        image = var.image
        ports = ["http"]
        args  = ["--addr", "0.0.0.0:${var.http_port}"]
      }

      env {
        ORDO_DATABASE_URL        = var.database_url
        ORDO_ENGINE_URL          = var.engine_url
        ORDO_NATS_URL            = var.nats_url
        ORDO_NATS_SUBJECT_PREFIX = var.nats_subject_prefix
        ORDO_JWT_SECRET          = var.jwt_secret
        ORDO_PLATFORM_CORS_ORIGINS = var.cors_origins
        ORDO_LOG_LEVEL           = "info"
      }

      resources {
        cpu    = 500
        memory = 512
      }
    }
  }
}
