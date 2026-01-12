# Getting Started

This guide will help you get Ordo up and running quickly.

## Prerequisites

- **Rust**: 1.83 or later
- **Node.js**: 18 or later (for visual editor)
- **pnpm**: 8 or later (for visual editor)

## Installation

### Clone the Repository

```bash
git clone https://github.com/Pama-Lee/Ordo.git
cd Ordo
```

### Build the Server

```bash
cargo build --release
```

The compiled binary will be at `./target/release/ordo-server`.

### Run the Server

```bash
# Start with default settings (HTTP on 8080, gRPC on 50051)
./target/release/ordo-server

# Or with persistence enabled
./target/release/ordo-server --rules-dir ./rules
```

## Verify Installation

Check the health endpoint:

```bash
curl http://localhost:8080/health
```

Expected response:

```json
{
  "status": "healthy",
  "version": "0.1.0",
  "uptime_seconds": 5,
  "storage": {
    "mode": "memory",
    "rules_count": 0
  }
}
```

## Visual Editor

To use the visual rule editor:

```bash
cd ordo-editor
pnpm install
pnpm dev
```

Open `http://localhost:3001` in your browser.

Or try the [online playground](https://pama-lee.github.io/Ordo/).

## Docker

```bash
# Build the image
docker build -t ordo-server .

# Run with persistence
docker run -p 8080:8080 -v ./rules:/rules ordo-server --rules-dir /rules
```

## Next Steps

- [Quick Start](./quick-start) - Create and execute your first rule
- [Rule Structure](./rule-structure) - Understand how rules are defined
- [Expression Syntax](./expression-syntax) - Learn the expression language
