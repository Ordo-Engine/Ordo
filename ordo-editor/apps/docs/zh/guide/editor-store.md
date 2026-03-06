# 编辑器状态管理与撤销/重做

编辑器 Store 为可视化规则编辑器提供状态管理，通过命令模式实现完整的撤销/重做支持。

## 概述

每个打开的规则集文件拥有独立的 `EditorStore` 实例，确保撤销/重做历史按文件隔离。

### 核心特性

- **命令模式** — 所有变更通过命令执行，支持撤销/重做
- **按文件隔离历史** — 每个文件维护独立的撤销/重做栈
- **Schema 感知编辑** — 条件构建器和值输入根据规则集 Schema 自适应
- **键盘快捷键** — `Cmd/Ctrl+Z` 撤销，`Cmd/Ctrl+Shift+Z` 重做

## 命令

所有规则变更以命令形式表达：

| 命令                  | 描述                    |
| --------------------- | ----------------------- |
| `AddStep`             | 向规则集添加新步骤      |
| `RemoveStep`          | 删除步骤                |
| `UpdateStep`          | 修改步骤属性            |
| `MoveStep`            | 改变步骤位置            |
| `AddBranch`           | 向决策步骤添加分支      |
| `RemoveBranch`        | 删除分支                |
| `UpdateBranch`        | 修改分支条件或目标      |
| `ReorderBranch`       | 改变分支求值顺序        |
| `ConnectSteps`        | 创建步骤间连接          |
| `DisconnectSteps`     | 删除连接                |
| `SetStartStep`        | 设置入口步骤            |
| `UpdateConfig`        | 更新规则集配置          |
| `SetSchema`           | 设置或更新规则集 Schema |
| `Batch`               | 原子性执行多个命令      |
| `PasteStep`           | 粘贴复制的步骤          |
| `ImportDecisionTable` | 导入决策表为步骤        |

## Schema 感知编辑

当规则集定义了 Schema 时，编辑器提供增强的编辑能力：

### 智能条件构建器

`OrdoSmartConditionBuilder` 组件在 Schema 可用时替代基础条件编辑器：

- **字段选择器** — 按分组浏览和搜索 Schema 字段，附带类型标签
- **运算符选择** — 根据字段类型过滤运算符（如数值字段显示 `>`、`<`、`>=`、`<=`）
- **类型感知值** — 输入组件根据字段类型自适应：
  - 字符串 → 文本输入
  - 数字 → 数字输入
  - 布尔 → 开关
  - 枚举 → 下拉选择器
- **模式切换** — 在简单、分组（AND/OR）和表达式模式间切换

### Schema 字段选择器

`OrdoSchemaFieldPicker` 提供：

- 按顶层对象分组展示
- 跨所有字段的模糊搜索
- 键盘导航（方向键、Enter、Escape）
- 类型标记以快速识别

### 增强建议

动作和终止编辑器自动提供：

- 基于 Schema 字段的变量名自动补全
- 结合 Schema 字段和已有变量的表达式输入建议

## Vue 集成

在 Vue 3 中使用 `useEditorStore` 组合式函数：

```vue
<script setup lang="ts">
import { useEditorStore } from '@ordo-engine/editor-vue';

const store = useEditorStore(fileId, initialRuleset);
const { state, dispatch, undo, redo, canUndo, canRedo, schemaContext } = store;
</script>
```

## 编程 API

```typescript
import { EditorStore } from '@ordo-engine/editor-core';

const store = new EditorStore(initialRuleset, { maxHistory: 80 });

// 派发命令
store.dispatch({ type: 'AddStep', payload: { step: newStep } });

// 撤销/重做
store.undo();
store.redo();

// 访问状态
console.log(store.state.steps);
console.log(store.canUndo);
```
