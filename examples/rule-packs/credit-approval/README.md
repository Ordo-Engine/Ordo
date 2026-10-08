# Credit approval

Pre-approval for a personal loan. Takes an applicant and a loan request and
returns `APPROVED`, `MANUAL_REVIEW` or `REJECTED`, with a risk grade, an APR
and the reason.

## Input

```json
{
  "applicant": { "age": 35, "credit_score": 780, "monthly_income": "12000", "monthly_debt": "1500" },
  "loan": { "amount": "60000", "term_months": 36 }
}
```

Money fields are `decimal`. Send them as strings to keep every cent exact.
`monthly_debt` defaults to 0. A missing required field is rejected before any
rule runs.

## How it decides

1. **Hard rules.** Under 18, credit score below 550, or no income: rejected.
2. **Affordability.** `monthly_payment = amount / term_months` and
   `dti = (monthly_debt + monthly_payment) / monthly_income`.
3. **Risk grade**, a decision table where the first matching row wins:

   | credit score | DTI    | grade | decision      | APR   |
   |--------------|--------|-------|---------------|-------|
   | any          | > 0.50 | E     | REJECTED      |       |
   | >= 750       | <= 0.30| A     | APPROVED      | 4.5%  |
   | >= 700       | <= 0.40| B     | APPROVED      | 6.5%  |
   | >= 650       | any    | C     | MANUAL_REVIEW | 8.9%  |
   | >= 600       | any    | D     | MANUAL_REVIEW | 11.9% |
   | otherwise    |        | E     | REJECTED      |       |

4. **Amount cap.** An approval above 10x monthly income goes to manual review.

## Run it

```bash
cd examples/rule-packs/credit-approval
ordo test
ordo trace credit-approval --input '{"applicant":{"age":35,"credit_score":780,"monthly_income":"12000"},"loan":{"amount":"60000","term_months":36}}'
```

## Change a threshold, see what moves

Say risk wants grade A to accept a DTI up to 35% instead of 30%. Change
`"<= 0.30"` to `"<= 0.35"` in the grade table, then:

```
$ ordo impact credit-approval
impact credit-approval  (HEAD → working tree)
  282 inputs: 13 tests, 0 captured, 269 boundary probes

  APPROVED → APPROVED  4 inputs

CHANGED test:score 750 with DTI 0.31 drops to grade B  {...}
    output.apr: 0.065 → 0.045
    output.grade: "B" → "A"
CHANGED probe:applicant.credit_score=751  {...}
    output.apr: 0.065 → 0.045
    output.grade: "B" → "A"
...
4 changed · 278 unchanged
```

Nobody's decision flips, but four applicants now get a lower rate. That is the
change you meant, so update the test that pinned the old grade. Add
`--inputs last-month.jsonl` to replay real applications too.
