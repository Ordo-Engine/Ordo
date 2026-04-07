# Ordo Dev Container
# Includes: Rust stable, Node 20, pnpm, protobuf, all build deps
# Note: devcontainer features (rust, node) are layered on top by devcontainer.json
# This base image just provides the OS + system packages.

FROM debian:bookworm-slim

# Avoid prompts from apt
ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y \
    # Build essentials
    build-essential \
    pkg-config \
    # Protobuf compiler (for gRPC)
    protobuf-compiler \
    # Useful dev tools
    git \
    curl \
    wget \
    ca-certificates \
    # Process management (for running multiple services)
    procps \
    # Shell
    zsh \
    sudo \
    # For health checks
    netcat-openbsd \
    && rm -rf /var/lib/apt/lists/*

# Create vscode user (devcontainers convention)
RUN useradd -m -s /bin/zsh -G sudo vscode \
    && echo "vscode ALL=(ALL) NOPASSWD:ALL" > /etc/sudoers.d/vscode \
    && chmod 0440 /etc/sudoers.d/vscode

# Data directories for backend services (owned by vscode so no sudo needed in dev)
RUN mkdir -p /data/platform /data/rules \
    && chown -R vscode:vscode /data

USER vscode
WORKDIR /workspace
