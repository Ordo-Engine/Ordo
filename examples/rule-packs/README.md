# Rule packs

Ready-to-run Ordo projects for common business decisions. Each one is a
folder you can copy, with its rules, its tests, and a README that shows how a
rule change plays out with `ordo impact`.

| pack | decides | shows |
|------|---------|-------|
| [credit-approval](./credit-approval) | approve, review or decline a personal loan | hard rules, computed DTI, a first-hit decision table, decimal money |
| [promo-stacking](./promo-stacking) | which promotions apply and what the customer pays | a collect table, summing and capping discounts |
| [fraud-scoring](./fraud-scoring) | allow, challenge or block a card payment | a scorecard built from a collect table |

## Use one

```bash
npm i -g @ordo-engine/cli
cp -r examples/rule-packs/credit-approval my-credit-rules
cd my-credit-rules && git init && git add . && git commit -m "start from the credit pack"
ordo test
```

Then change the numbers to your policy. Before you call a change done, run
`ordo impact <ruleset>`: it compares your edit with the last commit over the
tests and probes on both sides of every threshold, and lists each decision that
changed. Coding agents can do the same through `ordo mcp`.

The thresholds here are illustrative, not advice for any real lending, pricing
or fraud policy.
