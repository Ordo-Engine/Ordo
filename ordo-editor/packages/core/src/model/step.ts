/**
 * Step types for the Ordo rule engine
 * 步骤类型定义
 */

import { Condition } from './condition';
import { Expr } from './expr';

/** Step types */
export type StepType = 'decision' | 'action' | 'terminal' | 'sub_rule';

/** Base step interface */
export interface BaseStep {
  /** Unique step ID */
  id: string;
  /** Display name */
  name: string;
  /** Optional description */
  description?: string;
  /** Step type */
  type: StepType;
  /** Position in the flow editor (for visualization) */
  position?: {
    x: number;
    y: number;
  };
  /** Internal runtime marker; generated steps should not surface as authoring suggestions. */
  systemGenerated?: 'sub_rule_runtime' | 'concept_runtime';
}

/** Branch definition for decision steps */
export interface Branch {
  /** Unique branch ID */
  id: string;
  /** Branch label (for display) */
  label?: string;
  /** Condition to evaluate */
  condition: Condition;
  /** Next step ID if condition is true */
  nextStepId: string;
}

/** Decision step - evaluates conditions and branches */
export interface DecisionStep extends BaseStep {
  type: 'decision';
  /** Ordered list of branches (evaluated in order) */
  branches: Branch[];
  /** Default next step if no branch matches */
  defaultNextStepId: string;
}

/** Variable assignment */
export interface VariableAssignment {
  /** Variable name (without $ prefix) */
  name: string;
  /** Value expression */
  value: Expr;
}

/** External call configuration */
export interface ExternalCall {
  /** Call type */
  type: 'http' | 'grpc' | 'function';
  /** Target URL or function name */
  target: string;
  /** Request parameters */
  params?: Record<string, Expr>;
  /** Variable to store result */
  resultVariable?: string;
  /** Timeout in milliseconds */
  timeout?: number;
  /** Retry configuration */
  retry?: {
    maxAttempts: number;
    backoffMs: number;
  };
  /** Error handling */
  onError?: 'fail' | 'ignore' | 'fallback';
  /** Fallback value on error */
  fallbackValue?: Expr;
}

/** Action step - performs operations */
export interface ActionStep extends BaseStep {
  type: 'action';
  /** Variable assignments */
  assignments?: VariableAssignment[];
  /** External calls */
  externalCalls?: ExternalCall[];
  /** Logging configuration */
  logging?: {
    message: Expr;
    level?: 'debug' | 'info' | 'warn' | 'error';
  };
  /** Next step ID */
  nextStepId: string;
}

/** Output field definition */
export interface OutputField {
  /** Field name */
  name: string;
  /** Value expression */
  value: Expr;
}

/** Terminal step - ends execution with a result */
export interface TerminalStep extends BaseStep {
  type: 'terminal';
  /** Result code (e.g., "APPROVED", "REJECTED") */
  code: string;
  /** Result message (can be expression) */
  message?: Expr;
  /** Output fields */
  output?: OutputField[];
}

/** Input binding for a sub-rule: inject a parent-context expression into a named input field */
export interface SubRuleBinding {
  /** Field name in the child context */
  field: string;
  /** Expression evaluated in the parent context */
  expr: Expr;
}

/** Output mapping for a sub-rule: copy a child variable back to a parent variable */
export interface SubRuleOutput {
  /** Variable name in the parent context (without $ prefix) */
  parentVar: string;
  /** Variable name in the child context (without $ prefix) */
  childVar: string;
}

/** Managed SubRule asset reference. Sub-rules are snapshotted inline when the parent is published. */
export interface SubRuleAssetRef {
  scope: 'project' | 'org';
  name: string;
}

/** Sub-rule step - executes a managed SubRule asset and returns control to the parent */
export interface SubRuleStep extends BaseStep {
  type: 'sub_rule';
  /** Legacy/engine-compatible reference name. Prefer assetRef for Studio assets. */
  refName: string;
  /** Managed SubRule asset reference used by Studio and platform resolution. */
  assetRef?: SubRuleAssetRef;
  /** Input bindings: expressions from the parent context injected into the child context */
  bindings?: SubRuleBinding[];
  /** Output mappings: variables from the child context written back to the parent context */
  outputs?: SubRuleOutput[];
  /** Authoring-level return semantics. Execution lowering materializes runtime control flow. */
  returnPolicy?: 'continue' | 'propagate_terminal';
  /** Next step ID after the sub-rule completes. Empty means the child terminal result ends the parent flow. */
  nextStepId: string;
}

/** Step union type */
export type StepUnion = DecisionStep | ActionStep | TerminalStep | SubRuleStep;

// ============================================================================
// Step builder helpers
// ============================================================================

