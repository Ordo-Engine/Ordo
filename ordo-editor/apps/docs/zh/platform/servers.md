# 引擎

平台不执行规则。`ordo-server` 实例（引擎）向平台注册，项目绑定到引擎，发布的规则送到这些引擎上。

## 接入引擎

在 Studio 里打开「服务器」，为你的组织创建一个连接令牌。对话框会给出这样一条命令：

```bash
docker run -d --name ordo-engine -p 8080:8080 \
  -e ORDO_PLATFORM_URL=https://studio.example.com \
  -e ORDO_CONNECT_TOKEN=ordo_connect_xxxx \
  -e ORDO_SERVER_TOKEN=<每个引擎唯一> \
  -e ORDO_SERVER_NAME=engine-1 \
  -e ORDO_SERVER_URL=http://ENGINE_HOST:8080 \
  -e ORDO_NATS_URL=nats://NATS_HOST:4222 \
  ghcr.io/ordo-engine/ordo:latest
```

| 变量 | 为什么需要 |
| --- | --- |
| `ORDO_PLATFORM_URL` 和 `ORDO_SERVER_TOKEN` | 两个都设置了引擎才会注册。注册后每 30 秒发一次心跳 |
| `ORDO_CONNECT_TOKEN` | 把引擎放进你的组织，只有组织成员能看到它 |
| `ORDO_SERVER_URL` | 平台访问这个引擎用的地址 |
| `ORDO_NATS_URL` | 和平台用同一个 NATS，发布的规则通过它送达 |

吊销连接令牌后，不能再用它注册新引擎；已经注册的引擎会保留，直到你删除它。连接令牌出现之前注册的引擎不属于任何组织：设置 `ORDO_CONNECT_TOKEN` 后重启即可。

Studio 的服务器列表会显示每个引擎的健康状态和指标。

## 绑定项目

项目只能绑定本组织的引擎，也可以按环境分别绑定。某个环境的发布会送到它绑定的引擎。

## 调用规则集

业务系统直接调用引擎，项目 ID 就是租户：

```bash
curl -X POST https://<engine>/api/v1/execute/loan-approval \
  -H 'x-tenant-id: <project-id>' -H 'Content-Type: application/json' \
  -d '{"input":{"amount":5000,"is_vip":true}}'
```

```json
{ "code": "APPROVED", "message": "Within limit", "output": { "approved": true }, "duration_us": 6 }
```

按 `code` 分支，从 `output` 读计算出的字段。Studio 的「接入」页会按你选的规则集填好这个调用，有 curl、Node、Python 和 Go 几种写法。

如果业务系统只能访问平台，可以带着平台登录令牌走平台的代理：`POST /api/v1/engine/<project-id>/execute/loan-approval` 会被转发到绑定引擎的 `/api/v1/execute/loan-approval`。直接调用引擎更快。

引擎面向内网使用。API token、TLS 和限流见[运行服务](/zh/guide/server#访问控制)。
