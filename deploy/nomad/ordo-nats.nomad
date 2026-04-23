variable "datacenter" {
  type    = string
  default = "dc1"
}

variable "node_name" {
  type        = string
  default     = "node1"
  description = "Nomad node hostname constraint"
}

variable "image" {
  type    = string
  default = "nats:2.11-alpine"
}

variable "data_path" {
  type    = string
  default = "/home/ubuntu/.local/share/ordo-nats"
}

variable "client_port" {
  type    = number
  default = 4222
}

variable "server_name" {
  type    = string
  default = "ordo-nats-node1"
}

variable "nats_token" {
  type = string
}

job "ordo-nats" {
  datacenters = [var.datacenter]
  type        = "service"

  group "nats" {
    count = 1

    constraint {
      attribute = "${attr.unique.hostname}"
      value     = var.node_name
    }

    network {
      mode = "host"

      port "client" {
        static = var.client_port
      }
    }

    service {
      name     = "ordo-nats"
      provider = "consul"
      port     = "client"

      check {
        name     = "nats-tcp"
        type     = "tcp"
        interval = "15s"
        timeout  = "5s"
      }
    }

    task "nats" {
      driver = "docker"

      config {
        image        = var.image
        network_mode = "host"
        ports        = ["client"]
        args = [
          "--js",
          "--sd=/data",
          "--server_name=${var.server_name}",
          "--addr=0.0.0.0",
          "--port=${NOMAD_PORT_client}",
          "--auth=${var.nats_token}",
        ]
        volumes = [
          "${var.data_path}:/data",
        ]
      }

      resources {
        cpu    = 300
        memory = 512
      }
    }
  }
}
