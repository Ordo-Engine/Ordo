# Ordo Server Nomad Deployment

This directory contains HashiCorp Nomad job configurations for deploying the Ordo Rule Engine Server.

## Prerequisites

1. **Docker**: Installed and running
2. **Nomad**: Installed and running (listening on http://127.0.0.1:4646)

### Verify Nomad Status

```bash
# Check if Nomad is running
nomad status

# View node information
nomad node status

# Start Nomad in dev mode (for local development)
nomad agent -dev
```

## Files

| File | Description |
|------|-------------|
| `ordo-server.nomad` | Production job config (static ports 8080/50051) |
| `ordo-server-dev.nomad` | Development job config (dynamic ports) |
| `deploy.sh` | Deployment automation script |

## Quick Start

### Option 1: Using the Deployment Script (Recommended)

```bash
# Make script executable
chmod +x deploy.sh

# Deploy to development environment
./deploy.sh deploy dev

# Deploy to production environment
./deploy.sh deploy prod

# View job status
./deploy.sh status dev

# Stream logs
./deploy.sh logs dev

# Stop the service
./deploy.sh stop dev

# Restart (stop + redeploy)
./deploy.sh restart prod
```

### Option 2: Manual Deployment

```bash
# 1. Pull image from GitHub Container Registry
docker pull ghcr.io/pama-lee/ordo-server:latest

# 2. Validate job configuration
nomad job validate deploy/nomad/ordo-server.nomad

# 3. Deploy to Nomad
nomad job run deploy/nomad/ordo-server.nomad

# 4. Check status
nomad job status ordo-server
```

### Option 3: Build Locally

```bash
# Build Docker image from source
cd /path/to/Ordo
docker build -t ordo-server:latest .

# Deploy using local image (edit .nomad file to use ordo-server:latest)
nomad job run deploy/nomad/ordo-server.nomad
```

## Port Configuration

### Production Environment (`ordo-server.nomad`)

| Service | Port | Description |
|---------|------|-------------|
| HTTP API | 8080 | RESTful API endpoints |
| gRPC | 50051 | gRPC service |

### Development Environment (`ordo-server-dev.nomad`)

Uses dynamic port allocation. Check assigned ports with:

```bash
nomad job status ordo-server-dev

# Or get specific allocation details
nomad alloc status <alloc-id>
```

## Service Discovery

Jobs automatically register services with Nomad's service discovery:

- **Production**: `ordo-http`, `ordo-grpc`
- **Development**: `ordo-dev-http`, `ordo-dev-grpc`

### Querying Services

```bash
# List registered services
nomad service list

# Get service info
nomad service info ordo-http
```

## Health Checks

| Service | Type | Endpoint | Interval |
|---------|------|----------|----------|
| HTTP | HTTP | `GET /health` | 10s |
| gRPC | TCP | Port connectivity | 10s |

## Resource Allocation

### Production
- **CPU**: 500 MHz
- **Memory**: 256 MB

### Development
- **CPU**: 200 MHz
- **Memory**: 128 MB

Adjust the `resources` block in the job files based on your workload requirements.

## Troubleshooting

### Common Commands

```bash
# View job status and allocations
nomad job status ordo-server

# Get detailed allocation info
nomad alloc status <alloc-id>

# View stdout/stderr logs
nomad alloc logs <alloc-id>
nomad alloc logs -stderr <alloc-id>

# Follow logs in real-time
nomad alloc logs -f <alloc-id>

# Execute command in container
nomad alloc exec -i -t <alloc-id> /bin/sh

# Force garbage collection
nomad system gc
```

### Common Issues

**1. Image Pull Failures**

```bash
# Check if you can pull the image
docker pull ghcr.io/pama-lee/ordo-server:latest

# For private repos, ensure Docker is authenticated
echo $GITHUB_TOKEN | docker login ghcr.io -u YOUR_USERNAME --password-stdin
```

**2. Port Already in Use**

```bash
# Check what's using the port
lsof -i :8080
lsof -i :50051

# Use development config with dynamic ports instead
./deploy.sh deploy dev
```

**3. Job Stuck in Pending**

```bash
# Check node status and available resources
nomad node status -self

# Check allocation events
nomad alloc status <alloc-id>
```

## Verify Deployment

```bash
# Check HTTP health endpoint
curl http://localhost:8080/health

# Expected response:
# {"status":"healthy","version":"0.1.0"}

# Test expression evaluation
curl -X POST http://localhost:8080/api/v1/eval \
  -H "Content-Type: application/json" \
  -d '{"expression": "1 + 2 * 3"}'

# Expected response:
# {"result":7}
```

## Environment Variables

The Ordo server accepts these environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Log level (trace, debug, info, warn, error) |
| `HTTP_ADDR` | `0.0.0.0:8080` | HTTP server bind address |
| `GRPC_ADDR` | `0.0.0.0:50051` | gRPC server bind address |

Modify the `env` block in the job files to customize these settings.

## Scaling

To run multiple instances (horizontal scaling):

```hcl
group "ordo" {
  count = 3  # Run 3 instances
  
  # ... rest of configuration
}
```

Note: When scaling, use dynamic ports to avoid port conflicts, or deploy behind a load balancer.
