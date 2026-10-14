# 嵌入使用

不启动服务也能执行规则：在 Rust 程序里用 `ordo-core`，或者在浏览器里用 WebAssembly。规则文件是同一份。

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

用 `from_json_compiled` 或 `from_yaml_compiled` 加载规则，它们会把每个表达式解析一次。普通的 `from_json` 把表达式留成字符串，每次执行都要重新解析。

规则集加载一次，之后每次调用都复用它。

## 浏览器（WebAssembly）

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
// { valid: true } 或 { valid: false, errors: [...] }

const { result: value } = JSON.parse(await eval_expression('a + 1', JSON.stringify({ a: 1 })));
// value 是 2
```

三个函数的参数和返回值都是 JSON 字符串。`execute_ruleset` 的最后一个参数控制是否返回追踪。解析或执行出错时会抛异常。

[在线演练场](https://ordo-engine.github.io/Ordo/)就是用这个包跑的。

WebAssembly 下没有 JIT，`external_call` 动作也没有可用的能力。
