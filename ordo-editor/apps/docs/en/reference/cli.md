# CLI Options

Complete reference for `ordo-server` command-line options.

## Usage

```bash
ordo-server [OPTIONS]
```

## Server Options

### --http-addr

HTTP server bind address.

```bash
ordo-server --http-addr 0.0.0.0:8080
```

|             |                |
| ----------- | -------------- |
| **Default** | `0.0.0.0:8080` |
| **Format**  | `host:port`    |

### --grpc-addr

gRPC server bind address.

```bash
ordo-server --grpc-addr 0.0.0.0:50051
```

|             |                 |
| ----------- | --------------- |
| **Default** | `0.0.0.0:50051` |
| **Format**  | `host:port`     |

### --uds-path

Unix Domain Socket path (Unix only).

```bash
ordo-server --uds-path /tmp/ordo.sock
```

|             |                 |
| ----------- | --------------- |
| **Default** | None (disabled) |
| **Format**  | File path       |

### --disable-http

Disable HTTP server.

```bash
ordo-server --disable-http
```

|             |         |
| ----------- | ------- |
| **Default** | `false` |

### --disable-grpc

Disable gRPC server.

```bash
ordo-server --disable-grpc
```

|             |         |
| ----------- | ------- |
| **Default** | `false` |

## Storage Options

### --rules-dir

Directory for rule persistence.

```bash
ordo-server --rules-dir ./rules
```

|             |                       |
| ----------- | --------------------- |
| **Default** | None (in-memory only) |
| **Format**  | Directory path        |

When specified:

- Rules are loaded from this directory on startup
- Rules are saved here when created/updated via API
- Rules are deleted from here when removed via API
- Supports `.json`, `.yaml`, `.yml` files

### --max-versions

Maximum historical versions to keep per rule.

```bash
ordo-server --rules-dir ./rules --max-versions 10
```

|              |               |
| ------------ | ------------- |
| **Default**  | `10`          |
| **Range**    | 1 - unlimited |
| **Requires** | `--rules-dir` |

## Audit Options

### --audit-dir

Directory for audit log files.

```bash
ordo-server --audit-dir ./audit
```

|             |                    |
| ----------- | ------------------ |
| **Default** | None (stdout only) |
| **Format**  | Directory path     |

When specified:

- Audit events are written to JSON Lines files
- Files are rotated daily (`audit-YYYY-MM-DD.jsonl`)
- Events are also logged to stdout

### --audit-sample-rate

Execution log sampling rate (percentage).

```bash
ordo-server --audit-sample-rate 10
```

|             |         |
| ----------- | ------- |
| **Default** | `10`    |
| **Range**   | 0 - 100 |

- `0` = No execution logging
- `100` = Log all executions
- Can be changed at runtime via API

## Signature Options

### --signature-enabled

Enable signature verification for rule updates and loads.

```bash
ordo-server --signature-enabled
```

|             |         |
| ----------- | ------- |
| **Default** | `false` |

### --signature-require

Reject unsigned rules on API updates.

```bash
ordo-server --signature-enabled --signature-require
```

|             |         |
| ----------- | ------- |
| **Default** | `false` |

### --signature-trusted-keys

Comma-separated base64 public keys.

```bash
ordo-server --signature-enabled --signature-trusted-keys "BASE64_KEY_1,BASE64_KEY_2"
```

### --signature-trusted-keys-file

File with one base64 public key per line.

```bash
ordo-server --signature-enabled --signature-trusted-keys-file /etc/ordo/trusted_keys.txt
```

### --signature-allow-unsigned-local

Allow unsigned local files on startup.

```bash
ordo-server --signature-enabled --signature-allow-unsigned-local false
```

|             |        |
| ----------- | ------ |
| **Default** | `true` |

## Logging Options

### --log-level

Log verbosity level.

```bash
ordo-server --log-level debug
```

|             |                                           |
| ----------- | ----------------------------------------- |
| **Default** | `info`                                    |
| **Values**  | `trace`, `debug`, `info`, `warn`, `error` |

## Examples

### Development

```bash
# Simple in-memory server
ordo-server

# With debug logging
ordo-server --log-level debug
```

### Production

```bash
# Full-featured setup
ordo-server \
  --http-addr 0.0.0.0:8080 \
  --grpc-addr 0.0.0.0:50051 \
  --rules-dir /var/lib/ordo/rules \
  --max-versions 20 \
  --audit-dir /var/log/ordo/audit \
  --audit-sample-rate 10 \
  --log-level info
```

### HTTP Only

```bash
ordo-server --disable-grpc --http-addr 0.0.0.0:8080
```

### gRPC Only

```bash
ordo-server --disable-http --grpc-addr 0.0.0.0:50051
```

### Unix Domain Socket

```bash
ordo-server --uds-path /var/run/ordo.sock --disable-http --disable-grpc
```

## Environment Variables

Ordo supports environment variables using the `ORDO_*` prefix. See the configuration reference for the full list.

## Signature CLI Tools

### ordo-keygen

Generate an Ed25519 keypair:

```bash
ordo-keygen --output ./keys
```

### ordo-sign

Sign JSON/YAML/.ordo files:

```bash
ordo-sign --key ./keys/private.key --input rule.json
```

### ordo-verify

Verify signatures for JSON/YAML/.ordo files:

```bash
ordo-verify --key ./keys/public.key --input rule.signed.json
```

## Help

```bash
ordo-server --help
ordo-server --version
```
