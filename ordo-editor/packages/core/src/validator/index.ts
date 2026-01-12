/**
 * Rule validation
 * 规则验证器
 */

import {
  RuleSet,
  Step,
  getStepById,
  getAllStepIds,
  getNextStepIds,
  getBrokenReferences,
  getTerminalSteps,
} from '../model';

/** Validation error severity */
export type ValidationSeverity = 'error' | 'warning' | 'info';

/** Validation error */
export interface ValidationError {
  /** Error code */
  code: string;
  /** Error message */
  message: string;
  /** Severity level */
  severity: ValidationSeverity;
  /** Path to the error location (e.g., "steps[0].branches[1].condition") */
  path?: string;
  /** Related step ID */
  stepId?: string;
  /** Related branch ID */
  branchId?: string;
}

/** Validation result */
export interface ValidationResult {
  /** Whether the ruleset is valid */
  valid: boolean;
  /** List of errors */
  errors: ValidationError[];
  /** List of warnings */
  warnings: ValidationError[];
  /** List of info messages */
  infos: ValidationError[];
}

/** Validation options */
export interface ValidationOptions {
  /** Whether to check for unreachable steps */
  checkUnreachable?: boolean;
  /** Whether to check for circular references */
  checkCircular?: boolean;
  /** Whether to check schema compliance */
  checkSchema?: boolean;
  /** Maximum step count */
  maxSteps?: number;
  /** Maximum branch count per decision step */
  maxBranches?: number;
}

const DEFAULT_OPTIONS: ValidationOptions = {
  checkUnreachable: true,
  checkCircular: true,
  checkSchema: false,
  maxSteps: 100,
  maxBranches: 20,
};

/**
 * Validate a ruleset
 */
export function validateRuleSet(
  ruleset: RuleSet,
  options: ValidationOptions = {}
): ValidationResult {
  const opts = { ...DEFAULT_OPTIONS, ...options };
  const errors: ValidationError[] = [];
  const warnings: ValidationError[] = [];
  const infos: ValidationError[] = [];

  const addError = (error: Omit<ValidationError, 'severity'>) => {
    errors.push({ ...error, severity: 'error' });
  };

  const addWarning = (error: Omit<ValidationError, 'severity'>) => {
    warnings.push({ ...error, severity: 'warning' });
  };

  const addInfo = (error: Omit<ValidationError, 'severity'>) => {
    infos.push({ ...error, severity: 'info' });
  };

  // Check basic structure
  if (!ruleset.config.name) {
    addError({
      code: 'MISSING_NAME',
      message: 'RuleSet name is required',
      path: 'config.name',
    });
  }

  if (ruleset.steps.length === 0) {
    addError({
      code: 'NO_STEPS',
      message: 'RuleSet must have at least one step',
      path: 'steps',
    });
  }

  // Check step count
  if (opts.maxSteps && ruleset.steps.length > opts.maxSteps) {
    addWarning({
      code: 'TOO_MANY_STEPS',
      message: `RuleSet has ${ruleset.steps.length} steps, maximum recommended is ${opts.maxSteps}`,
      path: 'steps',
    });
  }

  // Check start step
  if (!ruleset.startStepId) {
    addError({
      code: 'MISSING_START_STEP',
      message: 'Start step ID is required',
      path: 'startStepId',
    });
  } else if (!getStepById(ruleset, ruleset.startStepId)) {
    addError({
      code: 'INVALID_START_STEP',
      message: `Start step "${ruleset.startStepId}" does not exist`,
      path: 'startStepId',
    });
  }

  // Check for duplicate step IDs
  const stepIds = getAllStepIds(ruleset);
  const duplicateIds = stepIds.filter((id, i) => stepIds.indexOf(id) !== i);
  for (const id of new Set(duplicateIds)) {
    addError({
      code: 'DUPLICATE_STEP_ID',
      message: `Duplicate step ID: "${id}"`,
      stepId: id,
    });
  }

  // Check each step
  for (let i = 0; i < ruleset.steps.length; i++) {
    const step = ruleset.steps[i];
    validateStep(step, i, ruleset, opts, addError, addWarning, addInfo);
  }

  // Check broken references
  const brokenRefs = getBrokenReferences(ruleset);
  for (const { stepId, missingId } of brokenRefs) {
    addError({
      code: 'BROKEN_REFERENCE',
      message: `Step "${stepId}" references non-existent step "${missingId}"`,
      stepId,
    });
  }

  // Check for terminal steps
  const terminals = getTerminalSteps(ruleset);
  if (terminals.length === 0) {
    addWarning({
      code: 'NO_TERMINAL_STEPS',
      message: 'RuleSet has no terminal steps',
      path: 'steps',
    });
  }

  // Check for unreachable steps
  if (opts.checkUnreachable && ruleset.startStepId) {
    const reachable = getReachableSteps(ruleset);
    for (const step of ruleset.steps) {
      if (!reachable.has(step.id)) {
        addWarning({
          code: 'UNREACHABLE_STEP',
          message: `Step "${step.id}" is not reachable from start`,
          stepId: step.id,
        });
      }
    }
  }

  // Check for circular references
  if (opts.checkCircular && ruleset.startStepId) {
    const cycles = detectCycles(ruleset);
    for (const cycle of cycles) {
      addWarning({
        code: 'CIRCULAR_REFERENCE',
        message: `Circular reference detected: ${cycle.join(' -> ')}`,
        stepId: cycle[0],
      });
    }
  }

  return {
    valid: errors.length === 0,
    errors,
    warnings,
    infos,
  };
}

