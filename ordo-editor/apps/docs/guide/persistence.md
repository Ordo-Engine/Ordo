# Rule Persistence

By default, Ordo stores rules in memory. Enable file-based persistence to retain rules across restarts.

## Enable Persistence

Use the `--rules-dir` flag to specify a directory for rule storage:

```bash
ordo-server --rules-dir ./rules
```

## How It Works

When persistence is enabled:

1. **Startup**: Rules are loaded from `.json`, `.yaml`, and `.yml` files in the directory
2. **Create/Update**: Rules are saved to the directory when created or updated via API
3. **Delete**: Rule files are removed when deleted via API

## Directory Structure

```
rules/
├── discount-check.json
├── loan-approval.yaml
├── fraud-detection.json
└── .versions/
    ├── discount-check/
    │   ├── v1_1704700000.json
    │   └── v2_1704800000.json
    └── loan-approval/
        └── v1_1704700000.yaml
```

## File Formats

### JSON Format

```json
{
  "config": {
    "name": "discount-check",
    "version": "1.0.0",
    "entry_step": "check_vip"
  },
  "steps": {
    "check_vip": {
      "id": "check_vip",
      "type": "decision",
      "branches": [{ "condition": "user.vip == true", "next_step": "vip_discount" }],
      "default_next": "normal_discount"
    }
  }
}
```

### YAML Format

```yaml
config:
  name: discount-check
  version: '1.0.0'
  entry_step: check_vip

steps:
  check_vip:
    id: check_vip
    type: decision
    branches:
      - condition: 'user.vip == true'
        next_step: vip_discount
    default_next: normal_discount

  vip_discount:
    id: vip_discount
    type: terminal
    result:
      code: VIP
      message: '20% discount applied'
```

## File Naming

- File name becomes the rule name (without extension)
- `discount-check.json` → rule name: `discount-check`
- If multiple files have the same base name, JSON takes priority over YAML

## Health Check

The health endpoint shows storage mode:

```bash
curl http://localhost:8080/health
```

```json
{
  "status": "healthy",
  "version": "0.1.0",
  "storage": {
    "mode": "persistent",
    "rules_dir": "./rules",
    "rules_count": 3
  }
}
```

## Best Practices

1. **Use version control**: Keep your rules directory in Git
2. **Organize by domain**: Use subdirectories for different rule categories
3. **Prefer YAML for readability**: YAML is easier to read and edit manually
4. **Use JSON for programmatic access**: JSON is better for API responses
5. **Backup regularly**: Implement backup strategies for production

## Troubleshooting

### Rules not loading

- Check file permissions
- Verify JSON/YAML syntax
- Check server logs for parsing errors

### Changes not persisting

- Verify `--rules-dir` is specified
- Check directory write permissions
- Ensure disk space is available

### Startup errors

```bash
# Check for syntax errors
cat rules/my-rule.json | jq .

# Validate YAML
python -c "import yaml; yaml.safe_load(open('rules/my-rule.yaml'))"
```
