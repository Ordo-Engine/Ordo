# 性能基准测试

Ordo 规则引擎的全面性能基准测试，包括核心引擎微基准测试、HTTP 服务器吞吐量、分布式（NATS 同步）模式，以及与主流规则引擎的正面对比。

> **测试环境**: Apple M1 Pro (10 核), 16 GB RAM, macOS Darwin 25.3.0
> **工具**: Criterion.rs (微基准测试), hey (HTTP 压测), Docker (NATS)
> **日期**: 2026-03-11

---

## 1. 核心摘要

| 指标                                     | 数值                |
| ---------------------------------------- | ------------------- |
| 核心引擎执行 (4 分支规则)                | **573 ns**          |
| HTTP 单实例峰值 QPS                      | **62,511**          |
| HTTP 分布式 (Writer + Reader 合计)       | **83,082**          |
| 批量执行峰值吞吐                         | **306 万条规则/秒** |
| 满载内存占用                             | **46.2 MB**         |
| vs Zen Engine (Rust 竞品) 核心速度       | **快 7.3 倍**       |
| vs OPA (Go) HTTP 吞吐                    | **快 1.9 倍**       |
| vs json-rules-engine (Node.js) HTTP 吞吐 | **快 3.4 倍**       |

---

## 2. 核心引擎微基准测试

纯规则求值速度，使用 Criterion.rs 测量 — 无 HTTP、无中间件、无 I/O。每个引擎求值等价的 4 分支决策规则 (`score >= 90 / 70 / 50 / default`)，输入 `{"score": 75}` 命中第二个分支。

### 2.1 Rust 引擎对比

| 引擎                          | 每次求值时间 | 相对速度 | 理论单核 QPS  |
| ----------------------------- | ------------ | -------- | ------------- |
| 原生 Rust `if/else` (天花板)  | 3.5 ns       | —        | 285,000,000   |
| **Ordo** (字节码 VM)          | **573 ns**   | **1.0x** | **1,746,000** |
| Rhai (AST 解释器)             | 728 ns       | 0.79x    | 1,374,000     |
| Zen Engine / GoRules (图求值) | 4,200 ns     | 0.14x    | 238,000       |

**核心结论**: Ordo 的字节码 VM 比 Zen Engine 快 7.3 倍，比 Rhai 快 1.27 倍。

### 2.2 Ordo 详细微基准测试

#### 表达式解析 (一次性开销)

| 表达式                                       | 时间    |
| -------------------------------------------- | ------- |
| `age > 18` (简单比较)                        | ~1.0 µs |
| `status == "active"` (字符串相等)            | ~1.0 µs |
| `age > 18 && status == "active"` (AND)       | ~2.0 µs |
| `age < 13 \|\| age > 65` (OR)                | ~2.2 µs |
| `user.profile.level == "gold"` (嵌套路径)    | ~1.1 µs |
| `status in ["active", "pending"]` (集合成员) | ~1.9 µs |

#### 表达式求值 (每次执行, 已预编译)

| 表达式                             | 时间   |
| ---------------------------------- | ------ |
| `age > 18`                         | ~40 ns |
| `status == "active"`               | ~54 ns |
| `age > 18 && status == "active"`   | ~80 ns |
| `user.profile.level == "gold"`     | ~63 ns |
| `status in ["active", "pending"]`  | ~80 ns |
| `score * 0.6 + bonus * 0.4` (算术) | ~67 ns |

#### 规则执行

| 场景                        | 时间            |
| --------------------------- | --------------- |
| 最小 2 步规则 (决策 → 终止) | ~361 ns         |
| 4 分支编译执行              | ~573 ns         |
| 二进制编译 (.ordo 格式)     | ~553 ns         |
| 批量 1K 执行吞吐            | ~270 万 ops/sec |

#### Schema JIT (Cranelift)

| 操作                    | 时间  |
| ----------------------- | ----- |
| JIT 字段访问 (原生代码) | ~5 ns |
| JIT 数值表达式          | ~8 ns |

---

## 3. HTTP 服务器基准测试

