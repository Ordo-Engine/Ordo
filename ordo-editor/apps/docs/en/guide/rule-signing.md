# Rule Signing

Ordo supports digital signatures for rules to ensure integrity and prevent tampering. This feature uses Ed25519 asymmetric cryptography to sign and verify rules across all formats (JSON, YAML, and compiled `.ordo` binary).

## Overview

Rule signing provides:

- **Tamper Protection**: Detect any unauthorized modifications to rules
- **Origin Verification**: Verify that rules come from trusted sources
- **Audit Trail**: Track who signed rules and when
- **Multi-format Support**: Works with JSON, YAML, and compiled binary formats

## Quick Start

### 1. Generate a Key Pair

```bash
ordo-keygen --output-dir ./keys
```

This creates two files:

- `keys/public.key` - Share this with servers that need to verify rules
- `keys/private.key` - Keep this secret, used for signing rules

### 2. Sign a Rule File

```bash
ordo-sign --private-key ./keys/private.key --input rules/my-rule.json
```

The signed rule will have a `_signature` field added:

```json
{
  "config": {
    "name": "my-rule",
    "entry_step": "start"
  },
  "steps": { ... },
  "_signature": {
    "algorithm": "ed25519",
    "public_key": "base64-encoded-public-key",
    "signature": "base64-encoded-signature",
    "signed_at": "2026-01-25T10:30:00Z"
  }
}
```

### 3. Verify a Rule File

```bash
ordo-verify --public-key ./keys/public.key --input rules/my-rule.json
```

### 4. Configure Server Verification

```bash
ordo-server \
  --signature-enabled \
  --signature-require \
  --signature-trusted-keys "base64-public-key-1,base64-public-key-2"
```

Or using environment variables:

```bash
export ORDO_SIGNATURE_ENABLED=true
export ORDO_SIGNATURE_REQUIRE=true
export ORDO_SIGNATURE_TRUSTED_KEYS_FILE=/etc/ordo/trusted-keys.txt
```

## CLI Tools

### ordo-keygen

Generate Ed25519 key pairs for signing rules.

```bash
ordo-keygen [OPTIONS]

Options:
  -o, --output-dir <DIR>    Output directory for key files [default: .]
  -p, --prefix <PREFIX>     Prefix for key file names [default: ""]
  -h, --help                Print help
```

**Examples:**

```bash
# Generate keys in current directory
ordo-keygen

# Generate keys with custom prefix
ordo-keygen --output-dir ./keys --prefix production-
# Creates: keys/production-public.key, keys/production-private.key
```

### ordo-sign

Sign rule files with a private key.

```bash
ordo-sign [OPTIONS] --private-key <FILE> --input <FILE>

Options:
  -k, --private-key <FILE>  Path to private key file
  -i, --input <FILE>        Input rule file (JSON, YAML, or .ordo)
  -o, --output <FILE>       Output file (defaults to overwriting input)
  -h, --help                Print help
```

**Examples:**

```bash
# Sign a JSON rule file (in-place)
ordo-sign -k private.key -i rule.json

# Sign and output to a new file
ordo-sign -k private.key -i rule.json -o rule-signed.json

# Sign a YAML file
ordo-sign -k private.key -i rule.yaml

# Sign a compiled .ordo binary
ordo-sign -k private.key -i rule.ordo
```

### ordo-verify

Verify rule file signatures.

```bash
ordo-verify [OPTIONS] --public-key <FILE> --input <FILE>

Options:
  -k, --public-key <FILE>   Path to public key file
  -i, --input <FILE>        Input rule file to verify
  -h, --help                Print help
```

**Examples:**

```bash
# Verify a signed rule
ordo-verify -k public.key -i rule.json

# Output on success:
# ✓ Signature valid for rule.json

# Output on failure:
# ✗ Signature verification failed: Invalid signature
```

## Signature Formats

### JSON/YAML Rules

For JSON and YAML files, the signature is embedded as a `_signature` field:

```json
{
  "config": { ... },
  "steps": { ... },
  "_signature": {
    "algorithm": "ed25519",
    "public_key": "MCowBQYDK2VwAyEA...",
    "signature": "MEUCIQD...",
    "signed_at": "2026-01-25T10:30:00Z"
  }
}
```

The signature is computed over the **canonical JSON** representation of the rule (excluding the `_signature` field), with keys sorted alphabetically.

### Compiled Binary (.ordo)

For compiled `.ordo` files, the signature is stored in the binary header:

```
┌─────────────────────────────────────┐
│ Magic: "ORDO" (4 bytes)             │
│ Version: 1 (1 byte)                 │
│ Flags: HAS_SIGNATURE (1 byte)       │
│ Checksum: CRC32 (4 bytes)           │
│ Payload Length (4 bytes)            │
├─────────────────────────────────────┤
│ Signature Block (if flag set):      │
│   - Public Key (32 bytes)           │
│   - Signature (64 bytes)            │
├─────────────────────────────────────┤
│ Compressed Payload (zstd)           │
└─────────────────────────────────────┘
```

