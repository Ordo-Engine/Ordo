/**
 * Reverse format adapter: Engine format → Editor format
 *
 * Converts the Rust ordo-server JSON response back to the editor's internal format,
 * so rulesets stored on the server can be loaded and edited in the Studio.
 *
 * This is the inverse of convertToEngineFormat() in adapter.ts.
 */

import type {
  RuleSet,
  Step,
  DecisionStep,
  ActionStep,
  TerminalStep,
  SubRuleStep,
  SubRuleGraph,
  Branch,
} from '../model';
import { Expr } from '../model';

/**
 * Engine RuleSet format (matches Rust ordo-core::RuleSet serialisation)
 */
interface EngineRuleSet {
  config: {
    name: string;
    version: string;
    description: string;
    entry_step: string;
    field_missing?: 'lenient' | 'strict' | 'default';
    max_depth?: number;
    timeout_ms?: number;
    enable_trace?: boolean;
    metadata?: Record<string, string>;
  };
  steps: Record<string, EngineStep>;
  sub_rules?: Record<string, EngineSubRuleGraph>;
}

interface EngineSubRuleGraph {
  entry_step: string;
  steps: Record<string, EngineStep>;
}

interface EngineStep {
  id: string;
  name: string;
  type: 'decision' | 'action' | 'terminal' | 'sub_rule';
  // Decision
  branches?: EngineBranch[];
  default_next?: string | null;
  // Action
  actions?: EngineAction[];
  next_step?: string;
  // Terminal
  result?: EngineTerminalResult;
  // SubRule
  ref_name?: string;
  bindings?: Array<[string, any]>;
  outputs?: Array<[string, string]>;
}

interface EngineBranch {
  condition: string | any; // Server returns parsed AST, not a raw string
  next_step: string;
  actions?: EngineAction[];
}

interface EngineAction {
  action: 'set_variable' | 'log' | 'metric' | 'external_call';
  name?: string;
  value?: any;
  message?: string;
  level?: string;
  service?: string;
  method?: string;
  params?: Array<[string, any]>;
  result_variable?: string;
  timeout_ms?: number;
  description?: string;
}

interface EngineTerminalResult {
  code: string;
  message: string;
  output: Array<[string, any]>;
  data?: any;
}

/**
 * Convert Engine RuleSet → Editor RuleSet
 */
export function convertFromEngineFormat(engine: EngineRuleSet): RuleSet {
  const steps: Step[] = Object.values(engine.steps).map(convertEngineStep);

  const subRules: Record<string, SubRuleGraph> = {};
  if (engine.sub_rules) {
    for (const [name, graph] of Object.entries(engine.sub_rules)) {
      subRules[name] = {
        entryStep: graph.entry_step,
        steps: Object.values(graph.steps).map(convertEngineStep),
      };
    }
  }

  return {
    config: {
      name: engine.config.name,
      version: engine.config.version,
      description: engine.config.description,
      timeout: engine.config.timeout_ms ?? 0,
      enableTrace: engine.config.enable_trace ?? true,
      metadata: engine.config.metadata ?? {},
    },
    startStepId: engine.config.entry_step,
    steps,
    ...(Object.keys(subRules).length > 0 && { subRules }),
  };
}

function convertEngineStep(step: EngineStep): Step {
  switch (step.type) {
    case 'decision':
      return convertEngineDecisionStep(step);
    case 'action':
      return convertEngineActionStep(step);
    case 'terminal':
      return convertEngineTerminalStep(step);
    case 'sub_rule':
      return convertEngineSubRuleStep(step);
    default:
      // Unknown type — treat as terminal with error
      return {
        id: step.id,
        name: step.name,
        type: 'terminal',
        code: 'ERROR',
        message: Expr.string(`Unknown step type: ${(step as any).type}`),
        output: [],
      } as TerminalStep;
  }
}