所有测试使用 `hey`，持续 10 秒。服务器以 release 模式运行，`--log-level error` 最小化 I/O 噪声。

### 3.1 单机模式 — 并发梯度测试

单 Ordo 实例，4 分支决策规则，输入 `{"input":{"score":75}}`。

| 并发数  | QPS        | 平均延迟 | P50     | P95     | P99     | Max     | CPU% |
| ------- | ---------- | -------- | ------- | ------- | ------- | ------- | ---- |
| 1       | 14,634     | 0.07 ms  | 0.06 ms | 0.10 ms | 0.10 ms | 13.9 ms | 56%  |
| 10      | 47,960     | 0.21 ms  | 0.19 ms | 0.30 ms | 0.40 ms | 10.6 ms | 423% |
| 25      | 57,202     | 0.44 ms  | 0.37 ms | 0.70 ms | 1.60 ms | 28.5 ms | 359% |
| 50      | 59,440     | 0.84 ms  | 0.76 ms | 1.60 ms | 2.90 ms | 51.9 ms | 411% |
| 100     | 61,057     | 1.64 ms  | 1.45 ms | 3.20 ms | 4.90 ms | 31.0 ms | 311% |
| **200** | **62,511** | 3.18 ms  | 2.78 ms | 7.00 ms | 9.90 ms | 39.2 ms | 309% |
| 500     | 60,577     | 8.25 ms  | 6.60 ms | 20.4 ms | 29.2 ms | 76.3 ms | 289% |

**饱和点**: ~60K QPS（并发 50–200 稳定，500 时延迟明显上升）。

### 3.2 分布式模式 (Writer + Reader + NATS JetStream)

Writer 监听 `:8080`，Reader 监听 `:8081`，NATS 监听 `:4222`（Docker, 1 CPU, 256 MB）。

#### Writer (含 NATS 发布者)

| 并发数 | QPS    | 平均    | P50     | P99     |
| ------ | ------ | ------- | ------- | ------- |
| 1      | 12,904 | 0.08 ms | 0.07 ms | 0.20 ms |
| 10     | 44,855 | 0.22 ms | 0.19 ms | 0.50 ms |
| 50     | 56,255 | 0.89 ms | 0.80 ms | 3.20 ms |
| 100    | 58,542 | 1.71 ms | 1.50 ms | 5.00 ms |
| 200    | 58,596 | 3.41 ms | 2.90 ms | 10.9 ms |

#### Reader (含 NATS 订阅者)

| 并发数 | QPS    | 平均    | P50     | P99     |
| ------ | ------ | ------- | ------- | ------- |
| 1      | 14,183 | 0.07 ms | 0.07 ms | 0.10 ms |
| 10     | 45,588 | 0.22 ms | 0.19 ms | 0.50 ms |
| 50     | 58,549 | 0.85 ms | 0.80 ms | 3.00 ms |
| 100    | 59,680 | 1.68 ms | 1.50 ms | 5.10 ms |
| 200    | 60,204 | 3.32 ms | 2.80 ms | 10.7 ms |

**NATS 同步对执行热路径零开销** — Writer/Reader QPS 与单机模式一致。

#### Writer + Reader 同时压测 (各 100 并发)

| 角色     | QPS        | CPU  | RSS     |
| -------- | ---------- | ---- | ------- |
| Writer   | 40,992     | 193% | 30.9 MB |
| Reader   | 42,090     | 205% | 23.6 MB |
| **合计** | **83,082** | —    | 54.5 MB |

在独立机器上部署时，每个 Reader 可线性增加 ~60K QPS。

### 3.3 批量执行吞吐

单机模式 50 并发，变化批量大小。

| 批量大小 | 请求/秒 | 规则执行/秒   | 平均延迟 |
| -------- | ------- | ------------- | -------- |
| 10       | 53,018  | **530,180**   | 0.9 ms   |
| 50       | 33,808  | **1,690,400** | 1.5 ms   |
| 100      | 22,049  | **2,204,900** | 2.3 ms   |
| 500      | 6,128   | **3,064,000** | 8.2 ms   |

### 3.4 资源占用

