# Nomad job for running the full Ordo devcontainer stack on a single node.
#
# This is intentionally not referenced by the public deployment README.
# It is meant for personal development use on an existing host that already has:
# - the repo checked out locally
# - a reachable Traefik instance backed by Consul Catalog
#
# Expected flow:
# 1. Build the runtime image locally:
#      docker build -t ordo-devcontainer:local -f deploy/nomad/devcontainer-runtime.Dockerfile .
# 2. Run the Nomad job:
#      NOMAD_TOKEN=<token> nomad job run \
#        -var "domain=studio.example.com" \
#        deploy/nomad/ordo-devcontainer.nomad
#
# Notes:
# - The job mounts the existing repo from the host.
# - The job uses a separate host data directory by default so current Docker data stays untouched.
# - Studio is exposed through Traefik. Platform and engine stay inside the host/network unless you add routes.

variable "datacenter" {
  type    = string
  default = "dc1"
}

variable "image" {
  type    = string
  default = "ordo-devcontainer:uid1001-watch8"
}

variable "node_name" {
  type        = string
  default     = "node1"
  description = "Nomad node hostname constraint"
}

variable "host_ip" {
  type        = string
  description = "Host IP used for in-container database connection"
}

variable "nats_url" {
  type = string
}

variable "domain" {
  type        = string
  description = "Traefik host rule target, for example studio.dev.example.com"
}

variable "repo_path" {
  type        = string
  default     = "/opt/ordo"
  description = "Absolute path to the cloned ordo repo on the host"
}

variable "data_path" {
  type        = string
  default     = "/opt/ordo-data"
  description = "Persistent data directory on the host"
}

variable "pgdata_host_path" {
  type    = string
  default = "/var/lib/docker/volumes/ordo_ordo-postgres/_data"
}

variable "postgres_user" {
  type    = string
  default = "ordo"
}

variable "postgres_password" {
  type        = string
  description = "PostgreSQL password for the ordo database"
}

variable "postgres_db" {
  type    = string
  default = "ordo_platform"
}

variable "jwt_secret" {
  type        = string
  description = "JWT secret for the platform running inside the devcontainer"
}

variable "database_url" {
  type        = string
  description = "Platform database URL passed into the devcontainer task env"
}

variable "platform_templates_dir" {
  type    = string
  default = "/workspace/crates/ordo-platform/templates"
}

variable "engine_url" {
  type        = string
  description = "External ordo-server base URL passed into the devcontainer platform"
}

variable "nats_subject_prefix" {
  type    = string
  default = "ordo.rules"
}

variable "studio_port" {
  type    = number
  default = 43002
}

variable "platform_port" {
  type    = number
  default = 43001
}

variable "engine_port" {
  type    = number
  default = 48080
}

job "ordo-devcontainer" {
  datacenters = [var.datacenter]
  type        = "service"

  group "devcontainer" {
    count = 1

    constraint {
      attribute = "${attr.unique.hostname}"
      value     = var.node_name
    }

    network {
      mode = "host"

      port "studio" {
        static = var.studio_port
      }

      port "platform" {
        static = var.platform_port
      }

      port "postgres" {
        static = 5432
      }
    }

    service {
      name     = "ordo-devcontainer-studio"
      port     = "studio"
      provider = "consul"
      tags = [
        "traefik.enable=true",
        "traefik.http.routers.ordo-devcontainer.rule=Host(`${var.domain}`)",
        "traefik.http.routers.ordo-devcontainer.entrypoints=web",
      ]

      check {
        name     = "studio-http"
        type     = "http"
        path     = "/"
        interval = "15s"
        timeout  = "5s"
      }
    }

    service {
      name     = "ordo-devcontainer-platform"
      port     = "platform"
      provider = "consul"

      check {
        name     = "platform-health"
        type     = "http"
        path     = "/health"
        interval = "15s"
        timeout  = "5s"
      }
    }

    task "workspace" {
      driver = "docker"

      config {
        image        = var.image
        network_mode = "host"
        ports        = ["studio", "platform"]

        volumes = [
          "${var.repo_path}:/workspace",
          "${var.data_path}:/data",
        ]
      }

      env {
        WORKSPACE                    = "/workspace"
        DATA_DIR                     = "/data"
        DATABASE_HOST                = var.host_ip
        DATABASE_PORT                = "5432"
        ORDO_JWT_SECRET              = var.jwt_secret
        ORDO_DATABASE_URL            = "postgresql://${var.postgres_user}:${var.postgres_password}@${var.host_ip}:5432/${var.postgres_db}"
        ORDO_ENGINE_URL              = var.engine_url
        ORDO_LOCAL_ENGINE_ENABLED    = "false"
        ORDO_NATS_URL                = var.nats_url
        ORDO_NATS_SUBJECT_PREFIX     = var.nats_subject_prefix
        ORDO_PLATFORM_TEMPLATES_DIR  = var.platform_templates_dir
        ORDO_PLATFORM_PROXY_TARGET   = "http://127.0.0.1:${var.platform_port}"
        STUDIO_PORT                  = "${var.studio_port}"
        PLATFORM_PORT                = "${var.platform_port}"
      }

      resources {
        cpu    = 1000
        memory = 4096
      }

      restart {
        attempts = 3
        interval = "10m"
        delay    = "20s"
        mode     = "delay"
      }
    }

    task "postgres" {
      driver = "docker"

      config {
        image        = "postgres:16-alpine"
        network_mode = "host"
        ports        = ["postgres"]

        volumes = [
          "${var.pgdata_host_path}:/var/lib/postgresql/data",
        ]
      }

      env {
        POSTGRES_USER     = var.postgres_user
        POSTGRES_PASSWORD = var.postgres_password
        POSTGRES_DB       = var.postgres_db
      }

      resources {
        cpu    = 300
        memory = 1024
      }

      restart {
        attempts = 3
        interval = "10m"
        delay    = "10s"
        mode     = "delay"
      }
    }
  }
}
