# Facts, Concepts and Contracts

These describe what a project's rules read and return, and Studio uses them when you edit rules and tests. In a CLI project they are `facts.json`, `concepts.json` and `contracts/<name>.json`.

## Facts

A fact is an input field that comes from outside, such as `user.age` or `order.amount`.

| Field         | Meaning                                                   |
| ------------- | --------------------------------------------------------- |
| `name`        | The field path rules use                                  |
| `data_type`   | `string`, `number`, `boolean`, `date` or `object`          |
| `source`      | Where the value comes from, for people reading the catalog |
| `null_policy` | `error`, `default` or `skip`                               |
| `description`, `owner` | Free text                                        |

Callers send facts in the `input` of the execute call. A fact that is not sent counts as missing, not as an error.

## Concepts

A concept is a named value computed from facts, such as `is_adult` from `user.age >= 18`. It has a `name`, a `data_type`, an `expression` and the `dependencies` it reads. Concepts are computed before the rules run, the same way in the CLI and on the platform, so a local `ordo test` gives the same answer as production.

## Contracts

A decision contract states, for one ruleset, which input fields it expects and which output fields it returns, each with a type and whether it is required. It also records an owner, a version pattern, an optional p99 latency target and notes. Use it to agree with the calling team on what goes in and what comes out.

## Sub-rules

A sub-rule is a piece of logic several rulesets share, such as a KYC check or a risk score. It can belong to a project, or to the organization so several projects can use it. A ruleset calls it with a sub-rule step, passing inputs in and mapping outputs back (see [Rule Structure](/en/guide/rule-structure#sub-rule)).

When a ruleset is published, the platform copies the current version of every sub-rule it uses into it, up to 8 levels deep. Later edits to a sub-rule do not change rulesets that are already published. Republish those rulesets to pick up the change.
