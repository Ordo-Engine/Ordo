/**
 * WASM Stub - Placeholder for when WASM module is not built
 * 
 * This file is used during development or when the Rust WASM module
 * hasn't been compiled. The actual WASM module should be built using:
 *   cd crates/ordo-wasm && ./build.sh
 */

// Throw an error when any function is called
const notAvailable = (name) => () => {
  throw new Error(
    `WASM module not available. Function '${name}' requires the compiled WASM module. ` +
    `Please build it using: cd crates/ordo-wasm && ./build.sh`
  );
};

export const execute_ruleset = notAvailable('execute_ruleset');
export const validate_ruleset = notAvailable('validate_ruleset');
export const eval_expression = notAvailable('eval_expression');
export const analyze_jit_compatibility = notAvailable('analyze_jit_compatibility');
export const analyze_ruleset_jit = notAvailable('analyze_ruleset_jit');

// Default export for init function
export default async function init() {
  console.warn(
    '[Ordo] WASM module not available. Rule execution will only work in HTTP mode.'
  );
}