function convertEngineDecisionStep(step: EngineStep): DecisionStep {
  return {
    id: step.id,
    name: step.name,
    type: 'decision',
    branches: (step.branches ?? []).map(convertEngineBranch),
    defaultNextStepId: step.default_next ?? '',
  };
}

function convertEngineBranch(branch: EngineBranch): Branch {
  // The engine returns conditions as parsed AST objects, not raw strings.
  // Convert AST → string first, then parse back to editor condition format.
  const condStr =
    typeof branch.condition === 'string'
      ? branch.condition
      : engineExprAstToString(branch.condition);
  return {
    id: generateId(),
    condition: parseConditionString(condStr),
    nextStepId: branch.next_step,
    label: condStr,
  };
}

/**
 * Convert engine Expr AST → expression string.
 *
 * The server stores conditions as parsed AST (Rust enum serialised to JSON):
 *   { "Binary": { "op": "Ge", "left": {"Field": "user.age"}, "right": {"Literal": 18} } }
 *   { "Field": "user.age" }
 *   { "Literal": 42 }
 *   { "And": [expr1, expr2] }
 *   { "Or":  [expr1, expr2] }
 *   { "Not": expr }
 */
function engineExprAstToString(expr: any): string {
  if (expr === null || expr === undefined) return 'true';
  if (typeof expr === 'string') return expr;
  if (typeof expr === 'number' || typeof expr === 'boolean') return String(expr);

  if ('Field' in expr) return String(expr.Field);
  if ('Literal' in expr) {
    const v = expr.Literal;
    if (typeof v === 'string') return `"${v}"`;
    if (v === null) return 'null';
    return String(v);
  }
  if ('Binary' in expr) {
    const { op, left, right } = expr.Binary;
    const opStr = engineOpToString(op);
    return `${engineExprAstToString(left)} ${opStr} ${engineExprAstToString(right)}`;
  }
  if ('And' in expr) {
    return (expr.And as any[]).map(engineExprAstToString).join(' && ');
  }
  if ('Or' in expr) {
    return (expr.Or as any[]).map(engineExprAstToString).join(' || ');
  }
  if ('Not' in expr) {
    return `!${engineExprAstToString(expr.Not)}`;
  }
  if ('Unary' in expr) {
    const { op, expr: inner } = expr.Unary;
    if (op === 'Not') return `!${engineExprAstToString(inner)}`;
    return `${op}(${engineExprAstToString(inner)})`;
  }

  // Unknown AST node — stringify as fallback
  return JSON.stringify(expr);
}

const ENGINE_OP_MAP: Record<string, string> = {
  Eq: '==',
  Ne: '!=',
  Gt: '>',
  Lt: '<',
  Ge: '>=',
  Le: '<=',
  Add: '+',
  Sub: '-',
  Mul: '*',
  Div: '/',
  Mod: '%',
  And: '&&',
  Or: '||',
};

function engineOpToString(op: string): string {
  return ENGINE_OP_MAP[op] ?? op;
}

function convertEngineActionStep(step: EngineStep): ActionStep {
  const assignments: ActionStep['assignments'] = [];
  const externalCalls: NonNullable<ActionStep['externalCalls']> = [];
  let logging: ActionStep['logging'] | undefined;

  for (const action of step.actions ?? []) {
    if (action.action === 'set_variable' && action.name !== undefined) {
      assignments.push({
        name: action.name,
        value: convertFromEngineExpr(action.value),
      });
    } else if (action.action === 'log') {
      logging = {
        message: Expr.string(action.message ?? ''),
        level: (action.level as any) ?? 'info',
      };
    } else if (action.action === 'external_call') {
      externalCalls.push(convertEngineExternalCall(action));
    }
  }

  return {
    id: step.id,
    name: step.name,
    type: 'action',
    assignments,
    externalCalls,
    logging,
    nextStepId: step.next_step ?? '',
  };
}

