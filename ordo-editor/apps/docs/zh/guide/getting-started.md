# 安装与运行

Ordo 有三个可以单独使用的部分：命令行 `ordo`、规则服务 `ordo-server`、Rust 库 `ordo-core`。按需要选一个即可。

## 命令行

本地写规则、跑测试、在 CI 里校验，用命令行就够了。npm 包里是预编译的二进制，支持 Linux、macOS 和 Windows。

```bash
npm install -g @ordo-engine/cli
ordo --help
```

## 规则服务（Docker）

```bash
docker run -p 8080:8080 ghcr.io/ordo-engine/ordo:latest
```

默认规则只存在内存里。要从目录加载并持久化规则，把目录挂进容器：

```bash
docker run -p 8080:8080 -v "$PWD/rules:/data/rules" \
  ghcr.io/ordo-engine/ordo:latest --rules-dir /data/rules
```

检查服务是否正常：

```bash
curl http://localhost:8080/health
```

HTTP 默认端口 8080，gRPC 默认端口 50051。全部启动参数见[服务器选项](/zh/reference/server-options)。

## 从源码构建

需要 Rust 1.83 或更高版本。

```bash
git clone https://github.com/Ordo-Engine/Ordo.git
cd Ordo
cargo build --release

./target/release/ordo-server --rules-dir ./rules
```

## 嵌入 Rust 程序

```toml
[dependencies]
ordo-core = { git = "https://github.com/Ordo-Engine/Ordo" }
```

加载规则后记得调用 `RuleSet::from_json_compiled()` 或 `from_yaml_compiled()`，这样表达式只解析一次。

## 可视化编辑器

不想装任何东西，可以直接用[在线演练场](https://ordo-engine.github.io/Ordo/)。在本地运行编辑器：

```bash
cd ordo-editor
pnpm install
pnpm dev
```

## 下一步

- [快速上手](./quick-start)：写出并运行第一条规则
- [分布式部署](./distributed-deployment)：多实例部署
- [Kubernetes](./integration/kubernetes)
