variable "datacenter" {
  type    = string
  default = "dc1"
}

variable "image" {
  type = string
}

variable "domain" {
  type = string
}

variable "nats_url" {
  type = string
}

variable "nats_url_node1" {
  type    = string
  default = ""
}

variable "nats_url_node2" {
  type    = string
  default = ""
}

variable "nats_subject_prefix" {
  type    = string
  default = "ordo.rules"
}

variable "ghcr_username" {
  type        = string
  description = "GitHub Container Registry username"
}

variable "ghcr_password" {
  type = string
}

variable "platform_url" {
  type        = string
  default     = ""
  description = "Optional platform URL for HTTP fallback registration."
}

variable "node1_name" {
  type        = string
  default     = "node1"
  description = "Nomad hostname for the first cluster node"
}

variable "node2_name" {
  type        = string
  default     = "node2"
  description = "Nomad hostname for the second cluster node"
}

job "ordo-server-cluster" {
  datacenters = [var.datacenter]
  type        = "service"

  group "node1" {
    count = 2

    constraint {
      attribute = "${attr.unique.hostname}"
      value     = var.node1_name
    }

    network {
      mode = "host"

      port "http" {}
      port "grpc" {}
    }

    service {
      name     = "ordo-server-cluster"
      provider = "consul"
      port     = "http"
      tags = [
        "traefik.enable=true",
        "traefik.http.routers.ordo-engine.rule=Host(`${var.domain}`) && PathPrefix(`/engine`)",
        "traefik.http.routers.ordo-engine.entrypoints=web",
        "traefik.http.routers.ordo-engine.priority=100",
        "traefik.http.routers.ordo-engine.service=ordo-engine",
        "traefik.http.routers.ordo-engine.middlewares=ordo-engine-strip",
        "traefik.http.middlewares.ordo-engine-strip.stripprefix.prefixes=/engine",
        "traefik.http.services.ordo-engine.loadbalancer.passhostheader=true",
      ]

      check {
        name     = "ordo-server-health"
        type     = "http"
        path     = "/health"
        interval = "15s"
        timeout  = "5s"
      }
    }

    task "server" {
      driver = "docker"

      template {
        destination = "secrets/ordo-server.env"
        env         = true
        data        = <<-EOF
ORDO_SERVER_NAME=ordo-server-{{ env "attr.unique.hostname" }}-{{ env "NOMAD_ALLOC_INDEX" }}
ORDO_SERVER_TOKEN=ordo-server-{{ env "attr.unique.hostname" }}-{{ env "NOMAD_ALLOC_ID" }}
ORDO_INSTANCE_ID=ordo-server-{{ env "attr.unique.hostname" }}-{{ env "NOMAD_ALLOC_INDEX" }}
ORDO_SERVER_URL=http://{{ env "NOMAD_IP_http" }}:{{ env "NOMAD_HOST_PORT_http" }}
ORDO_HTTP_ADDR=0.0.0.0:{{ env "NOMAD_PORT_http" }}
ORDO_GRPC_ADDR=0.0.0.0:{{ env "NOMAD_PORT_grpc" }}
EOF
      }

      config {
        image        = var.image
        network_mode = "host"
        ports        = ["http", "grpc"]
        auth {
          username = var.ghcr_username
          password = var.ghcr_password
        }
      }

      env {
        ORDO_RULES_DIR             = "/data/rules"
        ORDO_MULTI_TENANCY_ENABLED = "true"
        ORDO_NATS_URL              = var.nats_url_node1 != "" ? var.nats_url_node1 : var.nats_url
        ORDO_NATS_SUBJECT_PREFIX   = var.nats_subject_prefix
        ORDO_PLATFORM_URL          = var.platform_url
      }

      resources {
        cpu    = 200
        memory = 768
      }
    }
  }

  group "node2" {
    count = 2

    constraint {
      attribute = "${attr.unique.hostname}"
      value     = var.node2_name
    }

    network {
      mode = "host"

      port "http" {}
      port "grpc" {}
    }

    service {
      name     = "ordo-server-cluster"
      provider = "consul"
      port     = "http"
      tags = [
        "traefik.enable=true",
        "traefik.http.routers.ordo-engine.rule=Host(`${var.domain}`) && PathPrefix(`/engine`)",
        "traefik.http.routers.ordo-engine.entrypoints=web",
        "traefik.http.routers.ordo-engine.priority=100",
        "traefik.http.routers.ordo-engine.service=ordo-engine",
        "traefik.http.routers.ordo-engine.middlewares=ordo-engine-strip",
        "traefik.http.middlewares.ordo-engine-strip.stripprefix.prefixes=/engine",
        "traefik.http.services.ordo-engine.loadbalancer.passhostheader=true",
      ]

      check {
        name     = "ordo-server-health"
        type     = "http"
        path     = "/health"
        interval = "15s"
        timeout  = "5s"
      }
    }

    task "server" {
      driver = "docker"

      template {
        destination = "secrets/ordo-server.env"
        env         = true
        data        = <<-EOF
ORDO_SERVER_NAME=ordo-server-{{ env "attr.unique.hostname" }}-{{ env "NOMAD_ALLOC_INDEX" }}
ORDO_SERVER_TOKEN=ordo-server-{{ env "attr.unique.hostname" }}-{{ env "NOMAD_ALLOC_ID" }}
ORDO_INSTANCE_ID=ordo-server-{{ env "attr.unique.hostname" }}-{{ env "NOMAD_ALLOC_INDEX" }}
ORDO_SERVER_URL=http://{{ env "NOMAD_IP_http" }}:{{ env "NOMAD_HOST_PORT_http" }}
ORDO_HTTP_ADDR=0.0.0.0:{{ env "NOMAD_PORT_http" }}
ORDO_GRPC_ADDR=0.0.0.0:{{ env "NOMAD_PORT_grpc" }}
EOF
      }

      config {
        image        = var.image
        network_mode = "host"
        ports        = ["http", "grpc"]
        auth {
          username = var.ghcr_username
          password = var.ghcr_password
        }
      }

      env {
        ORDO_RULES_DIR             = "/data/rules"
        ORDO_MULTI_TENANCY_ENABLED = "true"
        ORDO_NATS_URL              = var.nats_url_node2 != "" ? var.nats_url_node2 : var.nats_url
        ORDO_NATS_SUBJECT_PREFIX   = var.nats_subject_prefix
        ORDO_PLATFORM_URL          = var.platform_url
      }

      resources {
        cpu    = 200
        memory = 768
      }
    }
  }
}