function convertEngineExternalCall(
  action: Extract<EngineAction, { action: 'external_call' }> | EngineAction
): NonNullable<ActionStep['externalCalls']>[number] {
  const params = Object.fromEntries(
    (action.params ?? [])
      .filter(([name]) => name !== 'url' && name !== 'json_body')
      .map(([name, expr]) => [name, convertFromEngineExpr(expr)])
  );

  if (action.service === 'network.http') {
    const urlEntry = (action.params ?? []).find(([name]) => name === 'url');
    const jsonBodyEntry = (action.params ?? []).find(([name]) => name === 'json_body');
    const url = urlEntry ? extractLiteralString(urlEntry[1]) : '';
    const method = (action.method ?? 'post').toUpperCase();
    const target = method === 'POST' ? url : `${method} ${url}`.trim();

    return {
      type: 'http',
      target,
      params:
        jsonBodyEntry && isEngineObjectExpr(jsonBodyEntry[1])
          ? Object.fromEntries(
              jsonBodyEntry[1].Object.map(([name, expr]: [string, any]) => [
                name,
                convertFromEngineExpr(expr),
              ])
            )
          : undefined,
      resultVariable: action.result_variable,
      timeout: action.timeout_ms,
    };
  }

  const target = formatCapabilityTarget(action.service, action.method);

  return {
    type: isGrpcCapability(action.service) ? 'grpc' : 'function',
    target,
    params: Object.keys(params).length > 0 ? params : undefined,
    resultVariable: action.result_variable,
    timeout: action.timeout_ms,
  };
}

function convertEngineTerminalStep(step: EngineStep): TerminalStep {
  const result = step.result;
  const output = (result?.output ?? []).map(([name, expr]) => ({
    name,
    value: convertFromEngineExpr(expr),
  }));

  return {
    id: step.id,
    name: step.name,
    type: 'terminal',
    code: result?.code ?? 'UNKNOWN',
    message: result?.message ? Expr.string(result.message) : undefined,
    output,
  };
}

function convertEngineSubRuleStep(step: EngineStep): SubRuleStep {
  return {
    id: step.id,
    name: step.name,
    type: 'sub_rule',
    refName: step.ref_name ?? '',
    bindings: (step.bindings ?? []).map(([field, expr]) => ({
      field,
      expr: convertFromEngineExpr(expr),
    })),
    outputs: (step.outputs ?? []).map(([parentVar, childVar]) => ({ parentVar, childVar })),
    nextStepId: step.next_step ?? '',
  };
}

/**
 * Convert Engine Expr (Rust enum serialised as tagged object) → Editor value object
 * Engine: { "Literal": <value> }  |  { "Field": "<path>" }
 * Editor: { type: 'literal', value: ... }  |  { type: 'variable', path: '...' }
 */
