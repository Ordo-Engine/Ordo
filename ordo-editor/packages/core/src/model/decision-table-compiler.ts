/**
 * Bidirectional compiler between DecisionTable and Step[] representations.
 * 决策表与步骤模型之间的双向编译器
 *
 * compileTableToSteps: DecisionTable → Step[]
 * decompileStepsToTable: Step[] → DecisionTable | null
 */

import type {
  DecisionTable,
  InputColumn,
  OutputColumn,
  CellValue,
  DecisionTableRow,
  SchemaFieldType,
} from './decision-table';
import type {
  Step,
  Branch,
  DecisionStep,
  TerminalStep,
  OutputField,
  VariableAssignment,
} from './step';
import { isDecisionStep, isActionStep, isTerminalStep } from './step';
import type { Condition, SimpleCondition } from './condition';
import { conditionToString } from './condition';
import type { Expr, LiteralExpr } from './expr';
import { Expr as E, exprToString } from './expr';
import type { SchemaField } from './ruleset';
import { generateId } from '../utils';

// ============================================================================
// Table → Steps (compile)
// ============================================================================

export interface CompileResult {
  steps: Step[];
  startStepId: string;
}

/**
 * Compiles a DecisionTable into an array of Steps.
 *
 * Strategy for `hitPolicy: 'first'`:
 *   - One Decision step with branches ordered by row priority
 *   - Each branch points to a unique Terminal step
 *   - A default "no match" Terminal step handles the fallthrough
 */
export function compileTableToSteps(table: DecisionTable): CompileResult {
  if (table.hitPolicy !== 'first') {
    throw new Error(
      `Hit policy "${table.hitPolicy}" is not yet supported. Only "first" is implemented.`
    );
  }

  const decisionStepId = generateId('dt_decision');
  const noMatchId = generateId('dt_nomatch');

  const sortedRows = [...table.rows].sort((a, b) => a.priority - b.priority);

  const terminalSteps: TerminalStep[] = [];
  const branches: Branch[] = [];

  const rowCount = sortedRows.length;
  for (let i = 0; i < rowCount; i++) {
    const row = sortedRows[i];
    const terminalId = generateId('dt_terminal');

    const condition = buildRowCondition(row, table.inputColumns);
    const terminal = buildTerminalStep(terminalId, row, table.outputColumns, i, rowCount);

    terminalSteps.push(terminal);
    branches.push({
      id: generateId('dt_branch'),
      label: row.resultCode || `Row ${row.priority}`,
      condition,
      nextStepId: terminalId,
    });
  }

  const noMatchTerminal: TerminalStep = {
    id: noMatchId,
    name: 'No Match',
    type: 'terminal',
    code: 'NO_MATCH',
    message: E.string('No matching rule found'),
    output: [],
    position: { x: 400, y: 200 + rowCount * 120 },
  };
  terminalSteps.push(noMatchTerminal);

  const decisionStep: DecisionStep = {
    id: decisionStepId,
    name: table.name || 'Decision Table',
    type: 'decision',
    branches,
    defaultNextStepId: noMatchId,
    position: { x: 200, y: 80 },
  };

  return {
    steps: [decisionStep, ...terminalSteps],
    startStepId: decisionStepId,
  };
}

/** Build the combined AND condition for a single row across all input columns. */
function buildRowCondition(row: DecisionTableRow, inputColumns: InputColumn[]): Condition {
  const parts: Condition[] = [];

  for (const col of inputColumns) {
    const cell = row.inputValues[col.id];
    if (!cell || cell.type === 'any') continue;

    const cond = cellValueToCondition(E.variable(col.fieldPath), cell);
    if (cond) parts.push(cond);
  }

  if (parts.length === 0) return { type: 'constant', value: true };
  if (parts.length === 1) return parts[0];
  return { type: 'logical', operator: 'and', conditions: parts };
}

/** Convert a single CellValue into a Condition against `field`. */
function cellValueToCondition(field: Expr, cell: CellValue): Condition | null {
  switch (cell.type) {
    case 'exact':
      return { type: 'simple', left: field, operator: 'eq', right: E.literal(cell.value) };

    case 'range': {
      const rangeParts: Condition[] = [];
      if (cell.min !== undefined) {
        rangeParts.push({
          type: 'simple',
          left: field,
          operator: cell.minInclusive !== false ? 'gte' : 'gt',
          right: E.number(cell.min),
        });
      }
      if (cell.max !== undefined) {
        rangeParts.push({
          type: 'simple',
          left: field,
          operator: cell.maxInclusive !== false ? 'lte' : 'lt',
          right: E.number(cell.max),
        });
      }
      if (rangeParts.length === 0) return null;
      if (rangeParts.length === 1) return rangeParts[0];
      return { type: 'logical', operator: 'and', conditions: rangeParts };
    }

    case 'in':
      return {
        type: 'simple',
        left: field,
        operator: 'in',
        right: { type: 'array', elements: cell.values.map((v) => E.literal(v)) },
      };

    case 'any':
      return null;

    case 'expression':
      return { type: 'expression', expression: cell.expr };
  }
}

