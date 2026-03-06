# CLI 选项

`ordo-server` 命令行选项的完整参考。

## 用法

```bash
ordo-server [OPTIONS]
```

## 服务器选项

### --http-addr

HTTP 服务器绑定地址。

```bash
ordo-server --http-addr 0.0.0.0:8080
```

|            |                |
| ---------- | -------------- |
| **默认值** | `0.0.0.0:8080` |
| **格式**   | `host:port`    |

### --grpc-addr

gRPC 服务器绑定地址。

```bash
ordo-server --grpc-addr 0.0.0.0:50051
```

|            |                 |
| ---------- | --------------- |
| **默认值** | `0.0.0.0:50051` |
| **格式**   | `host:port`     |

### --uds-path

Unix 域套接字路径（仅限 Unix）。

```bash
ordo-server --uds-path /tmp/ordo.sock
```

|            |           |
| ---------- | --------- |
| **默认值** | 无 (禁用) |
| **格式**   | 文件路径  |

### --disable-http

禁用 HTTP 服务器。

```bash
ordo-server --disable-http
```

|            |         |
| ---------- | ------- |
| **默认值** | `false` |

### --disable-grpc

禁用 gRPC 服务器。

```bash
ordo-server --disable-grpc
```

|            |         |
| ---------- | ------- |
| **默认值** | `false` |

## 存储选项

### --rules-dir

规则持久化目录。

```bash
ordo-server --rules-dir ./rules
```

|            |             |
| ---------- | ----------- |
| **默认值** | 无 (仅内存) |
| **格式**   | 目录路径    |

指定时：

- 启动时从此目录加载规则
- 通过 API 创建/更新规则时保存到此处
- 通过 API 删除规则时从此移除
- 支持 `.json`, `.yaml`, `.yml` 文件

### --max-versions

每个规则保留的最大历史版本数。

```bash
ordo-server --rules-dir ./rules --max-versions 10
```

|            |               |
| ---------- | ------------- |
| **默认值** | `10`          |
| **范围**   | 1 - 无限制    |
| **要求**   | `--rules-dir` |

## 审计选项

### --audit-dir

审计日志文件目录。

```bash
ordo-server --audit-dir ./audit
```

|            |                |
| ---------- | -------------- |
| **默认值** | 无 (仅 stdout) |
| **格式**   | 目录路径       |

指定时：

- 审计事件写入 JSON Lines 文件
- 文件每日轮换 (`audit-YYYY-MM-DD.jsonl`)
- 事件也会记录到 stdout

### --audit-sample-rate

执行日志采样率（百分比）。

```bash
ordo-server --audit-sample-rate 10
```

|            |         |
| ---------- | ------- |
| **默认值** | `10`    |
| **范围**   | 0 - 100 |

- `0` = 无执行日志
- `100` = 记录所有执行
- 可通过 API 在运行时更改

## 签名选项

### --signature-enabled

启用规则签名验证。

```bash
ordo-server --signature-enabled
```

|            |         |
| ---------- | ------- |
| **默认值** | `false` |

### --signature-require

API 更新时拒绝未签名规则。

```bash
ordo-server --signature-enabled --signature-require
```

|            |         |
| ---------- | ------- |
| **默认值** | `false` |

### --signature-trusted-keys

逗号分隔的 base64 公钥。

```bash
ordo-server --signature-enabled --signature-trusted-keys "BASE64_KEY_1,BASE64_KEY_2"
```

### --signature-trusted-keys-file

公钥文件（每行一个 base64 公钥）。

```bash
ordo-server --signature-enabled --signature-trusted-keys-file /etc/ordo/trusted_keys.txt
```

### --signature-allow-unsigned-local

启动时允许本地规则无签名。

```bash
ordo-server --signature-enabled --signature-allow-unsigned-local false
```

|            |        |
| ---------- | ------ |
| **默认值** | `true` |

## 部署选项

### --role

分布式部署的实例角色。

```bash
ordo-server --role reader --writer-addr http://writer-node:8080
```

|            |                                     |
| ---------- | ----------------------------------- |
| **默认值** | `standalone`                        |
| **值**     | `standalone`, `writer`, `reader`    |
| **环境变量** | `ORDO_ROLE`                       |

