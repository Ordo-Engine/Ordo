# Test Management

The platform gives every ruleset its own test suite — same YAML format as ordo-cli, with a single source of truth across Studio, CLI, and CI.

## Case Structure

```yaml
ruleset: discount-check
cases:
  - name: vip user gets 20% discount
    input:
      user: { id: u1, vip: true, age: 28 }
      order: { amount: 200 }
    expect:
      code: VIP
      output: { discount: 0.2 }

  - name: minors are denied
    input:
      user: { id: u2, vip: false, age: 16 }
      order: { amount: 50 }
    expect:
      code: DENY
      output: { reason: 'underage' }
```

## API

| Operation        | Endpoint                                                     |
| ---------------- | ------------------------------------------------------------ |
| List             | `GET  /api/v1/projects/:pid/rulesets/:name/tests`            |
| Create/update    | `POST/PUT /api/v1/projects/:pid/rulesets/:name/tests[/:tid]` |
| Run one          | `POST /api/v1/projects/:pid/rulesets/:name/tests/:tid/run`   |
| Run all          | `POST /api/v1/projects/:pid/rulesets/:name/tests/run`        |
| Project-wide run | `POST /api/v1/projects/:pid/tests/run`                       |
| Export YAML      | `GET  /api/v1/projects/:pid/rulesets/:name/tests/export`     |

## Coupling with Releases

When a [release request](./releases) is created, the platform automatically runs all test cases for the affected rulesets. **Any failure blocks creation of the release request.**

You can disable `auto_run_tests` in a release policy to skip this gate, but production usually shouldn't.

## CI Integration

- Export YAML from the platform and commit it to your code repository.
- Run via ordo-cli during PR checks:

```bash
ordo test --rules ./rulesets --tests ./tests --reporter junit > junit.xml
```

Output formats: JUnit XML, JSON, TAP — all directly consumable by GitHub Actions / GitLab CI.

## Trace & Failure Diagnosis

When a test fails, the platform returns the full execution trace. Click the result in Studio to see:

- Expected vs actual output code
- The last branch that matched before the divergence
- Each action node's assignment trail

See [Studio Editor — Trace Panel](./studio#trace-panel).