/** Build a TerminalStep from a row's output values. */
function buildTerminalStep(
  id: string,
  row: DecisionTableRow,
  outputColumns: OutputColumn[],
  rowIndex: number,
  totalRows: number
): TerminalStep {
  const output: OutputField[] = [];

  for (const col of outputColumns) {
    const cell = row.outputValues[col.id];
    if (!cell) continue;
    output.push({ name: col.fieldName, value: cellValueToExpr(cell) });
  }

  return {
    id,
    name: row.resultCode || `Row ${row.priority} Result`,
    type: 'terminal',
    code: row.resultCode || `ROW_${row.priority}`,
    message: row.resultMessage ? E.string(row.resultMessage) : undefined,
    output: output.length > 0 ? output : undefined,
    position: { x: 400 + (rowIndex % 3) * 280, y: 200 + Math.floor(rowIndex / 3) * 120 },
  };
}

/** Convert an output CellValue to an Expr for storage in a TerminalStep. */
function cellValueToExpr(cell: CellValue): Expr {
  switch (cell.type) {
    case 'exact':
      return E.literal(cell.value);
    case 'expression':
      return { type: 'variable', path: cell.expr };
    default:
      return E.null();
  }
}

// ============================================================================
// Steps → Table (decompile) — graph traversal approach
// ============================================================================

interface PathConditionEntry {
  condition: Condition;
  branchLabel?: string;
}

interface ExecutionPath {
  conditions: PathConditionEntry[];
  actions: VariableAssignment[];
  terminal: TerminalStep;
}

/**
 * DFS through the step graph, collecting conditions (from Decision branches)
 * and variable assignments (from Action steps) along each path to a Terminal.
 */
function traceExecutionPaths(
  stepMap: Map<string, Step>,
  currentStepId: string,
  conditions: PathConditionEntry[],
  actions: VariableAssignment[],
  visited: Set<string>,
  maxDepth: number
): ExecutionPath[] {
  if (maxDepth <= 0) return [];
  const step = stepMap.get(currentStepId);
  if (!step || visited.has(currentStepId)) return [];

  if (isTerminalStep(step)) {
    return [
      {
        conditions: [...conditions],
        actions: [...actions],
        terminal: step,
      },
    ];
  }

  visited.add(currentStepId);
  const results: ExecutionPath[] = [];

  if (isActionStep(step)) {
    const accumulated = [...actions, ...(step.assignments || [])];
    results.push(
      ...traceExecutionPaths(
        stepMap,
        step.nextStepId,
        conditions,
        accumulated,
        visited,
        maxDepth - 1
      )
    );
  } else if (isDecisionStep(step)) {
    for (const branch of step.branches) {
      results.push(
        ...traceExecutionPaths(
          stepMap,
          branch.nextStepId,
          [...conditions, { condition: branch.condition, branchLabel: branch.label }],
          actions,
          visited,
          maxDepth - 1
        )
      );
    }
    // Default path — no additional condition (becomes wildcard in the table)
    results.push(
      ...traceExecutionPaths(
        stepMap,
        step.defaultNextStepId,
        conditions,
        actions,
        visited,
        maxDepth - 1
      )
    );
  }

  visited.delete(currentStepId);
  return results;
}

/**
 * Decompiles a Step[] into a DecisionTable by tracing every execution path
 * through the step graph (Decision → Action → Terminal chains).
 *
 * Each path becomes one table row. Input columns are derived from branch
 * conditions, output columns from action assignments + terminal outputs.
 *
 * Returns `null` only when no valid paths exist or the graph is degenerate
 * (>200 paths).
 */
