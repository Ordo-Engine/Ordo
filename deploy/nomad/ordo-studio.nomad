# Ordo Studio - Nomad Job Configuration
#
# Purpose:
# - Run the Studio frontend dev server on Nomad
# - Keep all existing backend data in place
# - Route traffic through the existing Traefik instance via Consul Catalog
#
# Usage:
#   NOMAD_TOKEN=<token> \
#   nomad job run \
#     -var "domain=studio.example.com" \
#     -var "platform_proxy_target=http://<YOUR_PLATFORM_IP>:3001" \
#     deploy/nomad/ordo-studio.nomad
#
# Notes:
# - This job intentionally does NOT deploy postgres / nats / ordo-server / ordo-platform.
# - It only runs Studio and points /api traffic at the existing platform instance.
# - The source tree is mounted from the current host path so you keep dev-mode behavior.

variable "datacenter" {
  type    = string
  default = "dc1"
}

variable "domain" {
  type        = string
  description = "Domain bound by Traefik, e.g. studio.example.com"
}

variable "platform_proxy_target" {
  type        = string
  default     = "http://0.0.0.0:3001"
  description = "Existing ordo-platform base URL used by the Vite dev proxy"
}

variable "repo_path" {
  type        = string
  default     = "/opt/ordo"
  description = "Absolute path to the cloned ordo repo on the host"
}

variable "studio_port" {
  type    = number
  default = 3002
}

job "ordo-studio" {
  datacenters = [var.datacenter]
  type        = "service"

  group "studio" {
    count = 1

    network {
      port "http" {
        to = var.studio_port
      }
    }

    service {
      name     = "ordo-studio"
      port     = "http"
      provider = "consul"
      tags = [
        "traefik.enable=true",
        "traefik.http.routers.ordo-studio.rule=Host(`${var.domain}`)",
        "traefik.http.routers.ordo-studio.entrypoints=web",
        "traefik.http.services.ordo-studio.loadbalancer.server.port=${var.studio_port}",
      ]

      check {
        name     = "studio-http"
        type     = "http"
        path     = "/"
        interval = "15s"
        timeout  = "5s"

        check_restart {
          limit           = 3
          grace           = "2m"
          ignore_warnings = false
        }
      }
    }

    task "studio" {
      driver = "docker"

      config {
        image = "node:22-alpine"
        ports = ["http"]

        volumes = [
          "${var.repo_path}/ordo-editor:/app",
          "local/node_modules:/app/node_modules",
        ]

        command = "sh"
        args = [
          "-lc",
          "corepack enable && pnpm install && pnpm --filter @ordo-engine/studio dev -- --host 0.0.0.0 --port ${NOMAD_PORT_http}",
        ]
      }

      env {
        ORDO_PLATFORM_PROXY_TARGET = var.platform_proxy_target
        CI                         = "true"
      }

      resources {
        cpu    = 1000
        memory = 1536
      }

      restart {
        attempts = 3
        interval = "10m"
        delay    = "15s"
        mode     = "fail"
      }
    }
  }
}
