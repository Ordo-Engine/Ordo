/**
 * Format adapter for converting between Editor and Engine formats
 * 编辑器与引擎格式转换适配器
 *
 * The Rust engine expects a specific JSON format:
 * - Steps are stored in a HashMap<String, Step>
 * - StepKind uses #[serde(tag = "type")] so format is { "type": "decision", ... }
 * - Condition uses #[serde(untagged)] so string conditions are just strings
 * - Terminal steps have a "result" field containing code, message, output
 */

import type {
  RuleSet,
  Step,
  DecisionStep,
  ActionStep,
  TerminalStep,
  Branch as EditorBranch,
} from '../model';

/**
 * Engine RuleSet format (matches Rust ordo-core::RuleSet)
 */
interface EngineRuleSet {
  config: {
    name: string;
    version: string;
    description: string;
    entry_step: string;
    field_missing: 'lenient' | 'strict' | 'default';
    max_depth: number;
    timeout_ms: number;
    enable_trace: boolean;
    metadata: Record<string, string>;
  };
  steps: Record<string, EngineStep>;
}

/**
 * Engine Step format
 */
interface EngineStep {
  id: string;
  name: string;
  // Flattened StepKind fields - one of these will be present based on "type"
  type: 'decision' | 'action' | 'terminal';
  // Decision fields
  branches?: EngineBranch[];
  default_next?: string | null;
  // Action fields
  actions?: EngineAction[];
  next_step?: string;
  // Terminal fields
  result?: EngineTerminalResult;
}

interface EngineBranch {
  condition: string; // Expression string (untagged, so just a string)
  next_step: string;
  actions?: EngineAction[];
}

interface EngineAction {
  action: 'set_variable' | 'log' | 'metric';
  // set_variable fields
  name?: string;
  value?: any; // Expr
  // log fields
  message?: string;
  level?: 'debug' | 'info' | 'warn' | 'error';
  description?: string;
}

interface EngineTerminalResult {
  code: string;
  message: string;
  output: Array<[string, any]>; // Vec<(String, Expr)>
  data: any; // Value
}

/**
 * Convert Editor RuleSet to Engine format
 * 将编辑器 RuleSet 转换为引擎格式
 */
export function convertToEngineFormat(editorRuleset: RuleSet): EngineRuleSet {
  // Build steps map
  const stepsMap: Record<string, EngineStep> = {};
  for (const step of editorRuleset.steps) {
    stepsMap[step.id] = convertStep(step);
  }

  // Build config
  const config = {
    name: editorRuleset.config.name || 'unnamed',
    version: editorRuleset.config.version || '1.0.0',
    description: editorRuleset.config.description || '',
    entry_step: editorRuleset.startStepId || '',
    field_missing: 'lenient' as const,
    max_depth: 100,
    timeout_ms: editorRuleset.config.timeout || 0,
    enable_trace: editorRuleset.config.enableTrace ?? true,
    metadata: {},
  };

  return {
    config,
    steps: stepsMap,
  };
}

/**
 * Convert a single step from Editor to Engine format
 */
function convertStep(step: Step): EngineStep {
  switch (step.type) {
    case 'decision':
      return convertDecisionStep(step as DecisionStep);
    case 'action':
      return convertActionStep(step as ActionStep);
    case 'terminal':
      return convertTerminalStep(step as TerminalStep);
    default:
      throw new Error(`Unknown step type: ${(step as any).type}`);
  }
}

/**
 * Convert decision step
 */
function convertDecisionStep(step: DecisionStep): EngineStep {
  return {
    id: step.id,
    name: step.name,
    type: 'decision',
    branches: (step.branches || []).map(convertBranch),
    default_next: step.defaultNextStepId || null,
  };
}

/**
 * Convert branch
 */
function convertBranch(branch: EditorBranch): EngineBranch {
  const conditionStr = convertConditionToString(branch.condition);
  console.log('[Adapter] Branch condition:', branch.condition, '→', conditionStr);
  return {
    // Condition needs to be converted to a string expression
    condition: conditionStr,
    next_step: branch.nextStepId,
    actions: [],
  };
}

/**
 * Convert condition object to expression string
 * Editor stores conditions as objects like:
 * { type: 'simple', left: {...}, operator: 'eq', right: {...} }
 * Engine expects a string like: "user.level == 'vip'"
 */