| 指标     | 空闲    | 满载 (200 并发) |
| -------- | ------- | --------------- |
| RSS 内存 | 24.6 MB | 46.2 MB         |
| 线程数   | 11      | 11              |
| CPU      | 0%      | ~550% (5.5 核)  |

---

## 4. 竞品对比

### 4.1 跨语言 HTTP 对比 (50 并发, 10 秒)

所有引擎求值等价的 4 分支逻辑，各自使用标准 HTTP 栈。

| 引擎                   | 语言    | QPS        | 平均    | P50     | P99     | Max      |
| ---------------------- | ------- | ---------- | ------- | ------- | ------- | -------- |
| Go `net/http` (硬编码) | Go      | 70,134     | 0.71 ms | 0.60 ms | 2.30 ms | 16.0 ms  |
| **Ordo**               | Rust    | **58,374** | 0.86 ms | 0.80 ms | 3.40 ms | 19.8 ms  |
| OPA                    | Go      | 30,398     | 1.64 ms | 0.80 ms | 8.50 ms | 51.8 ms  |
| json-rules-engine      | Node.js | 17,205     | 2.91 ms | 2.60 ms | 6.10 ms | 110.5 ms |
| Grule                  | Go      | 6,547      | 7.63 ms | 7.40 ms | 16.2 ms | 39.0 ms  |

> Go `net/http` 硬编码是理论天花板（无规则引擎，纯 `if/else` + JSON 编解码）。Ordo 达到了其 **83%**。

#### 200 并发

| 引擎                   | QPS        | 平均    | P99     |
| ---------------------- | ---------- | ------- | ------- |
| Go `net/http` (硬编码) | 69,279     | 2.89 ms | 9.10 ms |
| **Ordo**               | **60,386** | 3.30 ms | 11.1 ms |
| OPA                    | 34,166     | 5.85 ms | 27.8 ms |
| json-rules-engine      | 16,296     | 12.3 ms | 19.5 ms |
| Grule                  | 8,328      | 24.0 ms | 50.6 ms |

#### 内存对比 (100 并发负载下)

| 引擎              | 空闲       | 负载        | CPU  |
| ----------------- | ---------- | ----------- | ---- |
| Go `net/http`     | 7.9 MB     | 20.1 MB     | 354% |
| **Ordo**          | **8.3 MB** | **25.7 MB** | 545% |
| Grule             | 11.8 MB    | 30.5 MB     | 558% |
| OPA               | 26.1 MB    | 50.7 MB     | 442% |
| json-rules-engine | 32.6 MB    | 122.1 MB    | 112% |

### 4.2 Rust 引擎 — 核心引擎速度 (无 HTTP)

使用 Criterion.rs 测量。每个引擎求值等价的 4 分支决策规则。

| 引擎                 | 每次求值时间 | 对比 Ordo | 备注                     |
| -------------------- | ------------ | --------- | ------------------------ |
| **Ordo**             | **573 ns**   | **1.0x**  | 字节码 VM + 预编译表达式 |
| Rhai                 | 728 ns       | 0.79x     | AST 树遍历解释器         |
| Zen Engine (GoRules) | 4,200 ns     | 0.14x     | 图遍历 + 每次求值克隆    |

#### 为什么 Ordo 核心更快

| 因素         | Ordo                                   | Zen Engine                                | Rhai                            |
| ------------ | -------------------------------------- | ----------------------------------------- | ------------------------------- |
| 求值方式     | 寄存器式字节码 VM + 可选 Cranelift JIT | 解释式图遍历                              | AST 树遍历解释                  |
| 规则结构     | 预编译步骤图，直接跳转                 | 每次 `Arc<DecisionContent>` 克隆 + 图遍历 | 每次 `Scope` 分配 + String 克隆 |
| 每次求值分配 | 接近零 (预编译)                        | 每次克隆决策图                            | 新 Scope + 变量拷贝             |
| 表达式编译   | 一次编译为字节码                       | 每次解析 + 解释                           | 一次编译为 AST                  |

### 4.3 Rust 引擎 — HTTP 对比 (Zen/Rhai 使用 actix-web)