- `standalone` — 完全读写访问（默认单节点模式）
- `writer` — 完全读写访问，作为主写入节点
- `reader` — 只读；写请求（对 rulesets、tenants、config 的 `POST`/`PUT`/`DELETE`）返回 `409 Conflict` 并附带 writer 地址

### --writer-addr

Writer 节点地址，Reader 实例在 `409` 响应中返回给客户端。

```bash
ordo-server --role reader --writer-addr http://ordo-writer:8080
```

|            |                     |
| ---------- | ------------------- |
| **默认值** | 无                  |
| **格式**   | URL                 |
| **环境变量** | `ORDO_WRITER_ADDR` |

### --watch-rules

启用文件系统监控，当磁盘上的规则文件变更时自动热重载。

```bash
ordo-server --rules-dir ./rules --watch-rules
```

|            |                    |
| ---------- | ------------------ |
| **默认值** | `false`            |
| **要求**   | `--rules-dir`      |
| **环境变量** | `ORDO_WATCH_RULES` |

启用后：

- 监控 `--rules-dir` 下的 `.json`、`.yaml`、`.yml` 文件变更
- 200ms 防抖处理快速连续变更
- 原生文件事件不可用时回退到 30 秒轮询
- 多租户模式下，同时监控 `<rules-dir>/tenants/` 并在 `tenants.json` 变更时重载租户配置

### --max-request-body-bytes

HTTP 请求体最大字节数。

```bash
ordo-server --max-request-body-bytes 5242880
```

|            |                              |
| ---------- | ---------------------------- |
| **默认值** | `10485760`（10 MB）          |
| **环境变量** | `ORDO_MAX_REQUEST_BODY_BYTES` |

同时应用于 gRPC 最大解码消息大小。

### --request-timeout-secs

HTTP 请求超时时间（秒），超时返回 `408 Request Timeout`。

```bash
ordo-server --request-timeout-secs 60
```

|            |                             |
| ---------- | --------------------------- |
| **默认值** | `30`                        |
| **环境变量** | `ORDO_REQUEST_TIMEOUT_SECS` |

## 日志选项

### --log-level

日志详细级别。

```bash
ordo-server --log-level debug
```

|            |                                           |
| ---------- | ----------------------------------------- |
| **默认值** | `info`                                    |
| **值**     | `trace`, `debug`, `info`, `warn`, `error` |

## 示例

### 开发环境

```bash
# 简单的内存服务器
ordo-server

# 启用调试日志
ordo-server --log-level debug
```

### 生产环境

```bash
# 全功能设置
ordo-server \
  --http-addr 0.0.0.0:8080 \
  --grpc-addr 0.0.0.0:50051 \
  --rules-dir /var/lib/ordo/rules \
  --max-versions 20 \
  --audit-dir /var/log/ordo/audit \
  --audit-sample-rate 10 \
  --watch-rules \
  --max-request-body-bytes 10485760 \
  --request-timeout-secs 30 \
  --log-level info
```

### Writer/Reader 部署

```bash
# Writer 节点
ordo-server --role writer \
  --rules-dir /shared/rules \
  --watch-rules

# Reader 节点
ordo-server --role reader \
  --writer-addr http://ordo-writer:8080 \
  --rules-dir /shared/rules \
  --watch-rules
```

### 仅 HTTP

```bash
ordo-server --disable-grpc --http-addr 0.0.0.0:8080
```

### 仅 gRPC

```bash
ordo-server --disable-http --grpc-addr 0.0.0.0:50051
```

### Unix 域套接字

```bash
ordo-server --uds-path /var/run/ordo.sock --disable-http --disable-grpc
```

## 环境变量

Ordo 支持 `ORDO_*` 前缀的环境变量配置，完整列表见配置文档。

## 签名 CLI 工具

### ordo-keygen

生成 Ed25519 密钥对：

```bash
ordo-keygen --output ./keys
```

### ordo-sign

签名 JSON/YAML/.ordo 文件：

```bash
ordo-sign --key ./keys/private.key --input rule.json
```

### ordo-verify

验证 JSON/YAML/.ordo 文件签名：

```bash
ordo-verify --key ./keys/public.key --input rule.signed.json
```

## 帮助

```bash
ordo-server --help
ordo-server --version
```
