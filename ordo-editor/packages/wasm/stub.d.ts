/**
 * WASM Stub Type Definitions
 */

export declare function execute_ruleset(
  ruleset_json: string,
  input_json: string,
  include_trace: boolean
): string;

export declare function validate_ruleset(ruleset_json: string): string;

export declare function eval_expression(
  expression: string,
  context_json: string
): string;

export default function init(): Promise<void>;
