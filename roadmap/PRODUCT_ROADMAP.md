# Ordo 商业化产品 Roadmap

> 版本: 1.2  
> 更新时间: 2026-04-01  
> 目标客户: 金融/支付、电商/营销、大型互联网企业  
> 部署模式: 私有化部署优先，支持 SaaS

---

## 设计原则

**核心理念**: 规则引擎必须是一个**自治系统**，能够在任何外部依赖失效时独立运行。

```
关键场景保障:
1. 规则管理系统挂了 → 引擎继续用本地规则执行
2. 引擎进程重启/崩溃 → 秒级恢复，规则不丢失  
3. 网络分区 → 引擎独立运行，恢复后自动同步
4. 规则推送失败 → 降级使用旧版本，不影响业务
```

---

## 当前状态

### 已完成

- [x] Core Engine: 1.63µs 执行延迟（解释器），50-80ns（JIT），500K+ QPS
- [x] HTTP REST API（Axum，完整 CRUD）
- [x] gRPC API（Tonic，多租户 metadata，批量执行，最大 1000 条/批）
- [x] Unix Domain Socket（gRPC-over-UDS，仅 Unix）
- [x] 执行追踪（ExecutionTrace / StepTrace，逐步调试）
- [x] 内置函数库（**92 个**，含字符串/数学/日期/集合/类型转换等）
- [x] WASM 浏览器执行（Cranelift JIT 在 wasm32 自动禁用）
- [x] 可视化规则编辑器（Vue 3 + React 组件库）
- [x] Flow 图执行追踪可视化
- [x] 多语言 i18n 支持
- [x] 规则持久化（文件系统，`--rules-dir`，支持 JSON/YAML/.ordo）
- [x] 规则版本管理与回滚（默认保留 10 个版本，可配置）
- [x] 审计日志（JSON Lines，可配置采样率）
- [x] JIT 编译（Cranelift，20-30x 加速，schema-aware 类型推断）
- [x] 编译后规则格式（`.ordo` 二进制，MAGIC header + CRC32 + 可选 ED25519 签名）
- [x] 规则完整性签名（ED25519 sign/verify，签名密钥配置，未签名规则可选拒绝）
- [x] 多租户隔离（X-Tenant-ID，per-tenant QPS/burst/timeout，数据命名空间隔离）
- [x] 执行超时与递归深度限制（`timeout_ms`，`max_depth`，逐步检查）
- [x] npm 包发布（`@ordo-engine/editor-core`，`@ordo-engine/editor-vue`，`@ordo-engine/editor-react`）
- [x] Playground 文件导入/导出（.ordo、JSON、YAML）
- [x] Prometheus 指标（`/metrics`）与 OpenTelemetry traces（OTLP 导出）
- [x] NATS JetStream 分布式同步（writer 广播，reader 订阅，`--nats-url`）
- [x] 文件热更新（`--watch-rules`，inotify，自写抑制）
- [x] Webhook 通知系统（异步投递，HMAC-SHA256 签名，指数退避重试）
- [x] 数据过滤 API（SQL/MongoDB/JSON 谓词下推生成，`filter/` 模块）
- [x] 规则测试框架（`testing.rs`，YAML 测试套件，pass/fail 断言）
- [x] 分布式部署模式（standalone / writer / reader 三种角色）
- [x] 运行时配置热更新（`/api/v1/admin/config`，audit_sample_rate / QPS 等无重启生效，PR #61）
- [x] CLI 工具（`ordo eval` / `ordo exec` / `ordo test`，仅依赖 ordo-core，PR #49）
- [x] Go SDK（连接池、指数退避重试、HTTP+gRPC 双协议，`sdk/go/`）
- [x] Python SDK（HTTP+gRPC，retry，batch，`sdk/python/`）
- [x] Java SDK（HTTP，retry，batch，`sdk/java/`）

### 待完善

- [ ] WAL 日志与崩溃恢复（进程重启后规则状态一致性）
- [ ] 离线运行模式（上游断开时自治执行，恢复后自动对账）
- [ ] 增强健康检查（暴露 online/offline 模式、同步延迟、pending 变更数）