function convertFromEngineExpr(expr: any): any {
  if (expr === null || expr === undefined) {
    return { type: 'literal', value: null, valueType: 'null' };
  }

  // { "Literal": <value> }
  if (typeof expr === 'object' && 'Literal' in expr) {
    const v = expr.Literal;
    return {
      type: 'literal',
      value: v,
      valueType:
        v === null
          ? 'null'
          : typeof v === 'boolean'
            ? 'boolean'
            : typeof v === 'number'
              ? 'number'
              : 'string',
    };
  }

  // { "Field": "<path>" }
  if (typeof expr === 'object' && 'Field' in expr) {
    const path = typeof expr.Field === 'string' ? expr.Field : String(expr.Field);
    return { type: 'variable', path };
  }

  if (typeof expr === 'object' && 'Binary' in expr) {
    return {
      type: 'binary',
      op: fromEngineBinaryOp(expr.Binary.op),
      left: convertFromEngineExpr(expr.Binary.left),
      right: convertFromEngineExpr(expr.Binary.right),
    };
  }

  if (typeof expr === 'object' && 'Unary' in expr) {
    return {
      type: 'unary',
      op: fromEngineUnaryOp(expr.Unary.op),
      operand: convertFromEngineExpr(expr.Unary.operand),
    };
  }

  if (typeof expr === 'object' && 'Call' in expr) {
    return {
      type: 'function',
      name: expr.Call.name,
      args: (expr.Call.args ?? []).map(convertFromEngineExpr),
    };
  }

  if (typeof expr === 'object' && 'Conditional' in expr) {
    return {
      type: 'conditional',
      condition: convertFromEngineExpr(expr.Conditional.condition),
      thenExpr: convertFromEngineExpr(expr.Conditional.then_branch),
      elseExpr: convertFromEngineExpr(expr.Conditional.else_branch),
    };
  }

  if (typeof expr === 'object' && 'Array' in expr) {
    return {
      type: 'array',
      elements: (expr.Array ?? []).map(convertFromEngineExpr),
    };
  }

  if (typeof expr === 'object' && 'Object' in expr) {
    return {
      type: 'object',
      properties: Object.fromEntries(
        (expr.Object ?? []).map(([name, value]: [string, any]) => [
          name,
          convertFromEngineExpr(value),
        ])
      ),
    };
  }

  // Primitives (edge case: engine may return untagged scalars in some versions)
  if (typeof expr === 'string' || typeof expr === 'number' || typeof expr === 'boolean') {
    return { type: 'literal', value: expr, valueType: typeof expr };
  }

  // Fallback: wrap as literal
  return { type: 'literal', value: expr, valueType: 'object' };
}

function isEngineObjectExpr(expr: any): expr is { Object: Array<[string, any]> } {
  return typeof expr === 'object' && expr !== null && Array.isArray(expr.Object);
}

function extractLiteralString(expr: any): string {
  if (typeof expr === 'object' && expr !== null && 'Literal' in expr) {
    return String(expr.Literal ?? '');
  }
  return '';
}

function formatCapabilityTarget(service?: string, method?: string): string {
  const normalizedService = service ?? '';
  const normalizedMethod = method ?? 'invoke';

  if (!normalizedService) {
    return normalizedMethod;
  }

  if (normalizedMethod === 'invoke' || normalizedMethod === 'call') {
    return normalizedService;
  }

  return `${normalizedService}#${normalizedMethod}`;
}

function isGrpcCapability(service?: string): boolean {
  return !!service && service.includes('grpc');
}

function fromEngineBinaryOp(op: string): string {
  const operatorMap: Record<string, string> = {
    Eq: 'eq',
    Ne: 'ne',
    Gt: 'gt',
    Ge: 'gte',
    Lt: 'lt',
    Le: 'lte',
    And: 'and',
    Or: 'or',
    Add: 'add',
    Sub: 'sub',
    Mul: 'mul',
    Div: 'div',
    Mod: 'mod',
    In: 'in',
    Contains: 'contains',
  };

  return operatorMap[op] ?? op.toLowerCase();
}

function fromEngineUnaryOp(op: string): string {
  const operatorMap: Record<string, string> = {
    Not: 'not',
    Neg: 'neg',
  };

  return operatorMap[op] ?? op.toLowerCase();
}

/**
 * Parse a condition string back to an editor condition object.
 *
 * The engine stores conditions as expression strings (e.g. "user.age >= 18").
 * The editor can handle these as 'expression' type conditions — the editor's
 * ExpressionInput component can display and edit them.
 *
 * We attempt simple parsing for common patterns; complex expressions fall back
 * to the 'expression' type which the editor renders as a raw string.
 */
