# Ordo Rule Engine Server - Nomad Job Configuration (Production)
# 
# This job deploys the Ordo rule engine server with static ports for
# production environments. It includes health checks, service registration,
# and automatic rollback on deployment failures.
#
# Usage: nomad job run ordo-server.nomad
#
# Ports:
#   - HTTP API: 8080
#   - gRPC:     50051

job "ordo-server" {
  # Datacenter where this job runs (modify as needed for your infrastructure)
  datacenters = ["dc1"]
  
  # Service type - long-running service that should be rescheduled if it fails
  type = "service"
  
  # Update strategy for rolling deployments
  update {
    # Deploy one instance at a time
    max_parallel     = 1
    # Minimum time an instance must be healthy before marking deployment successful
    min_healthy_time = "10s"
    # Maximum time to wait for an instance to become healthy
    healthy_deadline = "3m"
    # Automatically revert to previous version if deployment fails
    auto_revert      = true
    # Number of canary instances (0 = no canary deployment)
    canary           = 0
  }
  
  # Migration configuration for draining allocations
  migrate {
    max_parallel     = 1
    health_check     = "checks"
    min_healthy_time = "10s"
    healthy_deadline = "5m"
  }
  
  # Task group - a set of tasks that run together on the same node
  group "ordo" {
    # Number of instances to run
    count = 1
    
    # Network configuration with static ports for production
    network {
      port "http" {
        static = 8080
        to     = 8080
      }
      port "grpc" {
        static = 50051
        to     = 50051
      }
    }
    
    # HTTP Service registration for service discovery (using Nomad native)
    service {
      name     = "ordo-http"
      port     = "http"
      provider = "nomad"
      tags     = ["ordo", "http", "api", "production"]
      
      # HTTP health check
      check {
        name     = "http-health"
        type     = "http"
        path     = "/health"
        interval = "10s"
        timeout  = "3s"
      }
    }
    
    # gRPC Service registration (using Nomad native)
    service {
      name     = "ordo-grpc"
      port     = "grpc"
      provider = "nomad"
      tags     = ["ordo", "grpc", "api", "production"]
      
      # TCP health check for gRPC (gRPC health check requires additional setup)
      check {
        name     = "grpc-health"
        type     = "tcp"
        interval = "10s"
        timeout  = "3s"
      }
    }
    
    # Restart policy - what to do when a task fails
    restart {
      # Number of restart attempts within the interval
      attempts = 3
      # Time window for restart attempts
      interval = "5m"
      # Delay between restarts
      delay    = "15s"
      # What to do after attempts exhausted: "fail" stops the allocation
      mode     = "fail"
    }
    
    # Task definition - the actual container to run
    task "ordo-server" {
      # Use Docker driver
      driver = "docker"
      
      config {
        # Use GitHub Container Registry image
        # Replace 'pama-lee' with your GitHub username/org if different
        image = "ghcr.io/pama-lee/ordo-server:latest"
        
        # Expose configured ports
        ports = ["http", "grpc"]
        
        # Optional: Mount volume for persistent rule storage
        # volumes = [
        #   "/data/ordo/rules:/app/rules"
        # ]
      }
      
      # Environment variables
      env {
        RUST_LOG = "info"
      }
      
      # Resource constraints
      resources {
        cpu    = 500  # MHz
        memory = 256  # MB
      }
      
      # Log rotation configuration
      logs {
        max_files     = 5
        max_file_size = 10  # MB
      }
    }
  }
}
