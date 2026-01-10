/**
 * Engine-specific types
 * 引擎特定类型定义
 */

import type { Expr, Condition } from '../model';

/** Field missing behavior for engine */
export type FieldMissingBehavior = 'lenient' | 'strict' | 'default';

/** Engine RuleSet configuration */
export interface EngineRuleSetConfig {
  /** RuleSet name */
  name: string;
  /** RuleSet version */
  version: string;
  /** Description */
  description: string;
  /** Entry step ID (maps from startStepId) */
  entry_step: string;
  /** Field missing behavior */
  field_missing: FieldMissingBehavior;
  /** Max execution depth */
  max_depth: number;
  /** Timeout in milliseconds */
  timeout_ms: number;
  /** Enable execution trace */
  enable_trace: boolean;
  /** Custom metadata */
  metadata?: Record<string, string>;
}

/** Engine Step - uses discriminated union with "kind" */
export interface EngineStep {
  /** Step ID */
  id: string;
  /** Step name */
  name: string;
  /** Step kind (discriminated union) */
  kind: EngineStepKind;
}

/** Engine Step Kind */
export type EngineStepKind = 
  | { Decision: EngineDecisionKind }
  | { Action: EngineActionKind }
  | { Terminal: EngineTerminalKind };

/** Decision step kind */
export interface EngineDecisionKind {
  /** Branches */
  branches: EngineBranch[];
  /** Default next step */
  default_next: string;
}

/** Branch definition */
export interface EngineBranch {
  /** Branch ID */
  id: string;
  /** Condition */
  condition: Condition;
  /** Next step ID */
  next_step: string;
}

/** Action step kind */
export interface EngineActionKind {
  /** Actions to perform */
  actions: EngineAction[];
  /** Next step ID */
  next_step: string;
}

/** Action definition */
export type EngineAction =
  | { Assign: { var: string; value: Expr } }
  | { Log: { message: Expr; level?: string } }
  | { Call: EngineExternalCall };

/** External call */
export interface EngineExternalCall {
  /** Call type */
  call_type: 'http' | 'grpc' | 'function';
  /** Target */
  target: string;
  /** Parameters */
  params?: Record<string, Expr>;
  /** Result variable */
  result_var?: string;
}

/** Terminal step kind */
export interface EngineTerminalKind {
  /** Result code */
  code: string;
  /** Message expression */
  message: Expr | null;
  /** Output fields */
  output: EngineOutputField[];
}

/** Output field */
export interface EngineOutputField {
  /** Field name */
  name: string;
  /** Field value expression */
  value: Expr;
}

/** Engine RuleSet (complete format) */
export interface EngineRuleSet {
  /** Configuration */
  config: EngineRuleSetConfig;
  /** Steps as HashMap (id -> step) */
  steps: Record<string, EngineStep>;
}

/** Execution result */
export interface ExecutionResult {
  /** Result code */
  code: string;
  /** Result message */
  message: string;
  /** Output data */
  output: Record<string, any>;
  /** Execution duration in microseconds */
  duration_us: number;
  /** Execution trace (if requested) */
  trace?: ExecutionTrace;
}

/** Execution trace */
export interface ExecutionTrace {
  /** Path of executed steps */
  path: string;
  /** Step traces */
  steps: StepTrace[];
}

/** Step trace information */
export interface StepTrace {
  /** Step ID */
  id: string;
  /** Step name */
  name: string;
  /** Duration in microseconds */
  duration_us: number;
  /** Step result (for decision steps) */
  result?: string;
}

/** Validation result */
export interface ValidationResult {
  /** Whether validation passed */
  valid: boolean;
  /** Validation errors (if any) */
  errors?: string[];
}

/** Expression evaluation result */
export interface EvalResult {
  /** Result value */
  result: any;
  /** Parsed expression (for debugging) */
  parsed: string;
}