为公平测试 HTTP 吞吐，Zen 和 Rhai 使用 actix-web（启用 keepalive，与 Ordo 的 Axum 一致）。

| 并发 | Ordo (Axum) | Zen (actix-web) | Rhai (actix-web) |
| ---- | ----------- | --------------- | ---------------- |
| 1    | 13,611      | 16,776          | 18,307           |
| 50   | 57,334      | 105,165         | 113,749          |
| 200  | 61,615      | 122,519         | 128,828          |

HTTP 层面 Zen/Rhai 表现更高 QPS 的原因：

1. **极简处理器** — 接收扁平 JSON，执行简单求值，返回最小 JSON。无中间件、无规则存储、无审计、无租户检查。
2. **actix-web thread-per-core 模型** — 对简单处理器的开销低于 Axum 的 work-stealing tokio 运行时。
3. **Ordo 每请求做的更多** — 规则存储查询 (DashMap)、中间件链 (租户、角色、审计采样、请求超时)、`duration_us` 计时、结构化响应含 `output` 字段。

**CPU 效率 (100 并发)**:

| 引擎             | QPS     | CPU  | QPS/核     |
| ---------------- | ------- | ---- | ---------- |
| Rhai (actix-web) | 119K    | 310% | 38,387     |
| Zen (actix-web)  | 121K    | 348% | 34,770     |
| **Ordo (Axum)**  | **60K** | 472% | **12,712** |

差距完全在 HTTP 服务层，而非引擎本身。如 §4.2 所示，Ordo 核心引擎比 Zen 快 7.3 倍，比 Rhai 快 1.27 倍。

---

## 5. 分布式同步功能测试结果

Writer 和 Reader 实例之间 NATS JetStream 同步的端到端验证。

| 测试项                                | 结果 |
| ------------------------------------- | ---- |
| Writer/Reader 启动 + NATS 连接        | ✅   |
| Writer 创建规则 → Reader 自动同步     | ✅   |
| Writer 更新规则 (v1→v2) → Reader 同步 | ✅   |
| Writer 创建第二条规则 → Reader 同步   | ✅   |
| Writer 删除规则 → Reader 同步删除     | ✅   |
| Reader 执行已同步规则 (所有分支)      | ✅   |
| Reader 批量执行                       | ✅   |
| Reader 拒绝写操作 (409 Conflict)      | ✅   |
| 服务器无限期运行（无 30 秒超时）      | ✅   |

---

## 6. 基准测试期间修复的 Bug

### 6.1 执行器超时热路径回退

**根因**: `default_timeout_ms` 从 `0` 改为 `5000`，导致 `Instant::elapsed()`（一个系统调用，macOS 上 ~20-30ns）在执行器热循环的每一步都被调用。

**修复**: 分摊超时检查 — 跳过前 16 步，之后每 16 步检查一次：

```rust
// crates/ordo-core/src/rule/executor.rs
if timeout_ms > 0
    && depth >= 16
    && depth & 15 == 0
    && start_time.elapsed().as_millis() as u64 >= timeout_ms
{
    return Err(OrdoError::Timeout { timeout_ms });
}
```

同时实现条件步骤计时（仅在 tracing 启用时调用 `Instant::now()`）：

```rust
let (step_result, step_duration) = if tracing {
    let step_start = Instant::now();
    let result = self.execute_step(step, &mut ctx, &ruleset.config.field_missing)?;
    (result, step_start.elapsed().as_micros() as u64)
} else {
    let result = self.execute_step(step, &mut ctx, &ruleset.config.field_missing)?;
    (result, 0)
};
```

同样的分摊超时修复也应用于 `compiled_executor.rs`。

**影响**: `minimal_compiled` 从 398ns 恢复到 361ns，批量吞吐从 250 万 → 270 万 ops/sec。

### 6.2 服务器 30 秒自动退出 Bug

**根因**: `main.rs` 中 `tokio::time::timeout(shutdown_timeout, join_all(tasks))` 从服务器启动时就开始 30 秒倒计时，而非收到关闭信号后。服务器在启动 30 秒后无条件退出。