---

## Phase 1: 自治与容灾 (Autonomy) - 4-6 周

**目标**: 引擎具备完全自治能力，进程重启无感知，上游断开不影响业务

### 1.1 WAL 日志与崩溃恢复

**现状**: 规则持久化基于文件系统（JSON/YAML/`.ordo`），进程崩溃若写到一半可能造成文件损坏。

**方案**: 在现有文件存储之上叠加追加写 WAL，不引入外部数据库依赖。

**架构设计**:

```
┌─────────────────────────────────────────────────┐
│           Rule Management System                 │
│          (外部运营系统，可断开)                   │
└─────────────────┬───────────────────────────────┘
                  │ Push (可失败)
                  ▼
┌─────────────────────────────────────────────────┐
│              Ordo Rule Engine                    │
│  ┌───────────┐  ┌───────────┐  ┌─────────────┐ │
│  │  Memory   │◄─│  WAL Log  │◄─│  File Store │ │
│  │  Cache    │  │ (append)  │  │ (JSON/YAML) │ │
│  └─────┬─────┘  └───────────┘  └─────────────┘ │
│        │                                        │
│   [Execute]  ← 执行请求永远走内存缓存            │
└────────┼────────────────────────────────────────┘
```

**实现要点**:

| 组件 | 说明 |
|------|------|
| 追加写 WAL | 规则变更先写 `wal/` 目录下的 `.log` 文件，成功后再更新主文件 |
| 内存缓存 | 执行时零 I/O，纯内存操作（当前已是此模型） |
| 启动恢复 | 扫描未完成的 WAL 条目，重放或回滚 |
| 定期快照 | 压缩 WAL，加速启动恢复 |
| 原子写 | 先写临时文件，rename 保证原子性（已部分实现） |

**文件结构**:

```
/var/lib/ordo/
├── rules/                # 主规则文件存储（JSON/YAML/.ordo）
│   └── <name>.json
├── wal/                  # Write-Ahead Log
│   ├── 000001.log
│   └── 000002.log
├── snapshots/            # 定期快照
│   └── snap_20260101_001.bin
└── config.yaml           # 引擎配置
```

### 1.2 崩溃恢复机制

**恢复流程**:

```
启动 → 扫描 wal/ 目录 → 重放未提交条目 → 校验规则完整性 → 服务就绪
                                                    ↓
                                          后台: 与上游对账同步
```

**恢复时间目标**:

| 场景 | 目标时间 |
|------|----------|
| 冷启动（10K 规则） | < 3s |
| 热恢复（有快照） | < 500ms |
| 增量恢复（WAL） | < 100ms |

### 1.3 离线运行模式

**状态机**:

```
        ┌──────────────────────────────────────┐
        │                                      │
        ▼                                      │
   ┌─────────┐  上游断开   ┌──────────┐        │
   │ ONLINE  │───────────►│ OFFLINE  │        │
   │ (同步)  │◄───────────│ (自治)   │        │
   └────┬────┘  上游恢复   └────┬─────┘        │
        │                       │              │
        │ 正常推送规则          │ 使用本地缓存  │
        │                       │              │
        │       ┌───────────────┘              │
        │       │ 离线期间变更入队             │
        │       ▼                              │
        │  ┌──────────┐                        │
        │  │ 同步队列 │─── 恢复后重放 ─────────┘
        │  └──────────┘
        ▼
   [业务执行不中断]
```

**降级策略**:

| 场景 | 处理方式 |
|------|----------|
| 新规则推送失败 | 继续使用旧版本 |
| 规则删除失败 | 标记待删除，下次同步 |
| 配置更新失败 | 使用本地配置 |

### 1.4 增强健康检查

**增强后的健康端点**:

