# Benchmark Runner

Reproducible HTTP throughput benchmarks comparing Ordo against OPA, json-rules-engine, and Grule.

## Prerequisites

- Docker (≥ 10 GB RAM allocated in Docker Desktop → Settings → Resources)
- [`hey`](https://github.com/rakyll/hey) — `brew install hey` or `go install github.com/rakyll/hey@latest`
- `jq` — `brew install jq`

## Quick start — Layer A (cross-engine comparison)

```bash
cd scripts/bench
./benchmark-runner.sh a
```

Results land in `scripts/bench/results/<timestamp>/`:
- `aggregated/layer-a-summary.csv` — QPS, avg/p50/p95/p99 per engine × level × concurrency
- `raw/layer-a/` — full `hey` output for each run
- `metadata.json` — machine info and commit hash

Expected runtime: ~45 minutes for Layer A (4 engines × 4 levels × 3 concurrencies × 5 rounds).

## Layers

| Layer | What it measures | Runtime |
|-------|-----------------|---------|
| `a`   | Cross-engine QPS at c=1/50/200, 4-core Docker | ~45 min |
| `b`   | Ordo + OPA core scaling (1/2/4/all cores) | ~60 min |
| `c`   | Ordo deep profile across concurrency levels | ~30 min |
| `all` | All three layers | ~2.5 h |

## Running a single engine

```bash
./benchmark-runner.sh a --engine ordo
./benchmark-runner.sh a --engine opa
```

## Appending to an existing results directory

```bash
./benchmark-runner.sh a --results-dir results/20260311_120000 --engine grule
```

## Test rule complexity levels

| Level | Description | Conditions |
|-------|-------------|-----------|
| L1 | Trivial — single numeric compare | 1 |
| L2 | Simple — two conditions + branching | 3 |
| L3 | Medium — nested fields, multi-branch | 6 |
| L4 | Complex — deep nesting, array access | 12 |

## Methodology

- Each test point: 5 independent rounds, each a 30-second `hey` load
- Warmup before every round: 1,000 reqs at c=1, then 5,000 at target concurrency, then 5 s idle
- Cooldown: 15 s between same-engine tests, 30 s between engines
- Docker resource limits: `--cpuset-cpus=0-3 --memory=8g` for Layer A (equal footing)
- All engines serve the same logical rule and input payload; see `rules/` for definitions

## Published results

See [benchmark/BENCHMARK_REPORT_EN.md](../../benchmark/BENCHMARK_REPORT_EN.md) for the full report with raw numbers and graphs.