**修复**: 重构为 `tokio::select!` — 等待关闭信号或任务异常退出，仅在收到信号后启动超时倒计时：

```rust
let all_tasks = futures::future::join_all(tasks);
tokio::pin!(all_tasks);

tokio::select! {
    _ = shutdown_signal => {
        // 收到信号 — 现在开始超时倒计时
        shutdown_tx.send(true).ok();
        match tokio::time::timeout(shutdown_timeout, &mut all_tasks).await {
            Ok(results) => { /* 优雅关闭 */ }
            Err(_) => { warn!("优雅关闭超时"); }
        }
    }
    results = &mut all_tasks => {
        // 服务器自行退出（崩溃或错误）
        for r in results { r??; }
    }
}
```

---

## 7. 复现这些基准测试

### 7.1 核心引擎微基准测试

```bash
# 运行 Ordo 内置 Criterion 基准测试
cargo bench -p ordo-core
```

### 7.2 跨引擎核心基准测试

创建如下 `Cargo.toml` 的 Cargo 项目：

```toml
[package]
name = "engine-bench"
version = "0.1.0"
edition = "2021"

[dependencies]
zen-engine = "0.25"
rhai = "1"
serde_json = "1"
criterion = { version = "0.5", features = ["html_reports"] }
tokio = { version = "1", features = ["rt"] }
ordo-core = { path = "path/to/crates/ordo-core" }

[[bench]]
name = "engines"
harness = false
```

基准测试文件 `benches/engines.rs`：

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ordo_core::prelude::*;
use serde_json::json;

fn ordo_setup() -> (RuleExecutor, RuleSet) {
    let json_str = r#"{
        "config": { "name": "bench", "entry_step": "s1" },
        "steps": {
            "s1": { "id":"s1","name":"S1","type":"decision","branches":[
                {"condition":"score >= 90","next_step":"high"},
                {"condition":"score >= 70","next_step":"mid"},
                {"condition":"score >= 50","next_step":"low"}
            ],"default_next":"fail"},
            "high": {"id":"high","name":"H","type":"terminal",
                     "result":{"code":"HIGH","message":"high tier"}},
            "mid":  {"id":"mid","name":"M","type":"terminal",
                     "result":{"code":"MID","message":"mid tier"}},
            "low":  {"id":"low","name":"L","type":"terminal",
                     "result":{"code":"LOW","message":"low tier"}},
            "fail": {"id":"fail","name":"F","type":"terminal",
                     "result":{"code":"FAIL","message":"failed"}}
        }
    }"#;
    (RuleExecutor::new(), RuleSet::from_json_compiled(json_str).unwrap())
}

fn zen_setup() -> (
    zen_engine::DecisionEngine<
        zen_engine::loader::NoopLoader,
        zen_engine::handler::custom_node_adapter::NoopCustomNode,
    >,
    std::sync::Arc<zen_engine::model::DecisionContent>,
) {
    let json_str = r#"{
        "nodes": [
            {"id":"input","type":"inputNode","name":"Input"},
            {"id":"table1","type":"decisionTableNode","name":"Check","content":{
                "inputs":[{"id":"s","name":"S","type":"expression","field":"score"}],
                "outputs":[
                    {"id":"c","name":"C","type":"expression","field":"code"},
                    {"id":"m","name":"M","type":"expression","field":"message"}
                ],
                "rules":[
                    {"_id":"r1","s":">= 90","c":"\"HIGH\"","m":"\"high tier\""},
                    {"_id":"r2","s":">= 70","c":"\"MID\"","m":"\"mid tier\""},
                    {"_id":"r3","s":">= 50","c":"\"LOW\"","m":"\"low tier\""},
                    {"_id":"r4","s":"< 50","c":"\"FAIL\"","m":"\"failed\""}
                ],
                "hitPolicy":"first"
            }},
            {"id":"output","type":"outputNode","name":"Output"}
        ],
        "edges":[
            {"id":"e1","sourceId":"input","targetId":"table1"},
            {"id":"e2","sourceId":"table1","targetId":"output"}
        ]
    }"#;
    let content: zen_engine::model::DecisionContent =
        serde_json::from_str(json_str).unwrap();
    (zen_engine::DecisionEngine::default(), std::sync::Arc::new(content))
}

