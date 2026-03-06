/**
 * Decision Table model types
 * 决策表模型类型定义
 */

import type { SchemaField } from './ruleset';
import { generateId } from '../utils';

/** Schema field type alias (mirrors SchemaField['type']) */
export type SchemaFieldType = SchemaField['type'];

/** Hit policy determines how matching rows are handled */
export type HitPolicy = 'first' | 'all' | 'collect';

/** Input column definition — maps a schema field to a table column */
export interface InputColumn {
  id: string;
  /** JSONPath to the input field, e.g. "$.user.level" */
  fieldPath: string;
  /** Human-readable column header */
  label: string;
  type: SchemaFieldType;
}

/** Output column definition — maps a result field to a table column */
export interface OutputColumn {
  id: string;
  /** Name of the output field, e.g. "discount" */
  fieldName: string;
  /** Human-readable column header */
  label: string;
  type: SchemaFieldType;
}

/**
 * Cell value discriminated union.
 *
 * Each cell in the decision table holds one of these value types,
 * representing how the input field should be matched (for input columns)
 * or what value to produce (for output columns).
 */
export type CellValue =
  | { type: 'exact'; value: string | number | boolean }
  | {
      type: 'range';
      min?: number;
      max?: number;
      minInclusive?: boolean;
      maxInclusive?: boolean;
    }
  | { type: 'in'; values: (string | number)[] }
  | { type: 'any' }
  | { type: 'expression'; expr: string };

/** A single row in the decision table */
export interface DecisionTableRow {
  id: string;
  /** Row evaluation order (lower = higher priority) */
  priority: number;
  /** Column ID → cell value for each input column */
  inputValues: Record<string, CellValue>;
  /** Column ID → cell value for each output column */
  outputValues: Record<string, CellValue>;
  /** Terminal result code, e.g. "APPROVED" */
  resultCode?: string;
  /** Terminal result message */
  resultMessage?: string;
}

/** Complete decision table definition */
export interface DecisionTable {
  name: string;
  hitPolicy: HitPolicy;
  inputColumns: InputColumn[];
  outputColumns: OutputColumn[];
  rows: DecisionTableRow[];
}

// ============================================================================
// Factory helpers
// ============================================================================

export const DecisionTable = {
  createRow(priority: number): DecisionTableRow {
    return {
      id: generateId('row'),
      priority,
      inputValues: {},
      outputValues: {},
    };
  },

  createInputColumn(fieldPath: string, label: string, type: SchemaFieldType): InputColumn {
    return { id: generateId('col_in'), fieldPath, label, type };
  },

  createOutputColumn(fieldName: string, label: string, type: SchemaFieldType): OutputColumn {
    return { id: generateId('col_out'), fieldName, label, type };
  },

  anyCell(): CellValue {
    return { type: 'any' };
  },

  emptyTable(name = 'Decision Table'): DecisionTable {
    return {
      name,
      hitPolicy: 'first',
      inputColumns: [],
      outputColumns: [],
      rows: [],
    };
  },
};

/** Convert a CellValue to a human-readable display string. */
export function cellValueToString(cell: CellValue): string {
  switch (cell.type) {
    case 'exact':
      return String(cell.value);
    case 'range': {
      const left =
        cell.min !== undefined
          ? `${cell.minInclusive !== false ? '[' : '('}${cell.min}`
          : '(-\u221E';
      const right =
        cell.max !== undefined
          ? `${cell.max}${cell.maxInclusive !== false ? ']' : ')'}`
          : '\u221E)';
      return `${left}, ${right}`;
    }
    case 'in':
      return cell.values.join(', ');
    case 'any':
      return '*';
    case 'expression':
      return cell.expr;
  }
}