function parseConditionString(expr: string): any {
  if (!expr || expr === 'true') {
    return { type: 'expression', expression: expr || 'true' };
  }

  // Strip optional outer parens
  const trimmed = expr.trim();
  if (trimmed.startsWith('(') && trimmed.endsWith(')')) {
    const inner = trimmed.slice(1, -1).trim();
    // Only strip if the parens are truly wrapping the whole expression
    if (parenDepthIsZeroAfterOpen(inner)) {
      return parseConditionString(inner);
    }
  }

  // Split on top-level ' && ' / ' || ' (left-to-right, shortest match)
  const andParts = splitTopLevel(trimmed, '&&');
  if (andParts.length > 1) {
    return {
      type: 'logical',
      operator: 'and',
      conditions: andParts.map(parseConditionString),
    };
  }

  const orParts = splitTopLevel(trimmed, '||');
  if (orParts.length > 1) {
    return {
      type: 'logical',
      operator: 'or',
      conditions: orParts.map(parseConditionString),
    };
  }

  // Try to parse simple binary expressions: "left op right"
  const simpleOps = [
    { op: '>=', editorOp: 'gte' },
    { op: '<=', editorOp: 'lte' },
    { op: '!=', editorOp: 'ne' },
    { op: '==', editorOp: 'eq' },
    { op: '>', editorOp: 'gt' },
    { op: '<', editorOp: 'lt' },
  ];

  for (const { op, editorOp } of simpleOps) {
    const idx = trimmed.indexOf(op);
    if (idx === -1) continue;

    const left = trimmed.slice(0, idx).trim();
    const right = trimmed.slice(idx + op.length).trim();

    // Only parse if both sides look simple (no parens, no logical ops)
    if (left && right && !/[()&|]/.test(left) && !/[()&|]/.test(right)) {
      return {
        type: 'simple',
        left: parseValueToken(left),
        operator: editorOp,
        right: parseValueToken(right),
      };
    }
  }

  // Fall back to raw expression
  return { type: 'expression', expression: trimmed };
}

/**
 * Split an expression string on a top-level logical operator (&&  or ||),
 * respecting parentheses nesting. Returns the original single-element array
 * if the operator is not found at the top level.
 */
function splitTopLevel(expr: string, op: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let start = 0;
  const opLen = op.length;

  for (let i = 0; i < expr.length; i++) {
    const ch = expr[i];
    if (ch === '(') {
      depth++;
      continue;
    }
    if (ch === ')') {
      depth--;
      continue;
    }

    if (depth === 0 && expr.slice(i, i + opLen) === op) {
      // Ensure it's surrounded by whitespace so we don't split inside tokens
      const before = expr[i - 1];
      const after = expr[i + opLen];
      if ((!before || before === ' ') && (!after || after === ' ')) {
        parts.push(expr.slice(start, i).trim());
        start = i + opLen;
        i += opLen - 1;
      }
    }
  }

  parts.push(expr.slice(start).trim());
  return parts.length > 1 ? parts.filter(Boolean) : [expr];
}

/** Return true when `s` (the content after the opening paren was stripped)
 *  never goes negative depth — meaning the original outer parens truly wrapped everything. */
function parenDepthIsZeroAfterOpen(s: string): boolean {
  let depth = 0;
  for (const ch of s) {
    if (ch === '(') depth++;
    if (ch === ')') {
      if (depth === 0) return false;
      depth--;
    }
  }
  return depth === 0;
}

function parseValueToken(token: string): any {
  // Quoted string literal
  if (
    (token.startsWith('"') && token.endsWith('"')) ||
    (token.startsWith("'") && token.endsWith("'"))
  ) {
    return { type: 'literal', value: token.slice(1, -1), valueType: 'string' };
  }
  // Numeric literal
  if (/^-?\d+(\.\d+)?$/.test(token)) {
    return { type: 'literal', value: parseFloat(token), valueType: 'number' };
  }
  // Boolean literal
  if (token === 'true' || token === 'false') {
    return { type: 'literal', value: token === 'true', valueType: 'boolean' };
  }
  // Null
  if (token === 'null') {
    return { type: 'literal', value: null, valueType: 'null' };
  }
  // Field reference (dotted path like user.age)
  return { type: 'variable', path: token };
}

let _idCounter = 0;
function generateId(): string {
  return `b-${Date.now()}-${_idCounter++}`;
}