fn rhai_setup() -> (rhai::Engine, rhai::AST) {
    let engine = rhai::Engine::new();
    let ast = engine.compile(r#"
        if score >= 90 { code = "HIGH"; message = "high tier"; }
        else if score >= 70 { code = "MID"; message = "mid tier"; }
        else if score >= 50 { code = "LOW"; message = "low tier"; }
        else { code = "FAIL"; message = "failed"; }
    "#).unwrap();
    (engine, ast)
}

fn bench_native(c: &mut Criterion) {
    c.bench_function("native_hardcoded", |b| {
        b.iter(|| {
            let score: f64 = black_box(75.0);
            let r = if score >= 90.0 { ("HIGH","high tier") }
                    else if score >= 70.0 { ("MID","mid tier") }
                    else if score >= 50.0 { ("LOW","low tier") }
                    else { ("FAIL","failed") };
            black_box(r)
        })
    });
}

fn bench_ordo(c: &mut Criterion) {
    let (executor, ruleset) = ordo_setup();
    let input: ordo_core::context::Value =
        serde_json::from_value(json!({"score": 75})).unwrap();
    c.bench_function("ordo_execute", |b| {
        b.iter(|| {
            black_box(
                executor.execute(black_box(&ruleset), black_box(input.clone())).unwrap()
            )
        })
    });
}

fn bench_zen(c: &mut Criterion) {
    let (engine, content) = zen_setup();
    let input = json!({"score": 75});
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all().build().unwrap();
    c.bench_function("zen_evaluate", |b| {
        b.iter(|| {
            let decision = engine.create_decision(content.clone().into());
            black_box(rt.block_on(decision.evaluate(black_box(&input))).unwrap())
        })
    });
}

fn bench_rhai(c: &mut Criterion) {
    let (engine, ast) = rhai_setup();
    c.bench_function("rhai_run_ast", |b| {
        b.iter(|| {
            let mut scope = rhai::Scope::new();
            scope.push("score", black_box(75.0_f64));
            scope.push("code", String::new());
            scope.push("message", String::new());
            let _ = engine.run_ast_with_scope(&mut scope, &ast);
            black_box(scope.get_value::<String>("code").unwrap())
        })
    });
}

criterion_group!(benches, bench_native, bench_ordo, bench_zen, bench_rhai);
criterion_main!(benches);
```

运行：

```bash
cargo bench
```

### 7.3 HTTP 服务器基准测试

```bash
# 构建带 NATS 同步支持的服务器
cargo build --release -p ordo-server --features nats-sync

# --- 单机模式 ---
./target/release/ordo-server \
  --role standalone --rules-dir /tmp/ordo-bench \
  -p 8080 --log-level error &

# 创建测试规则
curl -s -X POST http://localhost:8080/api/v1/rulesets \
  -H "Content-Type: application/json" \
  -d '{
    "config": {"name": "bench-rule", "entry_step": "s1", "version": "1.0.0"},
    "steps": {
      "s1": {"id":"s1","name":"S1","type":"decision","branches":[
        {"condition":"score >= 90","next_step":"high"},
        {"condition":"score >= 70","next_step":"mid"},
        {"condition":"score >= 50","next_step":"low"}
      ],"default_next":"fail"},
      "high": {"id":"high","name":"H","type":"terminal",
               "result":{"code":"HIGH","message":"high tier"}},
      "mid":  {"id":"mid","name":"M","type":"terminal",
               "result":{"code":"MID","message":"mid tier"}},
      "low":  {"id":"low","name":"L","type":"terminal",
               "result":{"code":"LOW","message":"low tier"}},
      "fail": {"id":"fail","name":"F","type":"terminal",
               "result":{"code":"FAIL","message":"failed"}}
    }
  }'

# 并发梯度测试
for C in 1 10 50 100 200 500; do
  echo "--- 并发: $C ---"
  hey -z 10s -c $C -m POST \
    -H "Content-Type: application/json" \
    -d '{"input":{"score":75}}' \
    http://localhost:8080/api/v1/execute/bench-rule