```json
GET /health
{
  "status": "healthy|degraded|unhealthy",
  "mode": "online|offline",
  "storage": {
    "rules_count": 1234,
    "wal_pending_entries": 0,
    "last_snapshot": "2026-01-08T10:00:00Z"
  },
  "upstream": {
    "connected": false,
    "last_sync": "2026-01-08T09:55:00Z",
    "pending_changes": 3
  }
}
```

**新增 Prometheus Metrics**:

```prometheus
# 引擎模式
ordo_engine_mode{mode="online|offline"} 1

# 存储状态
ordo_storage_wal_pending_entries
ordo_storage_wal_size_bytes
ordo_storage_snapshot_age_seconds
ordo_storage_recovery_duration_seconds

# 同步状态
ordo_sync_lag_seconds
ordo_sync_queue_size
ordo_sync_failures_total
```

---

## Phase 2: 执行安全加固 (Security) - 3-4 周

**目标**: 满足金融级安全与合规要求

### 2.1 执行沙箱完善

**现状**: 已有 `timeout_ms` 和 `max_depth` 限制，但缺少循环次数限制和内存上限。

| 限制项 | 现状 | 目标 |
|--------|------|------|
| 执行超时 | ✅ 已实现（默认 100ms） | 保持 |
| 递归深度 | ✅ 已实现（`max_depth`） | 保持 |
| 循环次数 | ❌ 未实现 | 默认 10000 次 |
| 内存限制 | ❌ 未实现 | 单次执行 10MB |

- 在 `BytecodeVM` 执行循环中加入迭代计数器
- Context 大小软限制（Value 序列化后字节数）
- 禁止危险操作（已满足：无文件/网络访问路径）

### 2.2 审计日志完善

**现状**: 已有 JSON Lines 审计日志，采样率可配置。

**待加强**:

| 类型 | 内容 | 存储 |
|------|------|------|
| 规则变更 | 谁/何时/改了什么 | 本地 + 可上报 |
| 执行日志 | 输入/输出/耗时 | 采样存储（已实现） |
| 系统事件 | 启动/恢复/故障 | 本地 + 可上报 |

---

## Phase 3: 高性能优化 (Performance) - 4-6 周

**目标**: 进一步提升性能，支撑更大规模

### 3.1 表达式预编译缓存

**现状**: `RuleSet::compile()` 已将表达式预编译为 AST，但 JIT 编译结果未跨请求复用。

```
规则加载: JSON/YAML → Parse → AST → Compile → ByteCode (已实现)
                                                    ↓ JIT (Cranelift，热点触发)
执行:                                           [直接执行字节码 / 原生代码]
```

**待实现**: JIT 编译结果缓存，避免热点规则重复编译。

### 3.2 批量执行优化

- 批量请求共享上下文对象池（减少 GC 压力）
- 向量化计算（SIMD，针对数值型批量条件）

### 3.3 内存优化

- Context / Value 对象池（`object_pool` crate 或手写 Arena）
- 零拷贝字符串（`Cow<str>`）
- 内存预分配（批量执行时预估容量）

### 3.4 并发模型优化

```
┌─────────────────────────────────────┐
│           Request Queue             │
└─────────────┬───────────────────────┘
              │
    ┌─────────┼─────────┐
    ▼         ▼         ▼
┌───────┐ ┌───────┐ ┌───────┐
│Worker1│ │Worker2│ │Worker3│  (Work-stealing, tokio 已提供)
└───┬───┘ └───┬───┘ └───┬───┘
    │         │         │
    ▼         ▼         ▼
┌─────────────────────────────────────┐
│      Rule Cache (DashMap，无锁读)   │
└─────────────────────────────────────┘
```

---

## Phase 4: 生态扩展 (Ecosystem) - 持续迭代

### 4.1 SDK 完善

**现状**:

| SDK | 实现状态 | 待完善 |
|-----|----------|--------|
| Go | ✅ 基本完整（HTTP+gRPC，retry，连接池） | 集成测试、熔断降级 |
| Python | ✅ 基本完整（HTTP+gRPC，retry，batch） | gRPC 测试覆盖 |
| Java | ✅ 基本完整（HTTP，retry，batch） | gRPC 支持、集成测试 |
| Node.js | ❌ 未实现 | - |