export function decompileStepsToTable(
  steps: Step[],
  startStepId: string,
  schema?: SchemaField[]
): DecisionTable | null {
  const stepMap = new Map(steps.map((s) => [s.id, s]));
  const startStep = stepMap.get(startStepId);
  if (!startStep) return null;

  const paths = traceExecutionPaths(stepMap, startStepId, [], [], new Set(), 100);
  if (paths.length === 0 || paths.length > 200) return null;

  // --- Extract input columns from all path conditions ---
  const fieldPaths = new Set<string>();
  for (const path of paths) {
    for (const entry of path.conditions) {
      collectInputFieldPaths(entry.condition, fieldPaths);
    }
  }

  const inputColumns: InputColumn[] = [...fieldPaths].map((p) => {
    const schemaField = schema ? resolveSchemaField(schema, p) : undefined;
    return {
      id: generateId('col_in'),
      fieldPath: p,
      label: schemaField?.description || pathToLabel(p),
      type: (schemaField?.type ?? 'string') as SchemaFieldType,
    };
  });

  // --- Extract output columns (action assignments first, then terminal outputs) ---
  const outputFieldNames = new Map<string, boolean>();
  for (const path of paths) {
    for (const a of path.actions) {
      if (!outputFieldNames.has(a.name)) outputFieldNames.set(a.name, true);
    }
    path.terminal.output?.forEach((f) => {
      if (!outputFieldNames.has(f.name)) outputFieldNames.set(f.name, true);
    });
  }

  const outputColumns: OutputColumn[] = [...outputFieldNames.keys()].map((name) => {
    const schemaField = schema ? resolveSchemaField(schema, name) : undefined;
    return {
      id: generateId('col_out'),
      fieldName: name,
      label: schemaField?.description || pathToLabel(name),
      type: (schemaField?.type ?? 'string') as SchemaFieldType,
    };
  });

  // --- Build rows from execution paths ---
  const rows: DecisionTableRow[] = paths.map((path, index) => {
    // Combine all conditions along this path into one AND for per-field extraction
    const combined: Condition =
      path.conditions.length === 0
        ? { type: 'constant', value: true }
        : path.conditions.length === 1
          ? path.conditions[0].condition
          : {
              type: 'logical',
              operator: 'and',
              conditions: path.conditions.map((e) => e.condition),
            };

    const inputValues: Record<string, CellValue> = {};
    for (const col of inputColumns) {
      inputValues[col.id] = extractCellValueForField(combined, col.fieldPath);
    }

    const outputValues: Record<string, CellValue> = {};
    for (const col of outputColumns) {
      const assignment = path.actions.find((a) => a.name === col.fieldName);
      if (assignment) {
        outputValues[col.id] = exprToCellValue(assignment.value);
        continue;
      }
      const field = path.terminal.output?.find((f) => f.name === col.fieldName);
      outputValues[col.id] = field ? exprToCellValue(field.value) : { type: 'any' };
    }

    return {
      id: generateId('row'),
      priority: index + 1,
      inputValues,
      outputValues,
      resultCode: path.terminal.code,
      resultMessage: extractLiteralString(path.terminal.message),
    };
  });

  return {
    name: startStep.name || 'Decision Table',
    hitPolicy: 'first',
    inputColumns,
    outputColumns,
    rows,
  };
}

// ============================================================================
// Decompile helpers
// ============================================================================

/**
 * Recursively collect variable paths from the *left* side of SimpleConditions.
 * These represent the input fields being tested.
 */
function collectInputFieldPaths(condition: Condition, out: Set<string>): void {
  switch (condition.type) {
    case 'simple':
      if (condition.left.type === 'variable') out.add(condition.left.path);
      break;
    case 'logical':
      condition.conditions.forEach((c) => collectInputFieldPaths(c, out));
      break;
    case 'not':
      collectInputFieldPaths(condition.condition, out);
      break;
  }
}

/**
 * Given a branch condition and a target field path, extract the CellValue
 * that represents the constraint on that field.
 */
function extractCellValueForField(condition: Condition, fieldPath: string): CellValue {
  if (condition.type === 'constant') return { type: 'any' };

  if (condition.type === 'simple') {
    return isVariableFor(condition.left, fieldPath)
      ? simpleConditionToCellValue(condition)
      : { type: 'any' };
  }

  if (condition.type === 'logical' && condition.operator === 'and') {
    const relevant = condition.conditions.filter((c) => conditionReferencesField(c, fieldPath));
    if (relevant.length === 0) return { type: 'any' };
    if (relevant.length === 1) return extractCellValueForField(relevant[0], fieldPath);

    // Two simple conditions on the same numeric field → try to form a range
    if (relevant.length === 2 && relevant[0].type === 'simple' && relevant[1].type === 'simple') {
      const range = tryBuildRangeCell(
        relevant[0] as SimpleCondition,
        relevant[1] as SimpleCondition,
        fieldPath
      );
      if (range) return range;
    }

    return { type: 'expression', expr: relevant.map(conditionToString).join(' && ') };
  }

  if (condition.type === 'expression') {
    return { type: 'expression', expr: condition.expression };
  }

  return { type: 'any' };
}

