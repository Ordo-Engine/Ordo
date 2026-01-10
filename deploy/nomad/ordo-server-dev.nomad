# Ordo Rule Engine Server - Nomad Job Configuration (Development)
#
# This job is designed for local development with dynamic port allocation.
# It uses fewer resources and debug-level logging.
#
# Usage: nomad job run ordo-server-dev.nomad
#
# Ports: Dynamic allocation - check `nomad job status ordo-server-dev` for assigned ports

job "ordo-server-dev" {
  datacenters = ["dc1"]
  type        = "service"
  
  group "ordo" {
    count = 1
    
    # Network configuration with dynamic ports (suitable for development)
    network {
      # Dynamic port allocation - Nomad will assign available ports
      port "http" {
        to = 8080
      }
      port "grpc" {
        to = 50051
      }
    }
    
    # HTTP Service registration (using Nomad native service discovery)
    service {
      name     = "ordo-dev-http"
      port     = "http"
      provider = "nomad"
      tags     = ["ordo", "dev", "http"]
      
      check {
        name     = "health"
        type     = "http"
        path     = "/health"
        interval = "10s"
        timeout  = "3s"
      }
    }
    
    # gRPC Service registration (using Nomad native service discovery)
    service {
      name     = "ordo-dev-grpc"
      port     = "grpc"
      provider = "nomad"
      tags     = ["ordo", "dev", "grpc"]
      
      check {
        name     = "tcp-alive"
        type     = "tcp"
        interval = "10s"
        timeout  = "3s"
      }
    }
    
    task "ordo-server" {
      driver = "docker"
      
      config {
        # Use GitHub Container Registry image
        # For local development, you can also use: ordo-server:latest (locally built)
        image = "ghcr.io/pama-lee/ordo-server:latest"
        ports = ["http", "grpc"]
      }
      
      # Debug logging for development
      env {
        RUST_LOG = "debug"
      }
      
      # Lower resource limits for development
      resources {
        cpu    = 200  # MHz
        memory = 128  # MB
      }
    }
  }
}