**各 SDK 待实现功能**:
- 熔断降级（circuit breaker）
- 本地规则缓存（降低网络依赖）
- 集成测试（需要运行中的 server）

### 4.2 规则编排

```
规则链 (Chain):
Rule A → Rule B → Rule C → Result

规则组 (Group):
     ┌→ Rule A ─┐
Input┼→ Rule B ─┼→ Merge → Result
     └→ Rule C ─┘

条件路由:
Input → Router → Rule A (if condition)
              → Rule B (else)
```

**注**: `CallRuleSet` Action 类型已在 ordo-core 中实现基础框架。

### 4.3 动态数据源

| 数据源 | 用途 | 超时 |
|--------|------|------|
| HTTP | 外部服务调用 | 可配置 |
| Redis | 缓存查询 | 10ms |
| 数据库 | 只读查询 | 可配置 |

- 连接池管理
- 超时与熔断保护
- 结果缓存

---

## 架构总览

```
┌─────────────────────────────────────────────────────────────┐
│                    External Systems                          │
│  ┌─────────────────┐              ┌──────────────────────┐  │
│  │ Rule Management │              │  Monitoring System   │  │
│  │     System      │              │ (Prometheus/Grafana) │  │
│  └────────┬────────┘              └──────────▲───────────┘  │
└───────────┼──────────────────────────────────┼──────────────┘
            │ Push Rules (可断开)              │ Metrics
            ▼                                  │
┌─────────────────────────────────────────────────────────────┐
│                 Ordo Engine (Self-Contained)                 │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                    API Layer                          │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────────────┐   │   │
│  │  │   HTTP   │  │   gRPC   │  │  Health Check    │   │   │
│  │  └────┬─────┘  └────┬─────┘  └────────┬─────────┘   │   │
│  └───────┼─────────────┼─────────────────┼─────────────┘   │
│          │             │                 │                  │
│  ┌───────┴─────────────┴─────────────────┴─────────────┐   │
│  │                  Executor Layer                      │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────────────┐   │   │
│  │  │ Executor │  │  Cache   │  │  Rate Limiter    │   │   │
│  │  │  (JIT)   │  │ (Memory) │  │  (per tenant)    │   │   │
│  │  └────┬─────┘  └────┬─────┘  └──────────────────┘   │   │
│  └───────┼─────────────┼───────────────────────────────┘   │
│          │             │                                    │
│  ┌───────┴─────────────┴───────────────────────────────┐   │
│  │                  Storage Layer                       │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────────────┐   │   │
│  │  │   WAL    │  │  File    │  │    Recovery      │   │   │
│  │  │   Log    │  │  Store   │  │    Manager       │   │   │
│  │  └──────────┘  └──────────┘  └──────────────────┘   │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
            ▲                 ▲                 ▲
            │                 │                 │
┌───────────┴─────┐ ┌────────┴────────┐ ┌─────┴───────┐
│   Service A     │ │   Service B     │ │  Service C  │
│   (Go SDK)      │ │   (Java SDK)    │ │  (Direct)   │
└─────────────────┘ └─────────────────┘ └─────────────┘
```

---

## 优先级矩阵

| 优先级 | 特性 | 原因 | 阶段 | 预估工时 |
|--------|------|------|------|----------|
| **P0** | WAL + 崩溃恢复 | 进程重启规则一致性 | Phase 1 | 2周 |
| **P0** | 离线运行模式 | 上游断开不影响业务 | Phase 1 | 1周 |
| **P0** | 增强健康检查 | 运维可观测性 | Phase 1 | 0.5周 |
| **P1** | 循环次数限制 | 防止死循环 DoS | Phase 2 | 0.5周 |
| **P1** | 审计日志完善 | 合规要求 | Phase 2 | 1周 |
| **P2** | JIT 缓存 | 热点规则性能 | Phase 3 | 2周 |
| **P2** | 批量执行优化 | 吞吐量提升 | Phase 3 | 1周 |
| **P2** | SDK 熔断降级 | 生产可靠性 | Phase 4 | 1周/SDK |
| **P3** | 规则编排 | 高级业务场景 | Phase 4 | 3周 |
| **P3** | 动态数据源 | 数据增强 | Phase 4 | 2周 |
| **P3** | Node.js SDK | 前端/BFF 接入 | Phase 4 | 2周 |

