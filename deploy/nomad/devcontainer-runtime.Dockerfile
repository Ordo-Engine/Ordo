FROM debian:bookworm-slim

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    protobuf-compiler \
    git \
    curl \
    wget \
    ca-certificates \
    procps \
    zsh \
    sudo \
    netcat-openbsd \
    make \
    unzip \
    xz-utils \
    python3 \
    python3-pip \
    jq \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd -g 1001 vscode \
    && useradd -m -u 1001 -g 1001 -s /bin/bash -G sudo vscode \
    && echo "vscode ALL=(ALL) NOPASSWD:ALL" > /etc/sudoers.d/vscode \
    && chmod 0440 /etc/sudoers.d/vscode \
    && mkdir -p /data/platform /data/rules /opt/ordo \
    && chown -R vscode:vscode /data /opt/ordo

RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get update && apt-get install -y nodejs \
    && npm install -g pnpm \
    && corepack enable \
    && rm -rf /var/lib/apt/lists/*

USER vscode
ENV PATH=/home/vscode/.cargo/bin:/home/vscode/.local/share/pnpm:${PATH}
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --profile default --default-toolchain stable \
    && rustup target add wasm32-unknown-unknown \
    && cargo install cargo-watch wasm-pack

COPY --chown=vscode:vscode deploy/nomad/devcontainer-entrypoint.sh /opt/ordo/devcontainer-entrypoint.sh

WORKDIR /workspace

CMD ["/opt/ordo/devcontainer-entrypoint.sh"]
