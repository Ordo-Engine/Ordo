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
  --log-level info
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

目前 Ordo 仅使用命令行参数。计划未来支持环境变量。

## 帮助

```bash
ordo-server --help
ordo-server --version
```
