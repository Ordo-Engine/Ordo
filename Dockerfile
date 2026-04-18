# Build stage
FROM rust:1.88-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies (including protoc for gRPC)
RUN apt-get update && apt-get install -y \
    pkg-config \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build release with NATS sync support
RUN cargo build --release --package ordo-server --features nats-sync

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/ordo-server /app/ordo-server

# Create non-root user and pre-create writable data dirs so fresh named
# volumes inherit the expected ownership on first mount.
RUN useradd -r -s /bin/false ordo \
    && mkdir -p /data/rules \
    && chown -R ordo:ordo /app /data/rules
USER ordo

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Run
ENTRYPOINT ["/app/ordo-server"]
