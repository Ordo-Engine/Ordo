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

// ============================================================================
// JIT Compatibility Analysis Types
// ============================================================================

/**
 * Result of JIT compatibility analysis for a single expression
 */
export interface JITExprAnalysis {
  /** Whether the expression is JIT-compatible */
  jit_compatible: boolean;
  /** Reason for incompatibility (if not compatible) */
  reason: string | null;
  /** List of fields accessed by the expression */
  accessed_fields: string[];
  /** Unsupported features found in the expression */
  unsupported_features: string[];
  /** Supported features used in the expression */
  supported_features: string[];
}

/**
 * Entry for a single expression analysis in ruleset
 */
export interface JITExpressionEntry {
  /** Step ID containing this expression */
  step_id: string;
  /** Step name */
  step_name: string;
  /** Type of expression location (condition, assignment, etc.) */
  location: string;
  /** The expression string */
  expression: string;
  /** Analysis result */
  analysis: JITExprAnalysis;
}

/**
 * Information about a required field for JIT
 */
export interface RequiredFieldInfo {
  /** Field path (e.g., "user.age") */
  path: string;
  /** Inferred type from usage */
  inferred_type: string;
  /** Steps that access this field */
  used_in_steps: string[];
}

/**
 * Complete JIT analysis result for a ruleset
 */
export interface JITRulesetAnalysis {
  /** Overall JIT compatibility (all expressions must be compatible) */
  overall_compatible: boolean;
  /** Number of JIT-compatible expressions */
  compatible_count: number;
  /** Number of incompatible expressions */
  incompatible_count: number;
  /** Total number of expressions analyzed */
  total_expressions: number;
  /** Analysis of individual expressions */
  expressions: JITExpressionEntry[];
  /** Estimated performance improvement (1.0 = no improvement) */
  estimated_speedup: number;
  /** Summary of required schema fields */
  required_fields: RequiredFieldInfo[];
}

/**
 * Analyze a single expression for JIT compatibility
 * @param expression - Expression string to analyze
 * @returns JSON string containing JITExprAnalysis
 */
export declare function analyze_jit_compatibility(expression: string): Promise<string>;

/**
 * Analyze an entire ruleset for JIT compatibility
 * @param ruleset_json - RuleSet definition as JSON string
 * @returns JSON string containing JITRulesetAnalysis
 */
export declare function analyze_ruleset_jit(ruleset_json: string): Promise<string>;
