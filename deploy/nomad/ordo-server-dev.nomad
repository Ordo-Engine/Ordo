# Ordo Rule Engine Server - Nomad Job Configuration (Development)
#
# This job is designed for local development with dynamic port allocation.
# It uses fewer resources and debug-level logging.
#
# Usage: nomad job run ordo-server-dev.nomad
#
# Ports: Dynamic - use `nomad job status ordo-server-dev` to find allocated ports

job "ordo-server-dev" {
  datacenters = ["dc1"]
  type        = "service"
  
  group "ordo" {
    count = 1
    
    # Dynamic port allocation to avoid IPv6 binding issues
    network {
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
        timeout  = "5s"
        
        check_restart {
          limit           = 3
          grace           = "30s"
          ignore_warnings = false
        }
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
        timeout  = "5s"
        
        check_restart {
          limit           = 3
          grace           = "30s"
          ignore_warnings = false
        }
      }
    }
    
    task "ordo-server" {
      driver = "docker"
      
      config {
        # Use GitHub Container Registry image
        image = "ghcr.io/pama-lee/ordo:0.1.5-pre"
        ports = ["http", "grpc"]
      }
      
      # Debug logging for development
      env {
        # info 级别足够日常开发，debug 会消耗大量 CPU
        # 生产环境建议使用 warn
        RUST_LOG = "info"
      }
      
      # Resource limits for development
      # Note: cpu is a soft limit, container can burst above this
      resources {
        cpu    = 1000  # MHz (1 core) - 足够支撑 ~1000 QPS
        memory = 128   # MB - Ordo 内存占用很低
      }
    }
  }
}