---

## 技术选型

| 组件 | 选择 | 原因 |
|------|------|------|
| 本地存储 | 文件系统 + WAL | 零外部依赖，与现有存储模型兼容 |
| WAL 格式 | 自研（bincode 序列化） | 简单场景，避免复杂依赖 |
| 序列化 | bincode / serde_json | bincode 用于 WAL/快照，JSON 用于规则文件 |
| 监控 | Prometheus + OpenTelemetry | 行业标准，已实现 |
| 日志 | tracing | Rust 生态标准，已实现 |
| HTTP | axum | 高性能，已实现 |
| gRPC | tonic | Rust 原生，已实现 |
| 分布式同步 | NATS JetStream | 已实现，可选启用 |

---

## 里程碑

| 里程碑 | 目标 | 交付物 |
|--------|------|--------|
| M1: 自治基础 | +6周 | WAL 崩溃恢复、离线运行、增强健康检查 |
| M2: 安全加固 | +10周 | 循环限制、审计日志完善 |
| M3: 性能优化 | +16周 | JIT 缓存、批量执行优化、内存优化 |
| M4: SDK 完善 | +20周 | 熔断降级、集成测试、Node.js SDK |
| M5: 高级特性 | +26周 | 规则编排、动态数据源 |

---

## 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| WAL 文件过大 | 恢复时间增加 | 定期快照压缩，保留最近 N 个 WAL 段 |
| 内存占用过高 | OOM 风险 | 执行内存限制 + LRU 规则淘汰 |
| 版本兼容性 | 升级困难 | WAL/快照格式版本化，向后兼容读取 |
| NATS 不可用 | 多节点同步失败 | 降级为独立运行，文件热更新兜底 |

---

## 附录

### A. 配置示例

```yaml
# /etc/ordo/config.yaml
server:
  http_addr: "0.0.0.0:8080"
  grpc_addr: "0.0.0.0:50051"

storage:
  rules_dir: "/var/lib/ordo/rules"
  wal_dir: "/var/lib/ordo/wal"
  snapshot_dir: "/var/lib/ordo/snapshots"
  wal_sync_interval: "100ms"
  snapshot_interval: "1h"
  max_versions: 10

execution:
  timeout_ms: 100
  max_depth: 100
  max_loop_iterations: 10000

upstream:
  enabled: true
  endpoint: "http://rule-management:8080"
  sync_interval: "30s"
  retry_interval: "5s"

metrics:
  enabled: true
  endpoint: "/metrics"
```

### B. 现有 API 端点

| 方法 | 端点 | 说明 |
|------|------|------|
| GET | `/health` | 健康检查（K8s liveness/readiness probe） |
| GET | `/healthz/live` | Liveness probe |
| GET | `/healthz/ready` | Readiness probe |
| GET | `/metrics` | Prometheus 指标 |
| GET/POST/PUT/DELETE | `/api/v1/rules/:name` | 规则 CRUD |
| POST | `/api/v1/rules/:name/execute` | 执行规则 |
| POST | `/api/v1/execute/batch` | 批量执行 |
| POST | `/api/v1/rules/:name/rollback` | 规则回滚 |
| GET | `/api/v1/rules/:name/versions` | 版本列表 |
| GET/PUT | `/api/v1/admin/config` | 运行时配置热更新 |
| GET/POST/DELETE | `/api/v1/webhooks` | Webhook 管理 |
| GET/POST | `/api/v1/tenants` | 租户管理 |
| POST | `/api/v1/rules/:name/test` | 服务端规则测试 |

---

*文档维护: Ordo Team*  
*最后更新: 2026-04-01*
