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
| `k6-load-test.nomad` | k6 load testing batch job |
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

## Rule Persistence

The Ordo server supports file-based rule persistence via the `--rules-dir` flag:

```bash
# Enable persistence with a directory
ordo-server --rules-dir /var/lib/ordo/rules
```

**Behavior:**
- **On startup**: All `.json`, `.yaml`, `.yml` files in the directory are loaded as rules
- **On API create/update**: Rules are saved to the directory as JSON files
- **On API delete**: Rule files are removed from the directory
- **Without `--rules-dir`**: Pure in-memory mode (rules lost on restart)

**For Nomad deployments**, you can use a host volume to persist rules:

```hcl
group "ordo" {
  volume "rules" {
    type   = "host"
    source = "ordo-rules"
  }

  task "server" {
    volume_mount {
      volume      = "rules"
      destination = "/var/lib/ordo/rules"
    }

    config {
      args = ["--rules-dir", "/var/lib/ordo/rules"]
    }
  }
}
```

## Scaling

To run multiple instances (horizontal scaling):

```hcl
group "ordo" {
  count = 3  # Run 3 instances
  
  # ... rest of configuration
}
```

Note: When scaling, use dynamic ports to avoid port conflicts, or deploy behind a load balancer.

## Load Testing with k6

The `k6-load-test.nomad` job provides a convenient way to run load tests against the Ordo server using [k6](https://k6.io/).

### Basic Usage

```bash
# Run with default settings (health endpoint, 10 VUs, 30s)
nomad job run deploy/nomad/k6-load-test.nomad

# Check test progress
nomad job status k6-load-test
```

### Custom Parameters

The load test job supports various parameters via `-var` flags:

```bash
# Test the execute endpoint with 50 VUs for 1 minute
nomad job run \
  -var="target_endpoint=execute" \
  -var="vus=50" \
  -var="duration=1m" \
  -var="ruleset_name=my-ruleset" \
  deploy/nomad/k6-load-test.nomad

# Test expression evaluation with custom input
nomad job run \
  -var="target_endpoint=eval" \
  -var="vus=20" \
  -var="duration=30s" \
  -var='expression=x + y * 2' \
  -var='context_json={"x": 10, "y": 20}' \
  deploy/nomad/k6-load-test.nomad

# High-load test with rate limiting
nomad job run \
  -var="target_endpoint=health" \
  -var="vus=100" \
  -var="duration=5m" \
  -var="rps=1000" \
  deploy/nomad/k6-load-test.nomad
```

### Available Endpoints

| Endpoint | Description | Required Parameters |
|----------|-------------|---------------------|
| `health` | GET /health | - |
| `list` | GET /api/v1/rulesets | - |
| `get_ruleset` | GET /api/v1/rulesets/:name | `ruleset_name` |
| `execute` | POST /api/v1/execute/:name | `ruleset_name`, `input_json` |
| `eval` | POST /api/v1/eval | `expression`, `context_json` |

### All Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `target_endpoint` | `health` | Endpoint to test |
| `target_url` | (auto-discovery) | Base URL of Ordo server |
| `vus` | `10` | Number of virtual users |
| `duration` | `30s` | Test duration (e.g., 30s, 1m, 5m) |
| `rps` | `0` | Target requests per second (0 = unlimited) |
| `ruleset_name` | `demo` | RuleSet name for execute/get endpoints |
| `expression` | `1 + 1` | Expression for eval endpoint |
| `input_json` | `{"amount": 1000, "score": 75}` | Input JSON for execute |
| `context_json` | `{"x": 10, "y": 20}` | Context JSON for eval |
| `thresholds_p95` | `500` | P95 response time threshold (ms) |
| `thresholds_p99` | `1000` | P99 response time threshold (ms) |
| `thresholds_error_rate` | `0.01` | Max allowed error rate |

### View Test Results

```bash
# Get allocation ID
ALLOC_ID=$(nomad job status k6-load-test | grep -E "^[a-f0-9]+" | head -1 | awk '{print $1}')

# View k6 output logs
nomad alloc logs $ALLOC_ID load-test

# View detailed results (JSON)
nomad alloc logs $ALLOC_ID collect-results
```

### Parameterized Job Dispatch

You can also dispatch the job with meta parameters:

```bash
# Dispatch with custom parameters
nomad job dispatch -meta target_endpoint=execute -meta vus=30 k6-load-test
```
