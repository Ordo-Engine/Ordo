# 规则持久化

默认情况下，Ordo 将规则存储在内存中。启用基于文件的持久化，可在重启后保留规则。

## 启用持久化

使用 `--rules-dir` 标志指定规则存储目录：

```bash
ordo-server --rules-dir ./rules
```

## 工作原理

当持久化启用时：

1.  **启动**：从目录中的 `.json`、`.yaml` 和 `.yml` 文件加载规则
2.  **创建/更新**：通过 API 创建或更新时，规则将保存到目录中
3.  **删除**：通过 API 删除时，规则文件将被移除

## 目录结构

```
rules/
├── discount-check.json
├── loan-approval.yaml
├── fraud-detection.json
└── .versions/
    ├── discount-check/
    │   ├── v1_1704700000.json
    │   └── v2_1704800000.json
    └── loan-approval/
        └── v1_1704700000.yaml
```

## 文件格式

### JSON 格式

```json
{
  "config": {
    "name": "discount-check",
    "version": "1.0.0",
    "entry_step": "check_vip"
  },
  "steps": {
    "check_vip": {
      "id": "check_vip",
      "type": "decision",
      "branches": [{ "condition": "user.vip == true", "next_step": "vip_discount" }],
      "default_next": "normal_discount"
    }
  }
}
```

### YAML 格式

```yaml
config:
  name: discount-check
  version: '1.0.0'
  entry_step: check_vip

steps:
  check_vip:
    id: check_vip
    type: decision
    branches:
      - condition: 'user.vip == true'
        next_step: vip_discount
    default_next: normal_discount

  vip_discount:
    id: vip_discount
    type: terminal
    result:
      code: VIP
      message: '20% discount applied'
```

## 文件命名

- 文件名成为规则名称（不带扩展名）
- `discount-check.json` → 规则名称：`discount-check`
- 如果多个文件具有相同的基本名称，JSON 优先于 YAML

## 健康检查

健康检查端点显示存储模式：

```bash
curl http://localhost:8080/health
```

```json
{
  "status": "healthy",
  "version": "0.1.0",
  "storage": {
    "mode": "persistent",
    "rules_dir": "./rules",
    "rules_count": 3
  }
}
```

## 最佳实践

1.  **使用版本控制**：将规则目录保存在 Git 中
2.  **按领域组织**：为不同的规则类别使用子目录
3.  **首选 YAML 以提高可读性**：YAML 更易于手动阅读和编辑
4.  **使用 JSON 进行程序化访问**：JSON 更适合 API 响应
5.  **定期备份**：为生产环境实施备份策略

## 故障排除

### 规则未加载

- 检查文件权限
- 验证 JSON/YAML 语法
- 检查服务器日志中的解析错误

### 更改未持久化

- 验证是否指定了 `--rules-dir`
- 检查目录写入权限
- 确保磁盘空间可用

### 启动错误

```bash
# 检查语法错误
cat rules/my-rule.json | jq .

# 验证 YAML
python -c "import yaml; yaml.safe_load(open('rules/my-rule.yaml'))"
```
