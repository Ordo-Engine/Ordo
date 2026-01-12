# Audit Logging

Ordo provides structured audit logging to track rule changes, executions, and system events.

## Enable Audit Logging

```bash
ordo-server --audit-dir ./audit --audit-sample-rate 10
```

| Flag | Default | Description |
|------|---------|-------------|
| `--audit-dir` | None | Directory for audit log files |
| `--audit-sample-rate` | 10 | Execution sampling rate (0-100%) |

## Log Output

Audit events are written to:

1. **stdout** - Always (via tracing)
2. **File** - When `--audit-dir` is specified

## File Format

Logs are written as JSON Lines with daily rotation:

```
audit/
├── audit-2024-01-08.jsonl
├── audit-2024-01-09.jsonl
└── audit-2024-01-10.jsonl
```

Each line is a complete JSON object:

```json
{"timestamp":"2024-01-08T10:00:00.123Z","level":"INFO","event":"server_started","version":"0.1.0","rules_count":12}
{"timestamp":"2024-01-08T10:00:01.456Z","level":"INFO","event":"rule_created","rule_name":"payment-check","version":"1.0.0","source_ip":"127.0.0.1"}
{"timestamp":"2024-01-08T10:00:02.789Z","level":"INFO","event":"rule_executed","rule_name":"payment-check","duration_us":1500,"result":"success"}
```

## Event Types

### System Events

| Event | Description | Fields |
|-------|-------------|--------|
| `server_started` | Server startup | `version`, `rules_count` |
| `server_stopped` | Server shutdown | `uptime_seconds` |

### Rule Change Events

| Event | Description | Fields |
|-------|-------------|--------|
| `rule_created` | New rule created | `rule_name`, `version`, `source_ip` |
| `rule_updated` | Rule updated | `rule_name`, `from_version`, `to_version`, `source_ip` |
| `rule_deleted` | Rule deleted | `rule_name`, `source_ip` |
| `rule_rollback` | Version rollback | `rule_name`, `from_version`, `to_version`, `seq`, `source_ip` |

### Execution Events

| Event | Description | Fields |
|-------|-------------|--------|
| `rule_executed` | Rule execution (sampled) | `rule_name`, `duration_us`, `result`, `source_ip` |

### Configuration Events

| Event | Description | Fields |
|-------|-------------|--------|
| `sample_rate_changed` | Sampling rate updated | `from_rate`, `to_rate`, `source_ip` |

## Execution Sampling

To avoid overwhelming logs in high-throughput scenarios, execution events are sampled:

- `--audit-sample-rate 10` → Log ~10% of executions
- `--audit-sample-rate 100` → Log all executions
- `--audit-sample-rate 0` → Disable execution logging

### Dynamic Sample Rate

Update the sampling rate at runtime without restart:

```bash
# Get current rate
curl http://localhost:8080/api/v1/config/audit-sample-rate
```

```json
{"sample_rate": 10}
```

```bash
# Update to 50%
curl -X PUT http://localhost:8080/api/v1/config/audit-sample-rate \
  -H "Content-Type: application/json" \
  -d '{"sample_rate": 50}'
```

```json
{"sample_rate": 50, "previous": 10}
```

## Use Cases

### Compliance Auditing

Track who changed what and when:

```bash
grep "rule_updated\|rule_created\|rule_deleted" audit/audit-2024-01-08.jsonl
```

### Performance Analysis

Analyze execution times:

```bash
grep "rule_executed" audit/audit-2024-01-08.jsonl | \
  jq -r '.duration_us' | \
  awk '{sum+=$1; count++} END {print "avg:", sum/count, "us"}'
```

### Error Investigation

Find failed executions:

```bash
grep '"result":"error"' audit/audit-2024-01-08.jsonl
```

### Security Monitoring

Track access patterns:

```bash
grep "source_ip" audit/audit-2024-01-08.jsonl | \
  jq -r '.source_ip' | sort | uniq -c | sort -rn
```

## Log Rotation

Logs are automatically rotated daily. For additional rotation (size-based, compression), use external tools:

```bash
# Example logrotate config
/path/to/audit/*.jsonl {
    daily
    rotate 30
    compress
    delaycompress
    missingok
    notifempty
}
```

## Best Practices

1. **Set appropriate sample rate**: Start with 10%, increase if needed
2. **Monitor disk usage**: Audit logs can grow quickly
3. **Implement log rotation**: Use logrotate or similar
4. **Archive old logs**: Move to cold storage for compliance
5. **Secure log access**: Audit logs may contain sensitive info
6. **Parse with jq**: JSON Lines format works well with jq

## Integration with Log Aggregators

### Fluentd

```yaml
<source>
  @type tail
  path /var/log/ordo/audit/*.jsonl
  pos_file /var/log/fluentd/ordo-audit.pos
  tag ordo.audit
  <parse>
    @type json
  </parse>
</source>
```

### Filebeat

```yaml
filebeat.inputs:
  - type: log
    paths:
      - /var/log/ordo/audit/*.jsonl
    json.keys_under_root: true
    json.add_error_key: true
```
