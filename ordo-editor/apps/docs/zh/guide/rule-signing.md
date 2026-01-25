# 规则签名

Ordo 支持对规则进行数字签名，以确保完整性并防止篡改。此功能使用 Ed25519 非对称加密算法对所有格式（JSON、YAML 和编译后的 `.ordo` 二进制文件）的规则进行签名和验证。

## 概述

规则签名提供：

- **防篡改保护**：检测对规则的任何未授权修改
- **来源验证**：验证规则来自可信来源
- **审计追踪**：追踪谁签署了规则以及签署时间
- **多格式支持**：支持 JSON、YAML 和编译后的二进制格式

## 快速开始

### 1. 生成密钥对

```bash
ordo-keygen --output-dir ./keys
```

这将创建两个文件：

- `keys/public.key` - 与需要验证规则的服务器共享
- `keys/private.key` - 保密，用于签署规则

### 2. 签署规则文件

```bash
ordo-sign --private-key ./keys/private.key --input rules/my-rule.json
```

签名后的规则将添加 `_signature` 字段：

```json
{
  "config": {
    "name": "my-rule",
    "entry_step": "start"
  },
  "steps": { ... },
  "_signature": {
    "algorithm": "ed25519",
    "public_key": "base64编码的公钥",
    "signature": "base64编码的签名",
    "signed_at": "2026-01-25T10:30:00Z"
  }
}
```

### 3. 验证规则文件

```bash
ordo-verify --public-key ./keys/public.key --input rules/my-rule.json
```

### 4. 配置服务器验证

```bash
ordo-server \
  --signature-enabled \
  --signature-require \
  --signature-trusted-keys "base64公钥1,base64公钥2"
```

或使用环境变量：

```bash
export ORDO_SIGNATURE_ENABLED=true
export ORDO_SIGNATURE_REQUIRE=true
export ORDO_SIGNATURE_TRUSTED_KEYS_FILE=/etc/ordo/trusted-keys.txt
```

## CLI 工具

### ordo-keygen

生成用于签署规则的 Ed25519 密钥对。

```bash
ordo-keygen [选项]

选项：
  -o, --output-dir <目录>    密钥文件输出目录 [默认: .]
  -p, --prefix <前缀>        密钥文件名前缀 [默认: ""]
  -h, --help                 显示帮助
```

**示例：**

```bash
# 在当前目录生成密钥
ordo-keygen

# 使用自定义前缀生成密钥
ordo-keygen --output-dir ./keys --prefix production-
# 创建: keys/production-public.key, keys/production-private.key
```

### ordo-sign

使用私钥签署规则文件。

```bash
ordo-sign [选项] --private-key <文件> --input <文件>

选项：
  -k, --private-key <文件>  私钥文件路径
  -i, --input <文件>        输入规则文件 (JSON, YAML, 或 .ordo)
  -o, --output <文件>       输出文件 (默认覆盖输入文件)
  -h, --help                显示帮助
```

**示例：**

```bash
# 签署 JSON 规则文件（原地修改）
ordo-sign -k private.key -i rule.json

# 签署并输出到新文件
ordo-sign -k private.key -i rule.json -o rule-signed.json

# 签署 YAML 文件
ordo-sign -k private.key -i rule.yaml

# 签署编译后的 .ordo 二进制文件
ordo-sign -k private.key -i rule.ordo
```

### ordo-verify

验证规则文件签名。

```bash
ordo-verify [选项] --public-key <文件> --input <文件>

选项：
  -k, --public-key <文件>   公钥文件路径
  -i, --input <文件>        要验证的规则文件
  -h, --help                显示帮助
```

**示例：**

```bash
# 验证已签名的规则
ordo-verify -k public.key -i rule.json

# 成功输出：
# ✓ Signature valid for rule.json

# 失败输出：
# ✗ Signature verification failed: Invalid signature
```

## 签名格式

### JSON/YAML 规则

对于 JSON 和 YAML 文件，签名作为 `_signature` 字段嵌入：

```json
{
  "config": { ... },
  "steps": { ... },
  "_signature": {
    "algorithm": "ed25519",
    "public_key": "MCowBQYDK2VwAyEA...",
    "signature": "MEUCIQD...",
    "signed_at": "2026-01-25T10:30:00Z"
  }
}
```

签名是基于规则的**规范化 JSON** 表示计算的（不包括 `_signature` 字段），键按字母顺序排序。

### 编译后的二进制文件 (.ordo)

对于编译后的 `.ordo` 文件，签名存储在二进制头部：

```
┌─────────────────────────────────────┐
│ 魔数: "ORDO" (4 字节)               │
│ 版本: 1 (1 字节)                    │
│ 标志: HAS_SIGNATURE (1 字节)        │
│ 校验和: CRC32 (4 字节)              │
│ 载荷长度 (4 字节)                   │
├─────────────────────────────────────┤
│ 签名块 (如果设置了标志):            │
│   - 公钥 (32 字节)                  │
│   - 签名 (64 字节)                  │
├─────────────────────────────────────┤
│ 压缩载荷 (zstd)                     │
└─────────────────────────────────────┘
```

