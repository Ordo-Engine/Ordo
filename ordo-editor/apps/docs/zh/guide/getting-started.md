# 在本地运行引擎

本指南带你在本机跑起开源**引擎**(`ordo-server`)——用于嵌入、自建,或改核心代码。

> **要做一个决策服务?** 你不需要自己跑引擎。跟着[平台快速上手](/zh/platform/quickstart)走——五分钟内建项目、写规则、发布,引擎由平台托管。

##先决条件

- **Rust**: 1.83 或更高版本
- **Node.js**: 18 或更高版本（用于可视化编辑器）
- **pnpm**: 8 或更高版本（用于可视化编辑器）

## 安装

### 克隆仓库

```bash
git clone https://github.com/Ordo-Engine/Ordo.git
cd Ordo
```

### 构建服务器

```bash
cargo build --release
```

编译后的二进制文件位于 `./target/release/ordo-server`。

### 运行服务器

```bash
# 以默认设置启动（HTTP 端口 8080，gRPC 端口 50051）
./target/release/ordo-server

# 或者启用持久化
./target/release/ordo-server --rules-dir ./rules
```

## 验证安装

检查健康检查端点：

```bash
curl http://localhost:8080/health
```

预期响应：

```json
{
  "status": "healthy",
  "version": "0.4.2",
  "uptime_seconds": 5,
  "storage": {
    "mode": "memory",
    "rules_count": 0
  }
}
```

## 可视化编辑器

要使用可视化规则编辑器：

```bash
cd ordo-editor
pnpm install
pnpm dev
```

在浏览器中打开 `http://localhost:3001`。

或者尝试 [在线演练场](https://ordo-engine.github.io/Ordo/)。

## Docker

```bash
# 构建镜像
docker build -t ordo-server .

# 运行并启用持久化
docker run -p 8080:8080 -v ./rules:/rules ordo-server --rules-dir /rules
```

## 下一步

- [快速入门](./quick-start) - 创建并执行你的第一条规则
- [规则结构](./rule-structure) - 了解规则是如何定义的
- [表达式语法](./expression-syntax) - 学习表达式语言