## Server Configuration

### Configuration Options

| Option                             | Environment Variable                  | Description                                 |
| ---------------------------------- | ------------------------------------- | ------------------------------------------- |
| `--signature-enabled`              | `ORDO_SIGNATURE_ENABLED`              | Enable signature verification               |
| `--signature-require`              | `ORDO_SIGNATURE_REQUIRE`              | Require all rules to be signed              |
| `--signature-trusted-keys`         | `ORDO_SIGNATURE_TRUSTED_KEYS`         | Comma-separated list of trusted public keys |
| `--signature-trusted-keys-file`    | `ORDO_SIGNATURE_TRUSTED_KEYS_FILE`    | Path to file containing trusted keys        |
| `--signature-allow-unsigned-local` | `ORDO_SIGNATURE_ALLOW_UNSIGNED_LOCAL` | Allow unsigned rules from local files       |

### Verification Modes

**1. Disabled (Default)**

```bash
ordo-server
# No signature verification
```

**2. Optional Verification**

```bash
ordo-server \
  --signature-enabled \
  --signature-trusted-keys "key1,key2"
# Verifies signed rules, accepts unsigned rules
```

**3. Required Verification**

```bash
ordo-server \
  --signature-enabled \
  --signature-require \
  --signature-trusted-keys "key1,key2"
# Rejects all unsigned rules
```

**4. Local Files Exempt**

```bash
ordo-server \
  --signature-enabled \
  --signature-require \
  --signature-allow-unsigned-local \
  --signature-trusted-keys "key1,key2"
# Requires signatures for API uploads, allows unsigned local files
```

### Trusted Keys File Format

Create a file with one base64-encoded public key per line:

```text
# Production signing key
MCowBQYDK2VwAyEAabc123...

# CI/CD signing key
MCowBQYDK2VwAyEAxyz789...
```

## API Integration

### HTTP API

When pushing rules via the HTTP API, you can include the signature in two ways:

**1. Embedded in Body**

```bash
curl -X POST http://localhost:8080/api/v1/rulesets \
  -H "Content-Type: application/json" \
  -d '{
    "config": { "name": "my-rule", "entry_step": "start" },
    "steps": { ... },
    "_signature": {
      "algorithm": "ed25519",
      "public_key": "...",
      "signature": "..."
    }
  }'
```

**2. Via HTTP Headers**

```bash
curl -X POST http://localhost:8080/api/v1/rulesets \
  -H "Content-Type: application/json" \
  -H "X-Ordo-Signature: base64-signature" \
  -H "X-Ordo-Public-Key: base64-public-key" \
  -d '{
    "config": { "name": "my-rule", "entry_step": "start" },
    "steps": { ... }
  }'
```

### Error Responses

When signature verification fails:

```json
{
  "error": "Forbidden",
  "message": "Signature verification failed: Invalid signature"
}
```

## Security Best Practices

### Key Management

1. **Protect Private Keys**: Store private keys securely, never commit to version control
2. **Use Separate Keys**: Use different keys for different environments (dev, staging, prod)
3. **Rotate Keys**: Periodically rotate signing keys and update trusted keys on servers
4. **Audit Key Usage**: Log all signing operations for audit purposes

### Deployment

1. **Enable in Production**: Always enable signature verification in production
2. **Require Signatures**: Use `--signature-require` to reject unsigned rules
3. **Minimal Trust**: Only add necessary public keys to the trusted list
4. **Secure Key Distribution**: Use secure channels to distribute public keys

### CI/CD Integration

```yaml
# Example GitHub Actions workflow
- name: Sign Rules
  run: |
    echo "${{ secrets.ORDO_PRIVATE_KEY }}" > /tmp/private.key
    for rule in rules/*.json; do
      ordo-sign -k /tmp/private.key -i "$rule"
    done
    rm /tmp/private.key

- name: Deploy Rules
  run: |
    for rule in rules/*.json; do
      curl -X POST "$ORDO_SERVER/api/v1/rulesets" \
        -H "Content-Type: application/json" \
        -d @"$rule"
    done
```

## Troubleshooting

### Common Issues

**"Signature verification failed: No signature found"**

- The rule file doesn't contain a signature
- If using headers, ensure both `X-Ordo-Signature` and `X-Ordo-Public-Key` are set

**"Signature verification failed: Untrusted public key"**

- The public key in the signature is not in the server's trusted keys list
- Add the key to `--signature-trusted-keys` or the trusted keys file

**"Signature verification failed: Invalid signature"**

- The rule content has been modified after signing
- Re-sign the rule with the private key

**"Invalid public key base64"**

- The public key is not properly base64 encoded
- Ensure you're using the content of the `.key` file, not the file path