/** Validate a single step */
function validateStep(
  step: Step,
  index: number,
  ruleset: RuleSet,
  opts: ValidationOptions,
  addError: (e: Omit<ValidationError, 'severity'>) => void,
  addWarning: (e: Omit<ValidationError, 'severity'>) => void,
  _addInfo: (e: Omit<ValidationError, 'severity'>) => void
): void {
  const basePath = `steps[${index}]`;

  // Check step ID
  if (!step.id) {
    addError({
      code: 'MISSING_STEP_ID',
      message: 'Step ID is required',
      path: `${basePath}.id`,
    });
  }

  // Check step name
  if (!step.name) {
    addWarning({
      code: 'MISSING_STEP_NAME',
      message: `Step "${step.id}" has no name`,
      path: `${basePath}.name`,
      stepId: step.id,
    });
  }

  // Type-specific validation
  switch (step.type) {
    case 'decision':
      validateDecisionStep(step, basePath, opts, addError, addWarning);
      break;

    case 'action':
      validateActionStep(step, basePath, addError, addWarning);
      break;

    case 'terminal':
      validateTerminalStep(step, basePath, addError, addWarning);
      break;

    default:
      addError({
        code: 'INVALID_STEP_TYPE',
        message: `Invalid step type: "${(step as Step).type}"`,
        path: `${basePath}.type`,
        stepId: step.id,
      });
  }
}

/** Validate a decision step */
function validateDecisionStep(
  step: { id: string; branches: { id: string; nextStepId: string }[]; defaultNextStepId: string },
  basePath: string,
  opts: ValidationOptions,
  addError: (e: Omit<ValidationError, 'severity'>) => void,
  addWarning: (e: Omit<ValidationError, 'severity'>) => void
): void {
  // Check default next step
  if (!step.defaultNextStepId) {
    addError({
      code: 'MISSING_DEFAULT_NEXT',
      message: `Decision step "${step.id}" has no default next step`,
      path: `${basePath}.defaultNextStepId`,
      stepId: step.id,
    });
  }

  // Check branches
  if (step.branches.length === 0) {
    addWarning({
      code: 'NO_BRANCHES',
      message: `Decision step "${step.id}" has no branches`,
      path: `${basePath}.branches`,
      stepId: step.id,
    });
  }

  if (opts.maxBranches && step.branches.length > opts.maxBranches) {
    addWarning({
      code: 'TOO_MANY_BRANCHES',
      message: `Decision step "${step.id}" has ${step.branches.length} branches, maximum recommended is ${opts.maxBranches}`,
      path: `${basePath}.branches`,
      stepId: step.id,
    });
  }

  // Check each branch
  for (let i = 0; i < step.branches.length; i++) {
    const branch = step.branches[i];
    if (!branch.nextStepId) {
      addError({
        code: 'MISSING_BRANCH_NEXT',
        message: `Branch ${i} of step "${step.id}" has no next step`,
        path: `${basePath}.branches[${i}].nextStepId`,
        stepId: step.id,
        branchId: branch.id,
      });
    }
  }
}

/** Validate an action step */
function validateActionStep(
  step: { id: string; nextStepId: string },
  basePath: string,
  addError: (e: Omit<ValidationError, 'severity'>) => void,
  _addWarning: (e: Omit<ValidationError, 'severity'>) => void
): void {
  if (!step.nextStepId) {
    addError({
      code: 'MISSING_NEXT_STEP',
      message: `Action step "${step.id}" has no next step`,
      path: `${basePath}.nextStepId`,
      stepId: step.id,
    });
  }
}

/** Validate a terminal step */
function validateTerminalStep(
  step: { id: string; code: string },
  basePath: string,
  addError: (e: Omit<ValidationError, 'severity'>) => void,
  _addWarning: (e: Omit<ValidationError, 'severity'>) => void
): void {
  if (!step.code) {
    addError({
      code: 'MISSING_RESULT_CODE',
      message: `Terminal step "${step.id}" has no result code`,
      path: `${basePath}.code`,
      stepId: step.id,
    });
  }
}

/** Get all reachable steps from start */
function getReachableSteps(ruleset: RuleSet): Set<string> {
  const reachable = new Set<string>();
  const queue = [ruleset.startStepId];

  while (queue.length > 0) {
    const stepId = queue.shift()!;
    if (reachable.has(stepId)) continue;

    const step = getStepById(ruleset, stepId);
    if (!step) continue;

    reachable.add(stepId);
    queue.push(...getNextStepIds(step));
  }

  return reachable;
}

/** Detect cycles in the ruleset */
function detectCycles(ruleset: RuleSet): string[][] {
  const cycles: string[][] = [];
  const visited = new Set<string>();
  const recursionStack = new Set<string>();
  const path: string[] = [];

  function dfs(stepId: string): void {
    if (recursionStack.has(stepId)) {
      // Found a cycle
      const cycleStart = path.indexOf(stepId);
      if (cycleStart !== -1) {
        cycles.push([...path.slice(cycleStart), stepId]);
      }
      return;
    }

    if (visited.has(stepId)) return;

    const step = getStepById(ruleset, stepId);
    if (!step) return;

    visited.add(stepId);
    recursionStack.add(stepId);
    path.push(stepId);

    for (const nextId of getNextStepIds(step)) {
      dfs(nextId);
    }

    path.pop();
    recursionStack.delete(stepId);
  }

  dfs(ruleset.startStepId);
  return cycles;
}

/** Quick validation check (returns true/false) */
export function isValidRuleSet(ruleset: RuleSet): boolean {
  return validateRuleSet(ruleset, { checkUnreachable: false, checkCircular: false }).valid;
}
