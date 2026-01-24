/**
 * WASM Module Loader
 * 
 * This module attempts to load the compiled WASM module.
 * If it's not available, it falls back to the stub with a warning.
 */

let wasmModule = null;
let initPromise = null;

// Try to import the compiled WASM module
async function loadWasm() {
  try {
    // Dynamic import of the compiled WASM
    const wasm = await import('./dist/ordo_wasm.js');
    return wasm;
  } catch (e) {
    // Fall back to stub
    console.warn('[Ordo] Compiled WASM not found, using stub. WASM mode will not work.');
    const stub = await import('./stub.js');
    return stub;
  }
}

// Initialize and cache the module
async function ensureLoaded() {
  if (!initPromise) {
    initPromise = loadWasm().then(async (mod) => {
      wasmModule = mod;
      // Call init if it exists and is a function
      if (typeof mod.default === 'function') {
        await mod.default();
      }
      return mod;
    });
  }
  return initPromise;
}

// Default export - init function
export default async function init() {
  await ensureLoaded();
}

// Re-export functions that delegate to the loaded module
export async function execute_ruleset(ruleset_json, input_json, include_trace) {
  await ensureLoaded();
  return wasmModule.execute_ruleset(ruleset_json, input_json, include_trace);
}

export async function validate_ruleset(ruleset_json) {
  await ensureLoaded();
  return wasmModule.validate_ruleset(ruleset_json);
}

export async function eval_expression(expression, context_json) {
  await ensureLoaded();
  return wasmModule.eval_expression(expression, context_json);
}

// JIT Compatibility Analysis Functions
export async function analyze_jit_compatibility(expression) {
  await ensureLoaded();
  return wasmModule.analyze_jit_compatibility(expression);
}

export async function analyze_ruleset_jit(ruleset_json) {
  await ensureLoaded();
  return wasmModule.analyze_ruleset_jit(ruleset_json);
}

// Compiled RuleSet Functions
export async function compile_ruleset(ruleset_json) {
  await ensureLoaded();
  return wasmModule.compile_ruleset(ruleset_json);
}

export async function execute_compiled_ruleset(compiled_bytes, input_json) {
  await ensureLoaded();
  return wasmModule.execute_compiled_ruleset(compiled_bytes, input_json);
}

export async function get_compiled_ruleset_info(compiled_bytes) {
  await ensureLoaded();
  return wasmModule.get_compiled_ruleset_info(compiled_bytes);
}
