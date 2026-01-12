/**
 * WASM Module Type Definitions
 */

/**
 * Initialize the WASM module
 */
export default function init(): Promise<void>;

/**
 * Execute a ruleset with the given input
 */
export declare function execute_ruleset(
  ruleset_json: string,
  input_json: string,
  include_trace: boolean
): Promise<string>;

/**
 * Validate a ruleset
 */
export declare function validate_ruleset(ruleset_json: string): Promise<string>;

/**
 * Evaluate an expression with the given context
 */
export declare function eval_expression(expression: string, context_json: string): Promise<string>;