/** Convert a SimpleCondition into the most specific CellValue possible. */
function simpleConditionToCellValue(cond: SimpleCondition): CellValue {
  const { operator, right } = cond;

  if (operator === 'eq' && right.type === 'literal') {
    return { type: 'exact', value: right.value as string | number | boolean };
  }

  if (operator === 'in' && right.type === 'array') {
    const values = right.elements
      .filter((e): e is LiteralExpr => e.type === 'literal')
      .map((e) => e.value as string | number);
    return { type: 'in', values };
  }

  if (right.type === 'literal' && typeof right.value === 'number') {
    switch (operator) {
      case 'gt':
        return { type: 'range', min: right.value, minInclusive: false };
      case 'gte':
        return { type: 'range', min: right.value, minInclusive: true };
      case 'lt':
        return { type: 'range', max: right.value, maxInclusive: false };
      case 'lte':
        return { type: 'range', max: right.value, maxInclusive: true };
    }
  }

  return { type: 'expression', expr: conditionToString(cond) };
}

/**
 * Try to merge two simple conditions on the same field into a single range CellValue.
 * E.g. `$.age >= 18` + `$.age < 65` → `{ type: 'range', min: 18, max: 65, ... }`
 */
function tryBuildRangeCell(
  a: SimpleCondition,
  b: SimpleCondition,
  fieldPath: string
): CellValue | null {
  if (!isVariableFor(a.left, fieldPath) || !isVariableFor(b.left, fieldPath)) return null;

  let min: number | undefined;
  let max: number | undefined;
  let minInclusive = true;
  let maxInclusive = true;

  for (const cond of [a, b]) {
    if (cond.right.type !== 'literal' || typeof cond.right.value !== 'number') return null;
    const val = cond.right.value;
    switch (cond.operator) {
      case 'gt':
        min = val;
        minInclusive = false;
        break;
      case 'gte':
        min = val;
        minInclusive = true;
        break;
      case 'lt':
        max = val;
        maxInclusive = false;
        break;
      case 'lte':
        max = val;
        maxInclusive = true;
        break;
      default:
        return null;
    }
  }

  if (min === undefined && max === undefined) return null;
  return { type: 'range', min, max, minInclusive, maxInclusive };
}

/** Convert an output field Expr back to a CellValue. */
function exprToCellValue(expr: Expr): CellValue {
  if (expr.type === 'literal' && expr.value !== null) {
    return { type: 'exact', value: expr.value as string | number | boolean };
  }
  return { type: 'expression', expr: exprToString(expr) };
}

/** Extract a plain string from an Expr if it's a string literal. */
function extractLiteralString(expr: Expr | undefined): string | undefined {
  if (!expr) return undefined;
  if (expr.type === 'literal' && typeof expr.value === 'string') return expr.value;
  return exprToString(expr);
}

function isVariableFor(expr: Expr, path: string): boolean {
  return expr.type === 'variable' && expr.path === path;
}

function conditionReferencesField(condition: Condition, fieldPath: string): boolean {
  switch (condition.type) {
    case 'simple':
      return isVariableFor(condition.left, fieldPath);
    case 'logical':
      return condition.conditions.some((c) => conditionReferencesField(c, fieldPath));
    case 'not':
      return conditionReferencesField(condition.condition, fieldPath);
    default:
      return false;
  }
}

/** Resolve a dotted path like "$.user.level" against a SchemaField tree. */
function resolveSchemaField(schema: SchemaField[], path: string): SchemaField | undefined {
  const parts = path.replace(/^\$\.?/, '').split('.');
  let fields = schema;

  for (let i = 0; i < parts.length; i++) {
    const field = fields.find((f) => f.name === parts[i]);
    if (!field) return undefined;
    if (i === parts.length - 1) return field;
    if (!field.fields) return undefined;
    fields = field.fields;
  }
  return undefined;
}

/** Convert a field path into a human-readable label. "$.order.total_amount" → "Total Amount" */
function pathToLabel(path: string): string {
  const leaf =
    path
      .replace(/^\$\.?/, '')
      .split('.')
      .pop() || path;
  return leaf
    .replace(/([a-z])([A-Z])/g, '$1 $2')
    .replace(/[_-]/g, ' ')
    .replace(/\b\w/g, (c) => c.toUpperCase());
}
