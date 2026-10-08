# Fraud scoring

Risk check for a card payment. Adds up risk signals into a score and returns
`ALLOW`, `CHALLENGE` (ask for 3-D Secure or an OTP) or `BLOCK`, with the score
and the signals that fired.

## Input

```json
{
  "amount": "4999.99", "account_age_days": 3, "txn_count_1h": 1,
  "ip_country": "SG", "card_country": "US", "new_device": true, "on_blocklist": false
}
```

`amount` is `decimal`. `txn_count_1h`, `new_device` and `on_blocklist` are
optional.

## How it decides

1. A blocklisted card or account is blocked straight away.
2. Every matching signal adds points (a `collect` decision table):

   | signal           | when                          | points |
   |------------------|-------------------------------|--------|
   | large_amount     | amount >= 5000                | 30     |
   | medium_amount    | 1000 <= amount < 5000         | 10     |
   | new_account      | account younger than 7 days   | 25     |
   | young_account    | account 7 to 29 days old      | 10     |
   | velocity         | 5 or more payments in an hour | 20     |
   | new_device       | first time on this device     | 15     |
   | country_mismatch | IP country != card country    | 20     |

3. Score 70 or more blocks, 40 or more challenges, anything lower is allowed.

## Run it

```bash
cd examples/rule-packs/fraud-scoring
ordo test
ordo trace fraud-scoring --input '{"amount":"6000","account_age_days":400,"ip_country":"SG","card_country":"US"}'
```

## Change a threshold, see what moves

Chargebacks are up and someone proposes blocking from 65 instead of 70. Change
`$score >= 70` to `$score >= 65`, run `ordo test`: all 9 tests still pass. Then
run `ordo impact fraud-scoring`:

```
  130 inputs: 9 tests, 0 captured, 121 boundary probes

  CHALLENGE → BLOCK  5 inputs

CHANGED probe:amount=5000  {"account_age_days":400,"amount":5000,"card_country":"US","ip_country":"SG","new_device":true}
    code: CHALLENGE → BLOCK
CHANGED probe:txn_count_1h=5  {"account_age_days":400,"amount":"4999.99","card_country":"US","ip_country":"SG","new_device":true,"txn_count_1h":5}
    code: CHALLENGE → BLOCK
...
5 changed · 125 unchanged
```

The tests did not notice, but a long-standing customer paying 5000 from abroad
on a new phone used to get a 3-D Secure prompt and is now declined. If that is
intended, add it as a test. Run with `--inputs` over last week's payments to
see how many real customers the change would block.
