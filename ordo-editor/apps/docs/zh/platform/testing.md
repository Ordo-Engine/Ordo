# 测试管理

平台为每个规则集提供独立的测试套件——和 ordo-cli 用同一份 YAML 格式，覆盖 Studio、CLI、CI 三处入口。

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

发布请求创建时（[发布流程](./releases)），平台会自动跑被涉及规则集的全部测试用例。**任何用例失败都会阻断发布请求的创建**。

可在发布策略里关闭 `auto_run_tests` 跳过校验，但生产环境通常不建议这么做。

## CI 集成

- 平台导出 YAML 文件直接 commit 到代码仓库。
- ordo-cli 在 PR 阶段运行：

```bash
ordo test --rules ./rulesets --tests ./tests --reporter junit > junit.xml
```

输出格式：JUnit XML、JSON、TAP，可直接喂给 GitHub Actions / GitLab CI。

## Trace 与失败诊断

测试用例运行失败时，平台返回完整的执行 trace，Studio 中点开测试结果即可看到：

- 期望的 output code 与实际命中的 code
- 失败前命中的最后一条分支
- 每个 action 节点的赋值过程

详见 [Studio 编辑器 - 执行追踪](./studio#执行追踪面板)。