let stepIdCounter = 0;

function generateStepId(): string {
  return `step_${++stepIdCounter}`;
}

function generateBranchId(): string {
  return `branch_${++stepIdCounter}`;
}

/** Alias for Step type (backward compatibility) */
export type Step = StepUnion;

export const Step = {
  /** Reset ID counter (for testing) */
  resetIdCounter() {
    stepIdCounter = 0;
  },

  /** Create a decision step */
  decision(options: {
    id?: string;
    name: string;
    description?: string;
    branches?: Branch[];
    defaultNextStepId: string;
    position?: { x: number; y: number };
    systemGenerated?: BaseStep['systemGenerated'];
  }): DecisionStep {
    return {
      id: options.id || generateStepId(),
      name: options.name,
      description: options.description,
      type: 'decision',
      branches: options.branches || [],
      defaultNextStepId: options.defaultNextStepId,
      position: options.position,
      systemGenerated: options.systemGenerated,
    };
  },

  /** Create an action step */
  action(options: {
    id?: string;
    name: string;
    description?: string;
    assignments?: VariableAssignment[];
    externalCalls?: ExternalCall[];
    logging?: ActionStep['logging'];
    nextStepId: string;
    position?: { x: number; y: number };
    systemGenerated?: BaseStep['systemGenerated'];
  }): ActionStep {
    return {
      id: options.id || generateStepId(),
      name: options.name,
      description: options.description,
      type: 'action',
      assignments: options.assignments,
      externalCalls: options.externalCalls,
      logging: options.logging,
      nextStepId: options.nextStepId,
      position: options.position,
      systemGenerated: options.systemGenerated,
    };
  },

  /** Create a terminal step */
  terminal(options: {
    id?: string;
    name: string;
    description?: string;
    code: string;
    message?: Expr;
    output?: OutputField[];
    position?: { x: number; y: number };
    systemGenerated?: BaseStep['systemGenerated'];
  }): TerminalStep {
    return {
      id: options.id || generateStepId(),
      name: options.name,
      description: options.description,
      type: 'terminal',
      code: options.code,
      message: options.message,
      output: options.output,
      position: options.position,
      systemGenerated: options.systemGenerated,
    };
  },

  /** Create a branch */
  branch(options: {
    id?: string;
    label?: string;
    condition: Condition;
    nextStepId: string;
  }): Branch {
    return {
      id: options.id || generateBranchId(),
      label: options.label,
      condition: options.condition,
      nextStepId: options.nextStepId,
    };
  },

  /** Create a sub-rule step */
  subRule(options: {
    id?: string;
    name: string;
    description?: string;
    refName: string;
    assetRef?: SubRuleAssetRef;
    bindings?: SubRuleBinding[];
    outputs?: SubRuleOutput[];
    returnPolicy?: SubRuleStep['returnPolicy'];
    nextStepId: string;
    position?: { x: number; y: number };
    systemGenerated?: BaseStep['systemGenerated'];
  }): SubRuleStep {
    return {
      id: options.id || generateStepId(),
      name: options.name,
      description: options.description,
      type: 'sub_rule',
      refName: options.refName,
      assetRef: options.assetRef,
      bindings: options.bindings,
      outputs: options.outputs,
      returnPolicy: options.nextStepId ? options.returnPolicy ?? 'continue' : 'propagate_terminal',
      nextStepId: options.nextStepId,
      position: options.position,
      systemGenerated: options.systemGenerated,
    };
  },

  /** Create a variable assignment */
  assign(name: string, value: Expr): VariableAssignment {
    return { name, value };
  },

  /** Create an output field */
  output(name: string, value: Expr): OutputField {
    return { name, value };
  },
};

/** Check if a step is a decision step */
export function isDecisionStep(step: Step): step is DecisionStep {
  return step.type === 'decision';
}

/** Check if a step is an action step */
export function isActionStep(step: Step): step is ActionStep {
  return step.type === 'action';
}

/** Check if a step is a terminal step */
export function isTerminalStep(step: Step): step is TerminalStep {
  return step.type === 'terminal';
}

/** Check if a step is a sub-rule step */
export function isSubRuleStep(step: Step): step is SubRuleStep {
  return step.type === 'sub_rule';
}

/** Get the next step IDs from a step */
export function getNextStepIds(step: Step): string[] {
  switch (step.type) {
    case 'decision':
      return [...step.branches.map((b) => b.nextStepId), step.defaultNextStepId].filter(
        (id, i, arr) => arr.indexOf(id) === i
      ); // unique

    case 'action':
      return [step.nextStepId];

    case 'terminal':
      return [];

    case 'sub_rule':
      return step.nextStepId ? [step.nextStepId] : [];
  }
}
