# Ordo Platform Worker — Nomad job
#
# Background worker that drives release rollouts/rollbacks. Same image as
# ordo-platform, but overrides the entrypoint to the worker binary. Headless
# except for a small liveness/metrics server on :8090 (see C9 — a stalled poll
# loop trips /health/live so Nomad reschedules it).
#
# Shares the same Postgres / NATS / engine as ordo-platform — pass identical vars.

variable "image" {
  type    = string
  default = "ghcr.io/ordo-engine/ordo-platform:latest"
}

variable "health_port" {
  type    = number
  default = 8090
}

variable "database_url" {
  type = string
}

variable "nats_url" {
  type = string
}

variable "nats_subject_prefix" {
  type    = string
  default = "ordo.rules"
}

variable "engine_url" {
  type = string
}

variable "jwt_secret" {
  type        = string
  description = "Required by config validation (min 32 chars); pass via NOMAD_VAR_jwt_secret."
}

job "ordo-platform-worker" {
  datacenters = ["dc1"]
  type        = "service"

  group "worker" {
    count = 1

    network {
      mode = "host"
      port "health" {
        static = var.health_port
      }
    }

    service {
      name     = "ordo-platform-worker"
      port     = "health"
      provider = "nomad"
      tags     = ["ordo", "platform", "worker"]

      check {
        name     = "worker-liveness"
        type     = "http"
        path     = "/health/live"
        interval = "15s"
        timeout  = "3s"
        check_restart {
          limit = 3
          grace = "30s"
        }
      }
    }

    restart {
      attempts = 3
      interval = "5m"
      delay    = "15s"
      mode     = "delay"
    }

    task "worker" {
      driver = "docker"

      config {
        image      = var.image
        entrypoint = ["/app/ordo-platform-worker"]
        ports      = ["health"]
      }

      env {
        ORDO_DATABASE_URL        = var.database_url
        ORDO_ENGINE_URL          = var.engine_url
        ORDO_NATS_URL            = var.nats_url
        ORDO_NATS_SUBJECT_PREFIX = var.nats_subject_prefix
        ORDO_JWT_SECRET          = var.jwt_secret
        ORDO_WORKER_HEALTH_ADDR  = "0.0.0.0:${var.health_port}"
        ORDO_LOG_LEVEL           = "info"
      }

      resources {
        cpu    = 300
        memory = 256
      }
    }
  }
}
