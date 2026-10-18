# Embedding the Engine

You can run rules without a server: inside a Rust program with `ordo-core`, or in the browser with WebAssembly. The rule files are the same.

## Rust

```toml
[dependencies]
ordo-core = { git = "https://github.com/Ordo-Engine/Ordo" }
serde_json = "1"
```

```rust
use ordo_core::prelude::*;

fn main() -> Result<()> {
    let json = std::fs::read_to_string("rulesets/discount.json").unwrap();
    let ruleset = RuleSet::from_json_compiled(&json)?;
    let executor = RuleExecutor::new();

    let input: Value = serde_json::from_str(
        r#"{"user":{"tier":"gold"},"order":{"amount":1200}}"#,
    ).unwrap();
    let result = executor.execute(&ruleset, input)?;
    println!("{} {:?}", result.code, result.output);
    Ok(())
}
```

Load rules with `from_json_compiled` or `from_yaml_compiled`. They parse every expression once. Plain `from_json` leaves expressions as strings that are parsed again on every run.

Load the ruleset once and reuse it for every call.

## Browser (WebAssembly)

```bash
npm install @ordo-engine/wasm
```

```ts
import init, { execute_ruleset, validate_ruleset, eval_expression } from '@ordo-engine/wasm';

await init();

const result = JSON.parse(
  await execute_ruleset(JSON.stringify(ruleset), JSON.stringify(input), true),
);
console.log(result.code, result.output, result.trace?.path);

const check = JSON.parse(await validate_ruleset(JSON.stringify(ruleset)));
// { valid: true } or { valid: false, errors: [...] }

const { result: value } = JSON.parse(await eval_expression('a + 1', JSON.stringify({ a: 1 })));
// value is 2
```

All three take and return JSON strings. The last argument of `execute_ruleset` turns on the trace. Calls throw on a parse or execution error.

The [online playground](https://ordo-engine.github.io/Ordo/) runs on this package.

On WebAssembly the JIT is not available, and `external_call` actions have no providers.
