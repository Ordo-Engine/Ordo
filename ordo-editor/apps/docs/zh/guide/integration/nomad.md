# HashiCorp Nomad 集成

<img src="/integration/nomad.png" alt="Nomad Logo" width="120" style="margin-bottom: 20px;" />

Ordo 提供了完整的 [HashiCorp Nomad](https://www.nomadproject.io/) 集成支持，包含用于生产环境和开发环境的 Job 配置文件。

## 概述

Nomad 集成提供了以下特性：

- **服务发现**：自动注册 HTTP 和 gRPC 服务
- **健康检查**：配置了 HTTP (`/health`) 和 TCP 健康检查
- **滚动更新**：支持零停机部署和自动回滚
- **资源隔离**：配置了 CPU 和内存限制

## 快速开始

在项目根目录下，使用提供的部署脚本：

```bash
# 部署到开发环境（动态端口）
./deploy/nomad/deploy.sh deploy dev

# 部署到生产环境（静态端口 8080/50051）
./deploy/nomad/deploy.sh deploy prod
```

## 配置文件 (Job Spec)

以下是标准的生产环境配置示例 (`ordo-server.nomad`)：

```hcl
job "ordo-server" {
  datacenters = ["dc1"]
  type = "service"

  group "ordo" {
    count = 1

    network {
      port "http" {
        static = 8080
        to     = 8080
      }
      port "grpc" {
        static = 50051
        to     = 50051
      }
    }

    service {
      name     = "ordo-http"
      port     = "http"
      provider = "nomad"
      tags     = ["ordo", "http", "api"]

      check {
        name     = "http-health"
        type     = "http"
        path     = "/health"
        interval = "10s"
        timeout  = "3s"
      }
    }

    task "ordo-server" {
      driver = "docker"

      config {
        image = "ghcr.io/pama-lee/ordo-server:latest"
        ports = ["http", "grpc"]
      }

      resources {
        cpu    = 500
        memory = 256
      }
    }
  }
}
```

## 监控

部署后，Prometheus 指标可以通过 Nomad 服务发现自动抓取，或者直接访问 `/metrics` 端点。
