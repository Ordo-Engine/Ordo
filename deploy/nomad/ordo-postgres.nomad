# Nomad PostgreSQL job for reusing the existing Docker volume data.
# Important:
# - This must not run at the same time as the Docker Compose postgres container.
# - The source data directory is the existing Docker local volume mountpoint:
#   /var/lib/docker/volumes/ordo_ordo-postgres/_data

variable "datacenter" {
  type    = string
  default = "dc1"
}

variable "node_name" {
  type        = string
  default     = "node1"
  description = "Nomad node hostname constraint"
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

variable "postgres_port" {
  type    = number
  default = 5432
}

job "ordo-postgres" {
  datacenters = [var.datacenter]
  type        = "service"

  group "postgres" {
    count = 1

    constraint {
      attribute = "${attr.unique.hostname}"
      value     = var.node_name
    }

    network {
      port "db" {
        static = var.postgres_port
      }
    }

    service {
      name     = "ordo-postgres"
      port     = "db"
      provider = "consul"

      check {
        name     = "postgres-tcp"
        type     = "tcp"
        interval = "10s"
        timeout  = "3s"
      }
    }

    task "postgres" {
      driver = "docker"

      config {
        image = "postgres:16-alpine"
        ports = ["db"]
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
        cpu    = 500
        memory = 1024
      }
    }
  }
}
