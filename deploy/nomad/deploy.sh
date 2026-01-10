#!/bin/bash
# Ordo Server Nomad Deployment Script
#
# This script provides commands to build, deploy, and manage the Ordo server
# using HashiCorp Nomad orchestrator.
#
# Usage: ./deploy.sh <command> [environment]
#
# Commands:
#   build       - Build Docker image locally
#   deploy      - Build and deploy to Nomad
#   stop        - Stop the running job
#   restart     - Stop, rebuild, and redeploy
#   status      - Show job status
#   logs        - Stream logs from running allocation
#   help        - Show this help message
#
# Environments:
#   dev         - Development environment (dynamic ports)
#   prod        - Production environment (static ports 8080/50051)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# GitHub Container Registry configuration
GHCR_REGISTRY="ghcr.io"
GHCR_OWNER="pama-lee"
IMAGE_NAME="ordo-server"
IMAGE_TAG="${IMAGE_TAG:-latest}"
FULL_IMAGE="${GHCR_REGISTRY}/${GHCR_OWNER}/${IMAGE_NAME}:${IMAGE_TAG}"

# Console colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_step() {
    echo -e "${BLUE}[STEP]${NC} $1"
}

# Check required dependencies are installed and running
check_dependencies() {
    log_step "Checking dependencies..."
    
    # Check Docker installation
    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed. Please install Docker first."
        exit 1
    fi
    
    # Check Nomad installation
    if ! command -v nomad &> /dev/null; then
        log_error "Nomad is not installed. Please install Nomad first."
        exit 1
    fi
    
    # Check Docker daemon is running
    if ! docker info &> /dev/null; then
        log_error "Docker daemon is not running. Please start Docker."
        exit 1
    fi
    
    # Check Nomad agent is running
    if ! nomad status &> /dev/null; then
        log_error "Nomad agent is not running. Please start Nomad first."
        log_info "Hint: Run 'nomad agent -dev' for local development"
        exit 1
    fi
    
    log_info "All dependencies satisfied ✓"
}

# Build Docker image locally
build_image() {
    local use_local="${1:-false}"
    
    log_step "Building Docker image..."
    cd "$PROJECT_ROOT"
    
    if [ "$use_local" = "true" ]; then
        # Build with local tag for development
        docker build -t "${IMAGE_NAME}:latest" .
        log_info "Built local image: ${IMAGE_NAME}:latest"
    else
        # Build with GHCR tag
        docker build -t "${FULL_IMAGE}" .
        log_info "Built image: ${FULL_IMAGE}"
    fi
    
    log_info "Image build completed ✓"
}

# Pull image from GitHub Container Registry
pull_image() {
    log_step "Pulling image from GitHub Container Registry..."
    docker pull "${FULL_IMAGE}"
    log_info "Image pulled successfully ✓"
}

# Push image to GitHub Container Registry
push_image() {
    log_step "Pushing image to GitHub Container Registry..."
    
    # Check if logged in to GHCR
    if ! docker pull "${FULL_IMAGE}" &> /dev/null; then
        log_warn "You may need to login to GHCR first:"
        log_info "  echo \$GITHUB_TOKEN | docker login ghcr.io -u USERNAME --password-stdin"
    fi
    
    docker push "${FULL_IMAGE}"
    log_info "Image pushed successfully ✓"
}

# Deploy to Nomad
deploy() {
    local env="${1:-dev}"
    local job_file
    local use_local_image="${2:-false}"
    
    case "$env" in
        dev)
            job_file="$SCRIPT_DIR/ordo-server-dev.nomad"
            log_step "Deploying to DEVELOPMENT environment..."
            ;;
        prod)
            job_file="$SCRIPT_DIR/ordo-server.nomad"
            log_step "Deploying to PRODUCTION environment..."
            ;;
        *)
            log_error "Unknown environment: $env (use 'dev' or 'prod')"
            exit 1
            ;;
    esac
    
    # Validate job file syntax
    log_step "Validating job configuration..."
    if ! nomad job validate "$job_file"; then
        log_error "Job validation failed"
        exit 1
    fi
    log_info "Job configuration valid ✓"
    
    # Submit job to Nomad
    log_step "Submitting job to Nomad..."
    nomad job run "$job_file"
    
    log_info "Deployment submitted ✓"
    
    # Wait a moment and show status
    sleep 3
    echo ""
    log_info "Job Status:"
    echo "----------------------------------------"
    if [ "$env" = "dev" ]; then
        nomad job status ordo-server-dev
    else
        nomad job status ordo-server
    fi
}

