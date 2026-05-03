# 决策契约

决策契约（Decision Contract）是规则集的「类型签名」：声明这个规则期望读哪些字段、必定输出哪些结果代码、每个结果带什么字段。

契约让规则与调用方解耦——业务方按契约调用，规则作者按契约编写，发布前平台校验两端一致。

## 契约结构

```jsonc
// POST /api/v1/projects/:pid/contracts
{
  "name": "discount-check",
  "version": "1.0.0",
  "input": {
    "fields": [
      { "name": "user.age", "type": "number", "required": true },
      { "name": "user.vip", "type": "boolean", "required": false, "default": false },
      { "name": "order.amount", "type": "number", "required": true }
    ]
  },
  "outputs": [
    { "code": "VIP", "fields": [{ "name": "discount", "type": "number" }] },
    { "code": "NORMAL", "fields": [{ "name": "discount", "type": "number" }] },
    { "code": "DENY", "fields": [{ "name": "reason", "type": "string" }] }
  ]
}
```

## 校验时机

| 时机                | 校验内容                                                      |
| ------------------- | ------------------------------------------------------------- |
| Studio 编辑实时反馈 | 表达式引用的字段是否在契约的 `input` 中                       |
| 草稿保存            | RuleSet 的 Terminal 步骤的 `code` 是否在契约 `outputs` 列表中 |
| 测试运行            | 测试用例输入是否符合契约 `input.required`                     |
| 发布前              | 与契约 diff，破坏性变更需高级别审批                           |

## 与事实目录的关系

契约的 `input.fields` 引用 [事实目录](./catalog) 中的字段名与类型。改了目录中字段的类型，依赖该字段的契约会被标记为"待迁移"。

## API

| 操作      | 端点                                               |
| --------- | -------------------------------------------------- |
| 列出契约  | `GET /api/v1/projects/:pid/contracts`              |
| 创建契约  | `POST /api/v1/projects/:pid/contracts`             |
| 更新/删除 | `PUT/DELETE /api/v1/projects/:pid/contracts/:name` |

## 版本与破坏性变更

契约本身有 `version` 字段，遵循 SemVer：

- **patch / minor**：新增可选字段、新增 `output.code`、放宽校验——非破坏。
- **major**：删除字段、改类型、删除 output code——破坏性，发布请求会要求显式 ack。
