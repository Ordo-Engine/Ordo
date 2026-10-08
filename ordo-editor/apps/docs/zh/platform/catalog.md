# 事实目录与概念

**事实目录**（Fact Catalog）描述项目中规则可以读取的字段。Studio 编辑器、契约校验和测试用例的输入提示都依赖它。

## 为什么需要目录

业务规则常见的问题：

- 不同规则集对同一字段命名不一致（`user.id` / `customer_id` / `uid`）
- 类型漂移（原本是数字的字段在某次发布后变成字符串）
- 字段含义只有写规则的工程师知道

事实目录把这些约束写下来：每个字段都有名称、类型、描述和示例值，作为项目级的唯一来源。

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

概念是复合结构。多个规则集需要引用同一个对象（比如「用户」「订单」）时，用概念定义一次，在事实目录里通过 `concept:User` 引用。

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

## 与契约、Studio 的配合

- **契约**（[决策契约](./contracts)）通过事实和概念约束 RuleSet 的输入与输出。
- **Studio** 编写表达式时的下拉提示和类型校验来自目录。
- **测试用例**的输入字段提示也来自目录。项目的类型信息都集中在目录里。
