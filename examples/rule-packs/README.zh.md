# 规则包

几个常见业务决策的 Ordo 项目，拿来就能跑。每个规则包是一个文件夹，里面有规则、测试，以及一份 README，演示改一个阈值之后用 `ordo impact` 看清楚哪些决策变了。

| 规则包 | 做什么决策 | 用到的能力 |
|--------|-----------|-----------|
| [credit-approval](./credit-approval) | 个人贷款：通过、人工审核或拒绝 | 硬性规则、计算负债收入比、首条命中决策表、decimal 金额 |
| [promo-stacking](./promo-stacking) | 结账时哪些优惠生效、最后付多少 | collect 决策表、优惠叠加与封顶 |
| [fraud-scoring](./fraud-scoring) | 一笔卡支付：放行、加验证或拦截 | 用 collect 决策表做评分卡 |

## 怎么用

```bash
npm i -g @ordo-engine/cli
cp -r examples/rule-packs/credit-approval my-credit-rules
cd my-credit-rules && git init && git add . && git commit -m "start from the credit pack"
ordo test
```

然后把数字改成你们自己的政策。每次改完，先跑 `ordo impact <规则集>`：它拿你的改动和上一次提交对比，跑一遍测试用例和每个阈值两侧的探测输入，把变了的决策逐条列出来。编码 Agent 通过 `ordo mcp` 也能做同样的事。

这里的阈值只是示例，不构成任何真实信贷、定价或风控政策的建议。
