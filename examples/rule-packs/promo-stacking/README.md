# Promotion stacking

Which promotions apply at checkout, how they stack, and how much the customer
pays. Returns `PROMO_APPLIED`, `PROMO_CAPPED` or `NO_PROMO`, with the list of
offers, the discount and the amount to pay.

## Input

```json
{ "cart_total": "320", "member_tier": "gold", "coupon_code": "SAVE50", "is_first_order": true, "has_sale_items": false }
```

`cart_total` is `decimal`. Everything else is optional and has a default.

## How it decides

1. Carts under 50 get nothing.
2. Every matching offer applies (a `collect` decision table):

   | offer               | when                                             | discount   |
   |---------------------|--------------------------------------------------|------------|
   | member_gold_10pct   | gold member, no sale items in the cart           | 10%        |
   | member_silver_5pct  | silver member, no sale items in the cart         | 5%         |
   | first_order_20      | first order                                      | 20         |
   | spend_300_save_30   | cart >= 300                                      | 30         |
   | coupon_save50       | coupon `SAVE50`, cart >= 200, not a first order  | 50         |

3. The discounts add up, capped at 25% of the cart.

Percentages are rounded to cents, half away from zero, on exact decimals.

## Run it

```bash
cd examples/rule-packs/promo-stacking
ordo test
ordo trace promo-stacking --input '{"cart_total":"320","member_tier":"gold","is_first_order":true}'
```

## Change a threshold, see what moves

Finance wants the cap at 20% instead of 25%. Change `cart_total * 0.25` to
`cart_total * 0.20` in the `stack` step, then run `ordo impact promo-stacking`:

```
  114 inputs: 10 tests, 0 captured, 104 boundary probes

  PROMO_APPLIED → PROMO_CAPPED  9 inputs
  PROMO_CAPPED → PROMO_CAPPED  15 inputs

CHANGED test:three offers stack and hit the 25% cap  {...}
    output.discount: 80 → 64
    output.pay: 240 → 256
CHANGED probe:cart_total=200  {"cart_total":200,"coupon_code":"SAVE50"}
    code: PROMO_APPLIED → PROMO_CAPPED
    message: "Promotions applied" → "Discount capped at 25% of the cart"
    output.discount: 50 → 40
...
24 changed · 90 unchanged
```

Two things to read here. A plain `SAVE50` order at 200 is now capped, so the
coupon no longer gives 50 off. Decide if that is acceptable before shipping.
And the message still says 25%, so update it along with the number.
