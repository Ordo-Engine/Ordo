/* tslint:disable */
/* eslint-disable */

/**
 * Analyze a single expression for JIT compatibility
 *
 * # Arguments
 * * `expression` - Expression string to analyze
 *
 * # Returns
 * JSON string containing JITExprAnalysis
 */
export function analyze_jit_compatibility(expression: string): string;

/**
 * Analyze an entire ruleset for JIT compatibility
 *
 * # Arguments
 * * `ruleset_json` - RuleSet definition as JSON string
 *
 * # Returns
 * JSON string containing JITRulesetAnalysis
 */
export function analyze_ruleset_jit(ruleset_json: string): string;

/**
 * Compile a ruleset to binary format (.ordo)
 *
 * # Arguments
 * * `ruleset_json` - RuleSet definition as JSON string
 *
 * # Returns
 * Binary data as Uint8Array
 */
export function compile_ruleset(ruleset_json: string): Uint8Array;

/**
 * Evaluate an expression with given context
 *
 * # Arguments
 * * `expression` - Expression string to evaluate
 * * `context_json` - Context data as JSON string
 *
 * # Returns
 * JSON string containing the evaluation result and parsed expression
 */
export function eval_expression(expression: string, context_json: string): string;

/**
 * Execute a compiled ruleset (binary format)
 *
 * # Arguments
 * * `compiled_bytes` - Compiled ruleset binary data
 * * `input_json` - Input data as JSON string
 * * `include_trace` - Whether to include execution trace (not supported for compiled, ignored)
 *
 * # Returns
 * JSON string containing the execution result
 */
export function execute_compiled_ruleset(compiled_bytes: Uint8Array, input_json: string): string;

/**
 * Execute a ruleset with given input
 *
 * # Arguments
 * * `ruleset_json` - RuleSet definition as JSON string
 * * `input_json` - Input data as JSON string
 * * `include_trace` - Whether to include execution trace
 *
 * # Returns
 * JSON string containing the execution result
 */
export function execute_ruleset(ruleset_json: string, input_json: string, include_trace: boolean): string;

/**
 * Get compiled ruleset info (metadata)
 *
 * # Arguments
 * * `compiled_bytes` - Compiled ruleset binary data
 *
 * # Returns
 * JSON string containing metadata
 */
export function get_compiled_ruleset_info(compiled_bytes: Uint8Array): string;

/**
 * Initialize panic hook for better error messages in the browser console
 */
export function init(): void;

/**
 * Validate a ruleset
 *
 * # Arguments
 * * `ruleset_json` - RuleSet definition as JSON string
 *
 * # Returns
 * JSON string containing validation result: `{"valid": true}` or `{"valid": false, "errors": [...]}`
 */
export function validate_ruleset(ruleset_json: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly analyze_jit_compatibility: (a: number, b: number) => [number, number, number, number];
  readonly analyze_ruleset_jit: (a: number, b: number) => [number, number, number, number];
  readonly compile_ruleset: (a: number, b: number) => [number, number, number, number];
  readonly eval_expression: (a: number, b: number, c: number, d: number) => [number, number, number, number];
  readonly execute_compiled_ruleset: (a: number, b: number, c: number, d: number) => [number, number, number, number];
  readonly execute_ruleset: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
  readonly get_compiled_ruleset_info: (a: number, b: number) => [number, number, number, number];
  readonly validate_ruleset: (a: number, b: number) => [number, number, number, number];
  readonly init: () => void;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_externrefs: WebAssembly.Table;
  readonly __externref_table_dealloc: (a: number) => void;
  readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