function convertConditionToString(condition: any): string {
  if (!condition) {
    return 'true';
  }

  // If it's already a string, return it
  if (typeof condition === 'string') {
    return condition;
  }

  // Handle simple condition type
  if (condition.type === 'simple') {
    const left = convertValueToExprString(condition.left);
    const right = convertValueToExprString(condition.right);
    const op = convertOperator(condition.operator);
    return `${left} ${op} ${right}`;
  }

  // Handle compound conditions (and/or)
  if (condition.type === 'compound' || condition.type === 'and' || condition.type === 'or') {
    const operator = condition.operator || condition.type;
    const conditions = (condition.conditions || []).map(convertConditionToString);
    if (conditions.length === 0) return 'true';
    if (conditions.length === 1) return conditions[0];
    const joinOp = operator === 'and' ? ' && ' : ' || ';
    return `(${conditions.join(joinOp)})`;
  }

  // Handle expression type
  if (condition.type === 'expression' && condition.expression) {
    return condition.expression;
  }

  // Fallback: try to stringify or return 'true'
  console.warn('Unknown condition format:', condition);
  return 'true';
}

/**
 * Convert a value object to expression string
 */
function convertValueToExprString(value: any): string {
  if (!value) return 'null';

  if (typeof value === 'string') return value;
  if (typeof value === 'number') return String(value);
  if (typeof value === 'boolean') return String(value);

  // Handle variable reference
  if (value.type === 'variable' || value.type === 'field') {
    let path = value.path || value.name || '';
    // Remove $. prefix - engine Context.get() uses bare paths like "order.amount"
    // The Context stores input data at root level, not under "input." key
    if (path.startsWith('$.')) {
      path = path.slice(2); // $.order.amount -> order.amount
    } else if (path.startsWith('$')) {
      path = path.slice(1);
    }
    return path;
  }

  // Handle literal value
  if (value.type === 'literal') {
    const v = value.value;
    if (typeof v === 'string') return JSON.stringify(v);
    if (typeof v === 'number') return String(v);
    if (typeof v === 'boolean') return String(v);
    if (v === null) return 'null';
    return JSON.stringify(v);
  }

  // Handle expression
  if (value.type === 'expression' && value.expression) {
    return value.expression;
  }

  // Fallback
  return JSON.stringify(value);
}

/**
 * Convert operator to expression operator
 */
function convertOperator(op: string): string {
  const operatorMap: Record<string, string> = {
    eq: '==',
    ne: '!=',
    neq: '!=',
    gt: '>',
    gte: '>=',
    ge: '>=',
    lt: '<',
    lte: '<=',
    le: '<=',
    contains: 'contains',
    startsWith: 'startsWith',
    endsWith: 'endsWith',
    in: 'in',
    notIn: 'not in',
  };
  return operatorMap[op] || op;
}

/**
 * Convert action step
 */
function convertActionStep(step: ActionStep): EngineStep {
  const actions: EngineAction[] = [];

  // Convert variable assignments
  if (step.assignments) {
    for (const assignment of step.assignments) {
      actions.push({
        action: 'set_variable',
        name: assignment.name,
        value: convertToEngineExpr(assignment.value),
        description: '',
      });
    }
  }

  // Convert logging
  if (step.logging) {
    // Extract message string from logging object
    let message = step.logging.message;
    if (typeof message === 'object' && message !== null) {
      if (message.type === 'literal') {
        message = String(message.value);
      } else {
        message = JSON.stringify(message);
      }
    }

    actions.push({
      action: 'log',
      message: message,
      level: (step.logging.level as any) || 'info',
      description: '',
    });
  }

  return {
    id: step.id,
    name: step.name,
    type: 'action',
    actions,
    next_step: step.nextStepId || '',
  };
}

/**
 * Convert terminal step
 */
function convertTerminalStep(step: TerminalStep): EngineStep {
  // Convert output fields to tuple format: Vec<(String, Expr)>
  const output: Array<[string, any]> = (step.output || []).map((field) => [
    field.name,
    convertToEngineExpr(field.value),
  ]);

  // Extract message string
  let message = step.message || '';
  if (typeof message === 'object' && message !== null) {
    if ((message as any).type === 'literal') {
      message = String((message as any).value);
    } else {
      message = JSON.stringify(message);
    }
  }

  return {
    id: step.id,
    name: step.name,
    type: 'terminal',
    result: {
      code: step.code || 'UNKNOWN',
      message: message,
      output,
      data: null,
    },
  };
}

/**
 * Convert editor value to engine Expr format
 * The engine expects Expr which is a Rust enum serialized as:
 * - { "Literal": <value> }  -- NOT { "Literal": { "value": ... } }
 * - { "Field": "<path>" }   -- NOT { "Field": { "path": ... } }
 * etc.
 *
 * The Rust Expr enum:
 * - Literal(Value) serializes to { "Literal": <the_value> }
 * - Field(String) serializes to { "Field": "<the_string>" }
 */