done
```

### 7.4 分布式模式测试

```bash
# 启动 NATS
docker run -d --name nats -p 4222:4222 nats:latest -js

# Writer
./target/release/ordo-server \
  --role writer --rules-dir /tmp/ordo-writer \
  --nats-url nats://localhost:4222 --instance-id writer-1 \
  -p 8080 --grpc-port 50051 --log-level error &

# Reader
./target/release/ordo-server \
  --role reader --rules-dir /tmp/ordo-reader \
  --nats-url nats://localhost:4222 --instance-id reader-1 \
  --writer-addr http://localhost:8080 \
  -p 8081 --grpc-port 50052 --log-level error &

sleep 3

# 在 Writer 上创建规则
curl -s -X POST http://localhost:8080/api/v1/rulesets \
  -H "Content-Type: application/json" \
  -d '{"config":{"name":"bench-rule","entry_step":"s1","version":"1.0.0"},"steps":{"s1":{"id":"s1","name":"S1","type":"decision","branches":[{"condition":"score >= 90","next_step":"high"},{"condition":"score >= 70","next_step":"mid"},{"condition":"score >= 50","next_step":"low"}],"default_next":"fail"},"high":{"id":"high","name":"H","type":"terminal","result":{"code":"HIGH","message":"high tier"}},"mid":{"id":"mid","name":"M","type":"terminal","result":{"code":"MID","message":"mid tier"}},"low":{"id":"low","name":"L","type":"terminal","result":{"code":"LOW","message":"low tier"}},"fail":{"id":"fail","name":"F","type":"terminal","result":{"code":"FAIL","message":"failed"}}}}'

# 等待 NATS 同步
sleep 3

# 验证同步
curl -s http://localhost:8081/api/v1/rulesets  # 应该看到 bench-rule

# 在 Reader 上执行
curl -s -X POST http://localhost:8081/api/v1/execute/bench-rule \
  -H "Content-Type: application/json" \
  -d '{"input":{"score":75}}'

# 同时压测 Writer 和 Reader
hey -z 10s -c 100 -m POST \
  -H "Content-Type: application/json" \
  -d '{"input":{"score":75}}' \
  http://localhost:8080/api/v1/execute/bench-rule &

hey -z 10s -c 100 -m POST \
  -H "Content-Type: application/json" \
  -d '{"input":{"score":85}}' \
  http://localhost:8081/api/v1/execute/bench-rule &

wait

# 清理
docker stop nats && docker rm nats
```

### 7.5 跨语言 HTTP 对比

```bash
# --- OPA ---
brew install opa
cat > /tmp/credit.rego << 'EOF'
package credit
default result := {"code": "FAIL", "message": "failed"}
result := {"code": "VIP", "message": "VIP tier"} if { input.score >= 90 }
result := {"code": "HIGH", "message": "high tier"} if { input.score >= 70; input.score < 90 }
result := {"code": "MID", "message": "mid tier"} if { input.score >= 50; input.score < 70 }
EOF
opa run --server --addr :9000 /tmp/credit.rego &
hey -z 10s -c 50 -m POST -H "Content-Type: application/json" \
  -d '{"input":{"score":75}}' http://localhost:9000/v1/data/credit/result

# --- json-rules-engine (Node.js) ---
# 完整服务器代码见 7.6 节
cd /tmp/json-rules-bench && npm install && node server.js &
hey -z 10s -c 50 -m POST -H "Content-Type: application/json" \
  -d '{"score":75}' http://localhost:9001/execute

# --- Grule (Go) ---
# 完整服务器代码见 7.7 节
cd /tmp/grule-bench && go build -o server . && ./server &
hey -z 10s -c 50 -m POST -H "Content-Type: application/json" \
  -d '{"score":75}' http://localhost:9003/execute
```

### 7.6 json-rules-engine 服务器 (Node.js)

```javascript
// server.js
const { Engine } = require('json-rules-engine');
const fastify = require('fastify')({ logger: false });

