# HashiCorp Nomad Integration

<img src="/integration/nomad.png" alt="Nomad Logo" width="120" style="margin-bottom: 20px;" />

Ordo provides comprehensive support for [HashiCorp Nomad](https://www.nomadproject.io/) integration, including job configuration files for both production and development environments.

## Overview

Nomad integration features:

- **Service Discovery**: Automatic registration for HTTP and gRPC services
- **Health Checks**: Configured HTTP (`/health`) and TCP checks
- **Rolling Updates**: Zero-downtime deployments with auto-revert
- **Resource Isolation**: CPU and memory limits configuration

## Quick Start

Use the provided deployment script in the project root:

```bash
# Deploy to development environment (dynamic ports)
./deploy/nomad/deploy.sh deploy dev

# Deploy to production environment (static ports 8080/50051)
./deploy/nomad/deploy.sh deploy prod
```

## Configuration (Job Spec)

Here is a standard production configuration example (`ordo-server.nomad`):

```hcl
job "ordo-server" {
  datacenters = ["dc1"]
  type = "service"

  group "ordo" {
    count = 1

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

    service {
      name     = "ordo-http"
      port     = "http"
      provider = "nomad"
      tags     = ["ordo", "http", "api"]

      check {
        name     = "http-health"
        type     = "http"
        path     = "/health"
        interval = "10s"
        timeout  = "3s"
      }
    }

    task "ordo-server" {
      driver = "docker"

      config {
        image = "ghcr.io/pama-lee/ordo-server:latest"
        ports = ["http", "grpc"]
      }

      resources {
        cpu    = 500
        memory = 256
      }
    }
  }
}
```

## Monitoring

Once deployed, Prometheus metrics can be scraped automatically via Nomad service discovery, or by accessing the `/metrics` endpoint directly.
