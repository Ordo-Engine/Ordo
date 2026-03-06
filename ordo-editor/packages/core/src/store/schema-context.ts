import type { SchemaField } from '../model/ruleset';
import type { SchemaFieldType } from '../model/decision-table';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ResolvedField {
  /** Bare dotted path (e.g. "user.level") */
  path: string;
  /** Full JSON-path (e.g. "$.user.level") */
  fullPath: string;
  /** Leaf name (e.g. "level") */
  name: string;
  type: SchemaFieldType;
  required: boolean;
  description?: string;
  /** Parent path (e.g. "user"), undefined for top-level fields */
  parent?: string;
  /** Known enum values, if any */
  enumValues?: string[];
}

export interface OperatorInfo {
  /** Internal value (e.g. "eq") */
  value: string;
  /** Human-readable label (e.g. "equals") */
  label: string;
  /** Symbol (e.g. "==") */
  symbol: string;
}

export interface ValueHint {
  /** Suggested value */
  value: string | number | boolean;
  /** Display label */
  label: string;
}

// ---------------------------------------------------------------------------
// Operator definitions per type
// ---------------------------------------------------------------------------

const OPERATORS: Record<string, OperatorInfo> = {
  eq:         { value: 'eq',         label: 'equals',          symbol: '==' },
  ne:         { value: 'ne',         label: 'not equals',      symbol: '!=' },
  gt:         { value: 'gt',         label: 'greater than',    symbol: '>'  },
  gte:        { value: 'gte',        label: 'greater or equal', symbol: '>=' },
  lt:         { value: 'lt',         label: 'less than',       symbol: '<'  },
  lte:        { value: 'lte',        label: 'less or equal',   symbol: '<=' },
  in:         { value: 'in',         label: 'in',              symbol: 'in' },
  contains:   { value: 'contains',   label: 'contains',        symbol: 'contains' },
  startsWith: { value: 'startsWith', label: 'starts with',     symbol: 'startsWith' },
  endsWith:   { value: 'endsWith',   label: 'ends with',       symbol: 'endsWith' },
};

const TYPE_OPERATORS: Record<SchemaFieldType, string[]> = {
  string:  ['eq', 'ne', 'contains', 'startsWith', 'endsWith', 'in'],
  number:  ['eq', 'ne', 'gt', 'gte', 'lt', 'lte', 'in'],
  boolean: ['eq', 'ne'],
  array:   ['contains', 'in'],
  object:  ['eq', 'ne'],
  any:     ['eq', 'ne', 'gt', 'gte', 'lt', 'lte', 'in', 'contains', 'startsWith', 'endsWith'],
};

// ---------------------------------------------------------------------------
// SchemaContext implementation
// ---------------------------------------------------------------------------

export interface SchemaContext {
  getField(path: string): ResolvedField | undefined;
  getAllFields(): ResolvedField[];
  getFieldsOfType(type: SchemaFieldType): ResolvedField[];
  getOperatorsForField(path: string): OperatorInfo[];
  getOperatorsForType(type: SchemaFieldType): OperatorInfo[];
  getValueHintsForField(path: string): ValueHint[];
  getTopLevelGroups(): string[];
  getFieldsByParent(parent: string): ResolvedField[];
  search(query: string): ResolvedField[];
}

export function createSchemaContext(schemaFields: SchemaField[]): SchemaContext {
  const fieldMap = new Map<string, ResolvedField>();
  const byType = new Map<SchemaFieldType, ResolvedField[]>();
  const byParent = new Map<string, ResolvedField[]>();
  const topLevelGroups: string[] = [];

  function resolve(
    fields: SchemaField[],
    parentPath: string | undefined,
  ): void {
    for (const field of fields) {
      const path = parentPath ? `${parentPath}.${field.name}` : field.name;
      const resolved: ResolvedField = {
        path,
        fullPath: `$.${path}`,
        name: field.name,
        type: field.type,
        required: field.required ?? false,
        description: field.description,
        parent: parentPath,
      };

      fieldMap.set(path, resolved);

      const typeList = byType.get(field.type) ?? [];
      typeList.push(resolved);
      byType.set(field.type, typeList);

      const parentKey = parentPath ?? '__root__';
      const parentList = byParent.get(parentKey) ?? [];
      parentList.push(resolved);
      byParent.set(parentKey, parentList);

      if (!parentPath && field.type === 'object' && field.fields?.length) {
        topLevelGroups.push(field.name);
      }

      if (field.type === 'object' && field.fields) {
        resolve(field.fields, path);
      }
    }
  }

  resolve(schemaFields, undefined);

  return {
    getField(path: string): ResolvedField | undefined {
      const clean = path.startsWith('$.') ? path.slice(2) : path;
      return fieldMap.get(clean);
    },

    getAllFields(): ResolvedField[] {
      return Array.from(fieldMap.values());
    },

    getFieldsOfType(type: SchemaFieldType): ResolvedField[] {
      return byType.get(type) ?? [];
    },

    getOperatorsForField(path: string): OperatorInfo[] {
      const field = this.getField(path);
      if (!field) return Object.values(OPERATORS);
      return this.getOperatorsForType(field.type);
    },

    getOperatorsForType(type: SchemaFieldType): OperatorInfo[] {
      const keys = TYPE_OPERATORS[type] ?? TYPE_OPERATORS.any;
      return keys.map((k) => OPERATORS[k]);
    },

    getValueHintsForField(path: string): ValueHint[] {
      const field = this.getField(path);
      if (!field) return [];

      if (field.enumValues) {
        return field.enumValues.map((v) => ({ value: v, label: String(v) }));
      }
      if (field.type === 'boolean') {
        return [
          { value: true, label: 'true' },
          { value: false, label: 'false' },
        ];
      }
      return [];
    },

    getTopLevelGroups(): string[] {
      return topLevelGroups;
    },

    getFieldsByParent(parent: string): ResolvedField[] {
      return byParent.get(parent) ?? [];
    },

    search(query: string): ResolvedField[] {
      if (!query) return this.getAllFields();
      const q = query.toLowerCase();
      return this.getAllFields().filter(
        (f) =>
          f.path.toLowerCase().includes(q) ||
          f.name.toLowerCase().includes(q) ||
          (f.description && f.description.toLowerCase().includes(q)),
      );
    },
  };
}