# Stop a running job
stop() {
    local env="${1:-dev}"
    local job_name
    
    case "$env" in
        dev)
            job_name="ordo-server-dev"
            ;;
        prod)
            job_name="ordo-server"
            ;;
        *)
            log_error "Unknown environment: $env"
            exit 1
            ;;
    esac
    
    log_step "Stopping job: $job_name..."
    
    # Stop and purge the job (removes from Nomad entirely)
    if nomad job stop -purge "$job_name" 2>/dev/null; then
        log_info "Job stopped and purged ✓"
    else
        log_warn "Job was not running or already stopped"
    fi
}

# Show job status
status() {
    local env="${1:-dev}"
    local job_name
    
    case "$env" in
        dev) job_name="ordo-server-dev" ;;
        prod) job_name="ordo-server" ;;
        *) log_error "Unknown environment: $env"; exit 1 ;;
    esac
    
    echo ""
    log_info "Job Status: $job_name"
    echo "========================================"
    nomad job status "$job_name" 2>/dev/null || log_warn "Job not found"
}

# Stream logs from running allocation
logs() {
    local env="${1:-dev}"
    local job_name
    
    case "$env" in
        dev) job_name="ordo-server-dev" ;;
        prod) job_name="ordo-server" ;;
        *) log_error "Unknown environment: $env"; exit 1 ;;
    esac
    
    # Get the latest allocation ID for this job
    local alloc_id
    alloc_id=$(nomad job allocs -json "$job_name" 2>/dev/null | jq -r '.[0].ID' 2>/dev/null)
    
    if [ -z "$alloc_id" ] || [ "$alloc_id" = "null" ]; then
        log_error "No running allocation found for $job_name"
        exit 1
    fi
    
    log_info "Streaming logs from allocation: $alloc_id"
    echo "----------------------------------------"
    nomad alloc logs -f "$alloc_id"
}

# Show help message
show_help() {
    cat << EOF
Ordo Server Nomad Deployment Script
====================================

Usage: $0 <command> [environment]

Commands:
  build              Build Docker image locally
  pull               Pull image from GitHub Container Registry
  push               Push image to GitHub Container Registry
  deploy [env]       Deploy to Nomad (env: dev or prod, default: dev)
  stop [env]         Stop running job
  restart [env]      Stop, rebuild, and redeploy
  status [env]       Show job status
  logs [env]         Stream logs from running allocation
  help               Show this help message

Environments:
  dev                Development (dynamic ports, debug logging)
  prod               Production (static ports 8080/50051, info logging)

Examples:
  $0 build                   # Build Docker image
  $0 deploy dev              # Deploy to development environment
  $0 deploy prod             # Deploy to production environment
  $0 logs dev                # Stream logs from dev environment
  $0 stop dev                # Stop development job
  $0 restart prod            # Restart production deployment

Environment Variables:
  IMAGE_TAG          Docker image tag (default: latest)

GitHub Container Registry:
  Image: ${FULL_IMAGE}

EOF
}

# Main entry point
main() {
    local cmd="${1:-help}"
    local env="${2:-dev}"
    
    case "$cmd" in
        build)
            check_dependencies
            build_image "false"
            ;;
        build-local)
            check_dependencies
            build_image "true"
            ;;
        pull)
            check_dependencies
            pull_image
            ;;
        push)
            check_dependencies
            build_image "false"
            push_image
            ;;
        deploy)
            check_dependencies
            deploy "$env"
            ;;
        stop)
            check_dependencies
            stop "$env"
            ;;
        restart)
            check_dependencies
            stop "$env"
            sleep 2
            deploy "$env"
            ;;
        status)
            status "$env"
            ;;
        logs)
            logs "$env"
            ;;
        help|--help|-h)
            show_help
            ;;
        *)
            log_error "Unknown command: $cmd"
            echo ""
            show_help
            exit 1
            ;;
    esac
}

main "$@"
