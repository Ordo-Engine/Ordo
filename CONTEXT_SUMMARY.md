# Ordo 项目上下文总结

## 项目概述

Ordo 是一个企业级高性能规则引擎，使用 Rust 实现核心引擎，并提供 Web UI 编辑器组件。

## 核心架构

### 后端 (Rust)
- **ordo-core**: 核心规则引擎库，使用 Rete 算法
- **ordo-server**: HTTP/gRPC 服务
- **通信协议**: gRPC (主要), HTTP, Unix Domain Socket (UDS)
- **数据格式**: Protobuf (生产), JSON (调试)

### 前端 (TypeScript/Vue)
位于 `/ordo-editor/` 目录：
- **packages/core**: 核心数据模型和工具函数
- **packages/vue**: Vue 组件库
- **apps/playground**: 开发测试用的 Playground 应用

## 当前工作重点

### Flow Editor 布局优化

正在优化基于分组（Group）的流程图布局算法。

#### 关键文件：

1. **`/ordo-editor/packages/vue/src/components/flow/utils/layout.ts`**
   - `applyGroupBasedLayout()`: 新的基于分组的布局算法
   - `applyDagreLayout()`: 传统的 Dagre 布局（无分组时使用）
   - 布局策略：
     - Groups 水平排列（从左到右）
     - 组内节点垂直排列
     - Group 框自动计算大小包裹内部节点

2. **`/ordo-editor/packages/vue/src/components/flow/utils/converter.ts`**
   - `rulesetToFlow()`: 将 RuleSet 转换为 Vue Flow 节点和边
   - `flowToRuleset()`: 将 Vue Flow 数据转换回 RuleSet
   - Group nodes 现在作为独立的背景节点（不使用 Vue Flow 的 parentNode 机制）
   - Group node ID 格式: `group-${group.id}`

3. **`/ordo-editor/packages/vue/src/components/flow/OrdoFlowEditor.vue`**
   - 主要的流程编辑器组件
   - `autoLayout()`: 触发自动布局
   - `initFromRuleset()`: 从 RuleSet 初始化流程图
   - `syncToRuleset()`: 同步更改回 RuleSet
   - 使用 `isInternalUpdate` 标志防止循环更新

4. **`/ordo-editor/packages/vue/src/components/flow/nodes/GroupNode.vue`**
   - Group 节点组件
   - 支持调整大小 (NodeResizer)
   - 支持双击编辑名称

5. **`/ordo-editor/apps/playground/src/App.vue`**
   - Playground 应用主组件
   - 包含多文件管理功能
   - 三个示例 RuleSet: `paymentRuleset`, `riskRuleset`, `discountRuleset`
   - 每个 RuleSet 都包含 `groups` 数组定义业务阶段

### 数据模型

#### StepGroup (业务阶段)
```typescript
interface StepGroup {
  id: string;
  name: string;
  description?: string;
  color?: string;
  position: { x: number; y: number };
  size: { width: number; height: number };
  stepIds: string[];  // 属于该组的 step IDs
}
```

#### Step 类型
- **Decision**: 决策节点，有多个分支
- **Action**: 动作节点，执行赋值操作
- **Terminal**: 终结节点，输出结果

#### Branch 接口
```typescript
interface Branch {
  id: string;
  label: string;  // 注意：是 label 不是 name
  condition?: Condition;
  nextStepId?: string;
}
```

## 已实现的功能

1. ✅ 流程图编辑器基础功能
2. ✅ 右键菜单（节点/边）
3. ✅ 边的重新连接
4. ✅ 多文件管理
5. ✅ 表格视图按 Group 显示
6. ✅ 热重载开发环境
7. ✅ 国际化支持 (中/英)
8. ✅ 主题切换 (light/dark)

## 当前问题

### Flow 布局问题
- Group 节点的位置和大小可能没有正确更新
- 需要验证 `applyGroupBasedLayout` 是否正确计算并应用了 group 的位置和大小

### 调试建议
1. 在浏览器中切换到 Flow 模式
2. 点击 "Auto" 按钮触发自动布局
3. 检查 console 是否有错误
4. 验证 groups 是否正确包裹了对应的 steps

## 开发环境

```bash
# 启动 Playground
cd /Users/pamalee/Project/Ordo/ordo-editor
pnpm dev

# 访问地址
http://localhost:3001
```

## 下一步工作

1. **修复 Flow 布局问题**: 确保 Group 正确包裹对应的步骤节点
2. **优化 Group 视觉效果**: 改进 Group 的视觉呈现
3. **完善数据流连接**: 实现节点之间的数据流端口

## 相关文档

- `/docs/` - 内部文档（已在 .gitignore 中隐藏）
- `/benchmark/` - 性能基准测试报告
- `/README.md` - 项目说明

## Git 分支策略

- `main`: 主分支，稳定版本
- `develop`: 开发分支
- `feature/*`: 功能分支
- `fix/*`: 修复分支
- `hotfix/*`: 紧急修复
- `release/*`: 发布分支

## CI/CD

GitHub Actions 配置在 `.github/workflows/`:
- `check.yml`: 代码检查
- `test.yml`: 测试
- `bench.yml`: 基准测试
- `build.yml`: 构建
- `release.yml`: 发布（Docker 镜像，多平台二进制）

