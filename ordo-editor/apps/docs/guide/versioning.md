# Version Management

Ordo automatically maintains historical versions of your rules, enabling rollback to previous states.

## Enable Versioning

Versioning requires persistence to be enabled:

```bash
ordo-server --rules-dir ./rules --max-versions 10
```

| Flag             | Default | Description                       |
| ---------------- | ------- | --------------------------------- |
| `--rules-dir`    | None    | Directory for rule storage        |
| `--max-versions` | 10      | Maximum versions to keep per rule |

## How It Works

1. When a rule is **updated**, the current version is saved to `.versions/`
2. Old versions beyond `--max-versions` are automatically deleted
3. Versions can be listed and restored via API

## Version Storage

```
rules/
├── discount-check.json          # Current version
└── .versions/
    └── discount-check/
        ├── v1_1704700000.json   # seq=1, timestamp
        ├── v2_1704800000.json   # seq=2, timestamp
        └── v3_1704900000.json   # seq=3, timestamp
```

## List Versions

```bash
curl http://localhost:8080/api/v1/rulesets/discount-check/versions
```

Response:

```json
{
  "name": "discount-check",
  "current_version": "2.0.0",
  "versions": [
    {
      "seq": 3,
      "version": "1.5.0",
      "timestamp": "2024-01-08T10:00:00Z"
    },
    {
      "seq": 2,
      "version": "1.2.0",
      "timestamp": "2024-01-07T15:30:00Z"
    },
    {
      "seq": 1,
      "version": "1.0.0",
      "timestamp": "2024-01-06T09:00:00Z"
    }
  ]
}
```

## Rollback

Restore a rule to a previous version:

```bash
curl -X POST http://localhost:8080/api/v1/rulesets/discount-check/rollback \
  -H "Content-Type: application/json" \
  -d '{"seq": 2}'
```

Response:

```json
{
  "status": "rolled_back",
  "name": "discount-check",
  "from_version": "2.0.0",
  "to_version": "1.2.0"
}
```

::: warning
Rollback creates a new version entry. The rolled-back-from version is preserved in history.
:::

## Version Cleanup

Versions beyond `--max-versions` are automatically deleted (oldest first):

```bash
# Keep only last 5 versions
ordo-server --rules-dir ./rules --max-versions 5
```

## Audit Trail

When [audit logging](/guide/audit-logging) is enabled, version changes are recorded:

```json
{"event":"rule_updated","rule_name":"discount-check","from_version":"1.0.0","to_version":"2.0.0"}
{"event":"rule_rollback","rule_name":"discount-check","from_version":"2.0.0","to_version":"1.0.0","seq":1}
```

## Best Practices

1. **Set appropriate max-versions**: Balance storage vs. rollback depth
2. **Use semantic versioning**: `1.0.0` → `1.0.1` → `1.1.0` → `2.0.0`
3. **Document changes**: Use meaningful version numbers
4. **Test before rollback**: Verify the target version works correctly
5. **Monitor version growth**: Watch disk usage in production

## Memory-Only Mode

Without `--rules-dir`, versioning is not available:

```bash
curl http://localhost:8080/api/v1/rulesets/my-rule/versions
```

```json
{
  "name": "my-rule",
  "current_version": "1.0.0",
  "versions": []
}
```

Rollback returns an error:

```json
{
  "error": "Version rollback not available in memory-only mode"
}
```
