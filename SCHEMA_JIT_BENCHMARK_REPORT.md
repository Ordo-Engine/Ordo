# Schema-Aware JIT Benchmark Report

## Executive Summary

在极端压力测试下，Schema JIT 展现了惊人的性能优势：

| 测试场景 | Tree-walk | BytecodeVM | **Schema JIT** | **加速比** |
|----------|-----------|------------|----------------|------------|
| 20条件复杂规则 | 1,538ns | 1,217ns | **39ns** | **31x** |
| 处理50万条记录 | 197ms | 201ms | **6.6ms** | **30x** |
| 规则管道(5规则×1万条) | 2.8ms | 2.7ms | **128μs** | **21x** |

**峰值吞吐量**: 76M ops/sec (JIT) vs 2.5M ops/sec (VM) = **30x**

---

## Benchmark Results

### 1. Complex Rules (条件数量扩展)

| 条件数 | Tree-walk | BytecodeVM | Schema JIT | **JIT 加速** |
|--------|-----------|------------|------------|--------------|
| 10 | 405ns | 289ns | 12ns | **24x** |
| 15 | 940ns | 832ns | 26ns | **32x** |
| 20 | 1,538ns | 1,217ns | 39ns | **31x** |

```
Execution Time (ns) - Log Scale

10000 ┤
      │
 1000 ┤              ████████████████████ Tree (1538ns)
      │              ███████████████      VM (1217ns)
      │       █████████████
      │       ██████████
  100 ┤ ████████
      │ █████
   10 ┤ █    ██   ███                     JIT (12-39ns)
      │
    1 ┼───────────────────────────────────────────
          10      15       20        Conditions
```

### 2. High Volume Processing (大规模数据处理)

| 数据量 | Tree-walk | BytecodeVM | Schema JIT | **JIT 加速** |
|--------|-----------|------------|------------|--------------|
| 10,000 | 3.9ms | 4.0ms | 132μs | **30x** |
| 100,000 | 39ms | 40ms | 1.3ms | **30x** |
| 500,000 | 197ms | 201ms | 6.6ms | **30x** |

**吞吐量对比**:
- Tree-walk: 2.5M ops/sec
- BytecodeVM: 2.5M ops/sec  
- **Schema JIT: 76M ops/sec** (30x faster!)

```
Processing 500K Records

Time (ms)
    ^
200 ┤ ██████████████████████████████████████████ Tree (197ms)
    │ ██████████████████████████████████████████ VM (201ms)
150 ┤
    │
100 ┤
    │
 50 ┤
    │
  0 ┤ ██                                          JIT (6.6ms)
    ┼──────────────────────────────────────────────────────
        Tree          VM          JIT
```

### 3. Rule Pipeline (规则管道：5规则 × 10,000条记录)

模拟真实业务场景：每条记录需要经过5个规则的检验。

| 方法 | 总时间 | 吞吐量 | **对比** |
|------|--------|--------|----------|
| Tree-walk | 2.8ms | 17.9M evals/sec | 1x |
| BytecodeVM | 2.7ms | 18.6M evals/sec | 1.04x |
| **Schema JIT** | **128μs** | **389M evals/sec** | **21x** |

### 4. Single Evaluation Latency (单次求值延迟)

15条件规则的单次求值：

| 方法 | 延迟 | 适用场景 |
|------|------|----------|
| Tree-walk | ~950ns | 开发调试 |
| BytecodeVM | ~830ns | 冷规则 |
| **Schema JIT** | **~26ns** | **热点规则** |

---

## Real Business Scenario: 贷款审批系统

### 场景描述
- **规则复杂度**: 15个条件（信用评分、收入比、抵押率等）
- **日均处理量**: 100万笔申请
- **SLA要求**: P99 < 1ms

### 性能预估

| 方法 | 单次延迟 | 100万次/天 | 峰值(10x) |
|------|----------|------------|-----------|
| Tree-walk | 950ns | 950ms | 9.5s |
| BytecodeVM | 830ns | 830ms | 8.3s |
| **Schema JIT** | **26ns** | **26ms** | **260ms** |

**结论**: JIT 让规则引擎可以轻松应对突发流量。

---

## When to Use Each Method

### Schema JIT ✅ 推荐场景
- 热点规则（执行 >500 次）
- 高吞吐量场景（>10K QPS）
- Protobuf/已知结构体类型
- 纯数值/比较规则
- 实时风控、反欺诈

### BytecodeVM ✅ 推荐场景
- 冷规则（执行 <100 次）
- 动态 JSON 上下文
- 需要字符串操作
- 需要数组操作
- 规则变更频繁

### Tree-walk ✅ 推荐场景
- 开发和调试
- 一次性求值
- 需要详细错误信息

---

## Compilation Cost Analysis

| 执行次数 | JIT总时间 | VM总时间 | 赢家 |
|----------|-----------|----------|------|
| 10 | 27μs + 0.3μs = 27μs | 3μs | VM |
| 100 | 27μs + 3μs = 30μs | 30μs | 平手 |
| 1,000 | 27μs + 30μs = 57μs | 300μs | **JIT 5x** |
| 10,000 | 27μs + 300μs = 327μs | 3ms | **JIT 9x** |

**Break-even: ~100次执行**

---

## Running Benchmarks

```bash
# 压力测试
cargo bench --bench jit_stress_bench

# 真实场景测试  
cargo bench --bench jit_realistic_bench

# 查看 HTML 报告（有图表！）
open target/criterion/report/index.html
```

---

## Summary

| 指标 | 数值 |
|------|------|
| **最大加速比** | 32x (15条件规则) |
| **高吞吐加速** | 30x (76M vs 2.5M ops/sec) |
| **管道加速** | 21x (多规则场景) |
| **编译Break-even** | ~100次执行 |
| **适用场景** | 高吞吐、Protobuf、数值规则 |

**结论**: Schema-Aware JIT 在高吞吐量规则引擎场景下提供了 **20-30倍** 的性能提升。