## 服务器配置

### 配置选项

| 选项                               | 环境变量                              | 描述                   |
| ---------------------------------- | ------------------------------------- | ---------------------- |
| `--signature-enabled`              | `ORDO_SIGNATURE_ENABLED`              | 启用签名验证           |
| `--signature-require`              | `ORDO_SIGNATURE_REQUIRE`              | 要求所有规则必须签名   |
| `--signature-trusted-keys`         | `ORDO_SIGNATURE_TRUSTED_KEYS`         | 逗号分隔的可信公钥列表 |
| `--signature-trusted-keys-file`    | `ORDO_SIGNATURE_TRUSTED_KEYS_FILE`    | 包含可信密钥的文件路径 |
| `--signature-allow-unsigned-local` | `ORDO_SIGNATURE_ALLOW_UNSIGNED_LOCAL` | 允许本地文件不签名     |

### 验证模式

**1. 禁用（默认）**

```bash
ordo-server
# 不进行签名验证
```

**2. 可选验证**

```bash
ordo-server \
  --signature-enabled \
  --signature-trusted-keys "key1,key2"
# 验证已签名的规则，接受未签名的规则
```

**3. 强制验证**

```bash
ordo-server \
  --signature-enabled \
  --signature-require \
  --signature-trusted-keys "key1,key2"
# 拒绝所有未签名的规则
```

**4. 本地文件豁免**

```bash
ordo-server \
  --signature-enabled \
  --signature-require \
  --signature-allow-unsigned-local \
  --signature-trusted-keys "key1,key2"
# API 上传需要签名，允许本地文件不签名
```

### 可信密钥文件格式

创建一个文件，每行一个 base64 编码的公钥：

```text
# 生产环境签名密钥
MCowBQYDK2VwAyEAabc123...

# CI/CD 签名密钥
MCowBQYDK2VwAyEAxyz789...
```

## API 集成

### HTTP API

通过 HTTP API 推送规则时，可以通过两种方式包含签名：

**1. 嵌入请求体**

```bash
curl -X POST http://localhost:8080/api/v1/rulesets \
  -H "Content-Type: application/json" \
  -d '{
    "config": { "name": "my-rule", "entry_step": "start" },
    "steps": { ... },
    "_signature": {
      "algorithm": "ed25519",
      "public_key": "...",
      "signature": "..."
    }
  }'
```

**2. 通过 HTTP 头**

```bash
curl -X POST http://localhost:8080/api/v1/rulesets \
  -H "Content-Type: application/json" \
  -H "X-Ordo-Signature: base64签名" \
  -H "X-Ordo-Public-Key: base64公钥" \
  -d '{
    "config": { "name": "my-rule", "entry_step": "start" },
    "steps": { ... }
  }'
```

### 错误响应

当签名验证失败时：

```json
{
  "error": "Forbidden",
  "message": "Signature verification failed: Invalid signature"
}
```

## 安全最佳实践

### 密钥管理

1. **保护私钥**：安全存储私钥，永远不要提交到版本控制
2. **使用独立密钥**：为不同环境（开发、预发布、生产）使用不同的密钥
3. **轮换密钥**：定期轮换签名密钥并更新服务器上的可信密钥
4. **审计密钥使用**：记录所有签名操作以供审计

### 部署

1. **生产环境启用**：始终在生产环境启用签名验证
2. **强制签名**：使用 `--signature-require` 拒绝未签名的规则
3. **最小信任**：只将必要的公钥添加到可信列表
4. **安全分发密钥**：使用安全渠道分发公钥

### CI/CD 集成

```yaml
# GitHub Actions 工作流示例
- name: 签署规则
  run: |
    echo "${{ secrets.ORDO_PRIVATE_KEY }}" > /tmp/private.key
    for rule in rules/*.json; do
      ordo-sign -k /tmp/private.key -i "$rule"
    done
    rm /tmp/private.key

- name: 部署规则
  run: |
    for rule in rules/*.json; do
      curl -X POST "$ORDO_SERVER/api/v1/rulesets" \
        -H "Content-Type: application/json" \
        -d @"$rule"
    done
```

## 故障排除

### 常见问题

**"Signature verification failed: No signature found"**

- 规则文件不包含签名
- 如果使用头部，确保同时设置了 `X-Ordo-Signature` 和 `X-Ordo-Public-Key`

**"Signature verification failed: Untrusted public key"**

- 签名中的公钥不在服务器的可信密钥列表中
- 将密钥添加到 `--signature-trusted-keys` 或可信密钥文件

**"Signature verification failed: Invalid signature"**

- 规则内容在签名后被修改
- 使用私钥重新签署规则

**"Invalid public key base64"**

- 公钥未正确进行 base64 编码
- 确保使用的是 `.key` 文件的内容，而不是文件路径