function convertToEngineExpr(value: any): any {
  if (value === null || value === undefined) {
    return { Literal: null };
  }

  // Handle primitive types
  if (typeof value === 'string') {
    // Check if it looks like a field reference
    if (value.startsWith('$') || value.startsWith('input.') || value.startsWith('vars.')) {
      let path = value;
      if (path.startsWith('$.')) {
        path = path.slice(2); // $.order.amount -> order.amount
      } else if (path.startsWith('input.')) {
        path = path.slice(6); // input.order.amount -> order.amount
      } else if (path.startsWith('$')) {
        path = path.slice(1);
      }
      return { Field: path };
    }
    // Otherwise treat as literal string
    return { Literal: value };
  }

  if (typeof value === 'number' || typeof value === 'boolean') {
    return { Literal: value };
  }

  // Handle editor's typed value objects
  if (typeof value === 'object') {
    // Editor literal format: { type: 'literal', value: ..., valueType: ... }
    if (value.type === 'literal') {
      return { Literal: value.value };
    }

    // Editor variable format: { type: 'variable', path: '$.xxx' }
    if (value.type === 'variable' || value.type === 'field') {
      let path = value.path || value.name || '';
      if (path.startsWith('$.')) {
        path = path.slice(2); // Remove $. prefix
      } else if (path.startsWith('$')) {
        path = path.slice(1);
      }
      return { Field: path };
    }

    // Editor expression format: { type: 'expression', expression: '...' }
    if (value.type === 'expression' && value.expression) {
      // For expressions, we need to parse them - for now just wrap as literal
      return { Literal: value.expression };
    }

    // Check if it's already in Expr format (but with wrong nesting)
    if ('Literal' in value) {
      // Check if nested value also needs conversion
      const literalValue = value.Literal;
      if (literalValue && typeof literalValue === 'object' && literalValue.value !== undefined) {
        // Wrong format: { Literal: { value: x } } -> should be { Literal: x }
        return { Literal: literalValue.value };
      }
      return value;
    }

    if ('Field' in value) {
      const fieldValue = value.Field;
      if (fieldValue && typeof fieldValue === 'object' && fieldValue.path !== undefined) {
        // Wrong format: { Field: { path: x } } -> should be { Field: x }
        return { Field: fieldValue.path };
      }
      return value;
    }

    // Array
    if (Array.isArray(value)) {
      return { Literal: value };
    }

    // Otherwise treat as literal object
    return { Literal: value };
  }

  return { Literal: value };
}

/**
 * Validate Engine compatibility
 * 验证引擎兼容性
 */
export function validateEngineCompatibility(ruleset: RuleSet): string[] {
  const errors: string[] = [];

  // Check startStepId exists
  if (!ruleset.startStepId) {
    errors.push('Missing startStepId (required as entry_step in engine)');
  }

  // Check all steps have IDs
  for (let i = 0; i < ruleset.steps.length; i++) {
    const step = ruleset.steps[i];
    if (!step.id) {
      errors.push(`Step at index ${i} missing id: ${step.name || '(unnamed)'}`);
    }
  }

  // Check startStepId references an existing step
  if (ruleset.startStepId) {
    const startStepExists = ruleset.steps.some((s) => s.id === ruleset.startStepId);
    if (!startStepExists) {
      errors.push(`startStepId '${ruleset.startStepId}' does not reference an existing step`);
    }
  }

  // Check step references
  for (const step of ruleset.steps) {
    const stepIds = new Set(ruleset.steps.map((s) => s.id));

    switch (step.type) {
      case 'decision': {
        const decisionStep = step as DecisionStep;

        // Check branches
        for (const branch of decisionStep.branches || []) {
          if (branch.nextStepId && !stepIds.has(branch.nextStepId)) {
            errors.push(
              `Step '${step.id}' branch '${branch.id}' references non-existent step '${branch.nextStepId}'`
            );
          }
        }

        // Check default next
        if (decisionStep.defaultNextStepId && !stepIds.has(decisionStep.defaultNextStepId)) {
          errors.push(
            `Step '${step.id}' defaultNextStepId references non-existent step '${decisionStep.defaultNextStepId}'`
          );
        }
        break;
      }

      case 'action': {
        const actionStep = step as ActionStep;
        if (actionStep.nextStepId && !stepIds.has(actionStep.nextStepId)) {
          errors.push(
            `Step '${step.id}' nextStepId references non-existent step '${actionStep.nextStepId}'`
          );
        }
        break;
      }

      case 'terminal':
        // Terminal steps don't reference other steps
        break;

      default:
        errors.push(`Step '${step.id}' has unknown type: ${(step as any).type}`);
    }
  }

  return errors;
}

/**
 * Check if a RuleSet is engine-compatible
 * 检查 RuleSet 是否与引擎兼容
 */
export function isEngineCompatible(ruleset: RuleSet): boolean {
  return validateEngineCompatibility(ruleset).length === 0;
}
