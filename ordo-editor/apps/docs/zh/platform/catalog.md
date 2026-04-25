# 事实目录与概念

事实目录（Fact Catalog）是项目对「规则可以读到哪些字段」的统一描述。Studio 编辑器、契约校验、测试用例的智能提示都基于它工作。

## 为什么需要目录

业务规则常见痛点：

- 不同规则集对同一字段命名不一致（`user.id` / `customer_id` / `uid`）
- 类型漂移（曾经是数字的字段在某次发布后变成字符串）
- 字段含义只存在于工程师脑里

事实目录把这些约束**显式化**：每个字段都有名称、类型、描述、示例值，作为项目级 single source of truth。

## 事实（Fact）

一个事实是一个原子字段定义。

```jsonc
// POST /api/v1/projects/:pid/facts
{
  "name": "user.age",
  "type": "number",
  "description": "用户年龄（周岁）",
  "example": 28,
  "tags": ["user", "demographic"]
}
```

支持的类型：`string` · `number` · `boolean` · `array<T>` · `object` · `concept:<name>`。

## 概念（Concept）

复合结构。当多个规则集都需要引用同一个对象（比如「用户」「订单」），用概念定义一次，事实目录里通过 `concept:User` 引用。

```jsonc
// POST /api/v1/projects/:pid/concepts
{
  "name": "User",
  "fields": [
    { "name": "id", "type": "string" },
    { "name": "age", "type": "number" },
    { "name": "vip", "type": "boolean" }
  ]
}
```

## API

| 操作      | 端点                                              |
| --------- | ------------------------------------------------- |
| 列出事实  | `GET /api/v1/projects/:pid/facts`                 |
| 创建事实  | `POST /api/v1/projects/:pid/facts`                |
| 更新/删除 | `PUT/DELETE /api/v1/projects/:pid/facts/:name`    |
| 列出概念  | `GET /api/v1/projects/:pid/concepts`              |
| 创建概念  | `POST /api/v1/projects/:pid/concepts`             |
| 更新/删除 | `PUT/DELETE /api/v1/projects/:pid/concepts/:name` |

## 与契约、Studio 的协作

- **契约**（[决策契约](./contracts)）通过事实/概念约束 RuleSet 的输入与输出。
- **Studio** 编写表达式时下拉提示 / 类型校验都来自目录。
- **测试用例** 的输入字段提示也来自目录——Catalog 是项目所有「类型信息」的汇聚点。
