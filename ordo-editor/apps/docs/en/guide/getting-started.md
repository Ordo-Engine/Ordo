# Install & Run

Ordo has three parts you can use on their own: the `ordo` CLI, the `ordo-server` rule service, and the `ordo-core` Rust library. Pick the one you need.

## CLI

The CLI is enough to write rules, run tests and validate in CI. The npm package ships prebuilt binaries for Linux, macOS and Windows.

```bash
npm install -g @ordo-engine/cli
ordo --help
```

## Rule service (Docker)

```bash
docker run -p 8080:8080 ghcr.io/ordo-engine/ordo:latest
```

By default rules are kept in memory only. To load and persist rules from a directory, mount it into the container:

```bash
docker run -p 8080:8080 -v "$PWD/rules:/data/rules" \
  ghcr.io/ordo-engine/ordo:latest --rules-dir /data/rules
```

Check that the service is up:

```bash
curl http://localhost:8080/health
```

HTTP listens on 8080 and gRPC on 50051 by default. See [Server Options](/en/reference/server-options) for every flag.

## Build from source

Requires Rust 1.83 or later.

```bash
git clone https://github.com/Ordo-Engine/Ordo.git
cd Ordo
cargo build --release

./target/release/ordo-server --rules-dir ./rules
```

## Embed in a Rust program

```toml
[dependencies]
ordo-core = { git = "https://github.com/Ordo-Engine/Ordo" }
```

After loading a ruleset, use `RuleSet::from_json_compiled()` or `from_yaml_compiled()` so expressions are parsed once.

## Visual editor

To try it without installing anything, use the [online playground](https://ordo-engine.github.io/Ordo/). To run the editor locally:

```bash
cd ordo-editor
pnpm install
pnpm dev
```

## Next steps

- [Quick Start](./quick-start): write and run a first rule
- [Distributed Deployment](./distributed-deployment): running several instances
- [Kubernetes](./integration/kubernetes)