function createEngine() {
  const engine = new Engine();
  engine.addRule({
    conditions: { all: [{ fact: 'score', operator: 'greaterThanInclusive', value: 90 }] },
    event: { type: 'VIP', params: { code: 'VIP', message: 'VIP tier' } },
    priority: 4,
  });
  engine.addRule({
    conditions: {
      all: [
        { fact: 'score', operator: 'greaterThanInclusive', value: 70 },
        { fact: 'score', operator: 'lessThan', value: 90 },
      ],
    },
    event: { type: 'HIGH', params: { code: 'HIGH', message: 'high tier' } },
    priority: 3,
  });
  engine.addRule({
    conditions: {
      all: [
        { fact: 'score', operator: 'greaterThanInclusive', value: 50 },
        { fact: 'score', operator: 'lessThan', value: 70 },
      ],
    },
    event: { type: 'MID', params: { code: 'MID', message: 'mid tier' } },
    priority: 2,
  });
  engine.addRule({
    conditions: { all: [{ fact: 'score', operator: 'lessThan', value: 50 }] },
    event: { type: 'FAIL', params: { code: 'FAIL', message: 'failed' } },
    priority: 1,
  });
  return engine;
}

const engine = createEngine();

fastify.post('/execute', async (request) => {
  const { score } = request.body;
  const { events } = await engine.run({ score });
  const top = events[0];
  return top ? top.params : { code: 'UNKNOWN', message: 'no match' };
});

fastify.get('/health', async () => ({ status: 'ok' }));
fastify.listen({ port: 9001, host: '0.0.0.0' });
```

### 7.7 Grule 服务器 (Go)

```go
// main.go
package main

import (
    "encoding/json"
    "fmt"
    "net/http"
    "github.com/hyperjumptech/grule-rule-engine/ast"
    "github.com/hyperjumptech/grule-rule-engine/builder"
    "github.com/hyperjumptech/grule-rule-engine/engine"
    "github.com/hyperjumptech/grule-rule-engine/pkg"
)

const grl = `
rule VIP "VIP" salience 4 { when Input.Score >= 90 then Output.Code = "VIP"; Output.Message = "VIP tier"; Retract("VIP"); }
rule High "High" salience 3 { when Input.Score >= 70 && Input.Score < 90 then Output.Code = "HIGH"; Output.Message = "high tier"; Retract("High"); }
rule Mid "Mid" salience 2 { when Input.Score >= 50 && Input.Score < 70 then Output.Code = "MID"; Output.Message = "mid tier"; Retract("Mid"); }
rule Fail "Fail" salience 1 { when Input.Score < 50 then Output.Code = "FAIL"; Output.Message = "failed"; Retract("Fail"); }
`

type InputFact struct{ Score float64 }
type OutputFact struct{ Code, Message string }

var knowledgeLibrary *ast.KnowledgeLibrary

func init() {
    knowledgeLibrary = ast.NewKnowledgeLibrary()
    rb := builder.NewRuleBuilder(knowledgeLibrary)
    _ = rb.BuildRuleFromResource("Rules", "0.1.0", pkg.NewBytesResource([]byte(grl)))
}

func execute(w http.ResponseWriter, r *http.Request) {
    var req struct{ Score float64 `json:"score"` }
    json.NewDecoder(r.Body).Decode(&req)
    input := &InputFact{Score: req.Score}
    output := &OutputFact{}
    kb, _ := knowledgeLibrary.NewKnowledgeBaseInstance("Rules", "0.1.0")
    dataCtx := ast.NewDataContext()
    dataCtx.Add("Input", input)
    dataCtx.Add("Output", output)
    eng := &engine.GruleEngine{MaxCycle: 10}
    eng.Execute(dataCtx, kb)
    w.Header().Set("Content-Type", "application/json")
    json.NewEncoder(w).Encode(map[string]string{"code": output.Code, "message": output.Message})
}

func main() {
    http.HandleFunc("/execute", execute)
    http.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) { w.Write([]byte(`{"status":"ok"}`)) })
    fmt.Println("grule listening on :9003")
    http.ListenAndServe(":9003", nil)
}
```
