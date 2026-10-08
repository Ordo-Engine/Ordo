# 测试管理

平台为每个规则集提供独立的测试套件，格式与 ordo-cli 使用的 YAML 相同，可以在 Studio、CLI 和 CI 中运行。

## 用例结构

```yaml
# project-level testset
ruleset: discount-check
cases:
  - name: vip 用户走 20% 折扣
    input:
      user: { id: u1, vip: true, age: 28 }
      order: { amount: 200 }
    expect:
      code: VIP
      output: { discount: 0.2 }

  - name: 未成年拒单
    input:
      user: { id: u2, vip: false, age: 16 }
      order: { amount: 50 }
    expect:
      code: DENY
      output: { reason: 'underage' }
```

## API

| 操作       | 端点                                                         |
| ---------- | ------------------------------------------------------------ |
| 列出测试   | `GET  /api/v1/projects/:pid/rulesets/:name/tests`            |
| 创建/更新  | `POST/PUT /api/v1/projects/:pid/rulesets/:name/tests[/:tid]` |
| 单个运行   | `POST /api/v1/projects/:pid/rulesets/:name/tests/:tid/run`   |
| 全部运行   | `POST /api/v1/projects/:pid/rulesets/:name/tests/run`        |
| 项目级运行 | `POST /api/v1/projects/:pid/tests/run`                       |
| 导出 YAML  | `GET  /api/v1/projects/:pid/rulesets/:name/tests/export`     |

## 与发布的联动

创建发布请求时（见[发布流程](./releases)），平台会自动运行相关规则集的全部测试用例。任何用例失败，发布请求都无法创建。

可以在发布策略里关闭 `auto_run_tests` 跳过这一步，但生产环境不建议这样做。

## CI 集成

- 从平台导出 YAML 文件，提交到代码仓库。
- 在 PR 阶段用 ordo-cli 运行：

```bash
ordo test --rules ./rulesets --tests ./tests --reporter junit > junit.xml
```

支持的输出格式有 JUnit XML、JSON 和 TAP，可以直接用于 GitHub Actions / GitLab CI。

## Trace 与失败诊断

测试用例失败时，平台返回完整的执行 trace。在 Studio 中打开测试结果可以看到：

- 期望的 output code 与实际命中的 code
- 失败前命中的最后一条分支
- 每个 action 节点的赋值过程

详见 [Studio 编辑器：执行追踪](./studio#执行追踪面板)。
