/**
 * RuleSet - a collection of steps forming a decision flow
 * 规则集定义
 */

import { Step, getNextStepIds } from './step';
import type { StepGroup } from './group';

/** Input/Output schema field */
export interface SchemaField {
  /** Field name */
  name: string;
  /** Field type */
  type: 'string' | 'number' | 'boolean' | 'array' | 'object' | 'any';
  /** Whether the field is required */
  required?: boolean;
  /** Field description */
  description?: string;
  /** Default value */
  defaultValue?: unknown;
  /** Nested fields (for object type) */
  fields?: SchemaField[];
  /** Item type (for array type) */
  itemType?: SchemaField;
}

// ============================================================================
// JIT Schema Types - for Schema-Aware JIT Compilation
// ============================================================================

/** JIT-compatible primitive field types (matching Rust FieldType) */
export type JITPrimitiveType =
  | 'bool'
  | 'int32'
  | 'int64'
  | 'uint32'
  | 'uint64'
  | 'float32'
  | 'float64'
  | 'string'
  | 'bytes';

/** JIT field type (can be primitive, nested, repeated, or optional) */
export type JITFieldType =
  | JITPrimitiveType
  | { message: string } // Nested message type (reference by name)
  | { repeated: JITFieldType } // Array/repeated field
  | { optional: JITFieldType } // Optional field
  | { enum: string }; // Enum type (stored as i32)

/** JIT schema field definition */
export interface JITSchemaField {
  /** Field name */
  name: string;
  /** Field type */
  type: JITFieldType;
  /** Byte offset within the struct */
  offset: number;
  /** Field size in bytes (for primitive types) */
  size?: number;
  /** Protobuf tag number (if from protobuf) */
  protoTag?: number;
  /** Whether this field is required */
  required?: boolean;
  /** Field description */
  description?: string;
}

/** Complete JIT schema for a context type */
export interface JITSchema {
  /** Schema name (e.g., "LoanContext") */
  name: string;
  /** Schema version */
  version?: string;
  /** All fields in the schema */
  fields: JITSchemaField[];
  /** Total size of the struct in bytes (for #[repr(C)] layout) */
  totalSize: number;
  /** Source of the schema */
  source?: 'manual' | 'protobuf' | 'inferred';
  /** Original protobuf package name (if from protobuf) */
  protoPackage?: string;
}

/** JIT expression analysis result */
export interface JITExprAnalysis {
  /** Whether the expression is JIT-compatible */
  jitCompatible: boolean;
  /** Reason for incompatibility (if not compatible) */
  reason?: string;
  /** List of fields accessed by the expression */
  accessedFields: string[];
  /** Unsupported features found in the expression */
  unsupportedFeatures: string[];
  /** Supported features used in the expression */
  supportedFeatures: string[];
}

/** Required field info for JIT compilation */
export interface RequiredFieldInfo {
  /** Field path (e.g., "user.age") */
  path: string;
  /** Inferred type from usage */
  inferredType: string;
  /** Steps that access this field */
  usedInSteps: string[];
}

/** JIT expression entry in ruleset analysis */
export interface JITExpressionEntry {
  /** Step ID containing this expression */
  stepId: string;
  /** Step name */
  stepName: string;
  /** Type of expression location (condition, assignment, etc.) */
  location: string;
  /** The expression string */
  expression: string;
  /** Analysis result */
  analysis: JITExprAnalysis;
}

/** Complete JIT analysis result for a ruleset */
export interface JITRulesetAnalysis {
  /** Overall JIT compatibility (all expressions must be compatible) */
  overallCompatible: boolean;
  /** Number of JIT-compatible expressions */
  compatibleCount: number;
  /** Number of incompatible expressions */
  incompatibleCount: number;
  /** Total number of expressions analyzed */
  totalExpressions: number;
  /** Analysis of individual expressions */
  expressions: JITExpressionEntry[];
  /** Estimated performance improvement (1.0 = no improvement) */
  estimatedSpeedup: number;
  /** Summary of required schema fields */
  requiredFields: RequiredFieldInfo[];
}

// ============================================================================
// JIT Schema Utilities
// ============================================================================

/** Get the size in bytes for a primitive JIT type */
export function getJITPrimitiveSize(type: JITPrimitiveType): number {
  switch (type) {
    case 'bool':
      return 1;
    case 'int32':
    case 'uint32':
    case 'float32':
      return 4;
    case 'int64':
    case 'uint64':
    case 'float64':
      return 8;
    case 'string':
    case 'bytes':
      return 24; // Rust String/Vec size (ptr + len + cap)
    default:
      return 0;
  }
}

/** Check if a JIT field type is numeric (JIT-compatible) */
export function isJITNumericType(type: JITFieldType): boolean {
  if (typeof type === 'string') {
    return ['bool', 'int32', 'int64', 'uint32', 'uint64', 'float32', 'float64'].includes(type);
  }
  if (typeof type === 'object' && 'enum' in type) {
    return true; // Enums are stored as i32
  }
  return false;
}

/** Convert a simple SchemaField to JITSchemaField (basic inference) */
export function schemaFieldToJIT(field: SchemaField, offset: number): JITSchemaField {
  let jitType: JITFieldType;

  switch (field.type) {
    case 'number':
      jitType = 'float64';
      break;
    case 'boolean':
      jitType = 'bool';
      break;
    case 'string':
      jitType = 'string';
      break;
    case 'object':
      jitType = { message: field.name };
      break;
    case 'array':
      jitType = { repeated: field.itemType ? schemaFieldToJIT(field.itemType, 0).type : 'float64' };
      break;
    default:
      jitType = 'float64';
  }

  return {
    name: field.name,
    type: jitType,
    offset,
    size:
      typeof jitType === 'string' ? getJITPrimitiveSize(jitType as JITPrimitiveType) : undefined,
    required: field.required,
    description: field.description,
  };
}

/** Create an empty JIT schema */
export function createEmptyJITSchema(name: string): JITSchema {
  return {
    name,
    version: '1.0.0',
    fields: [],
    totalSize: 0,
    source: 'manual',
  };
}

/** Calculate total size and offsets for a JIT schema (C-compatible layout) */
export function calculateJITSchemaLayout(schema: JITSchema): JITSchema {
  let offset = 0;
  const fields: JITSchemaField[] = [];

  for (const field of schema.fields) {
    const size =
      field.size ??
      (typeof field.type === 'string' ? getJITPrimitiveSize(field.type as JITPrimitiveType) : 8); // Default to 8 for complex types

    // Align to field size (simple alignment rule)
    const alignment = Math.min(size, 8);
    offset = Math.ceil(offset / alignment) * alignment;

    fields.push({
      ...field,
      offset,
      size,
    });

    offset += size;
  }

  return {
    ...schema,
    fields,
    totalSize: Math.ceil(offset / 8) * 8, // Align total size to 8 bytes
  };
}

/** RuleSet configuration */
export interface RuleSetConfig {
  /** Unique name */
  name: string;
  /** Version */
  version?: string;
  /** Description */
  description?: string;
  /** Tags for categorization */
  tags?: string[];
  /** Input schema */
  inputSchema?: SchemaField[];
  /** Output schema */
  outputSchema?: SchemaField[];
  /** Whether to enable execution tracing */
  enableTrace?: boolean;
  /** Timeout in milliseconds */
  timeout?: number;
}

/** RuleSet - the main rule definition */
export interface RuleSet {
  /** Configuration */
  config: RuleSetConfig;
  /** Starting step ID */
  startStepId: string;
  /** All steps in the ruleset */
  steps: Step[];
  /** Step groups for visual organization */
  groups?: StepGroup[];
  /** Metadata */
  metadata?: {
    createdAt?: string;
    updatedAt?: string;
    createdBy?: string;
    updatedBy?: string;
  };
}

// ============================================================================
// RuleSet builder helpers
// ============================================================================

export const RuleSet = {
  /** Create a new ruleset */
  create(options: {
    name: string;
    version?: string;
    description?: string;
    tags?: string[];
    inputSchema?: SchemaField[];
    outputSchema?: SchemaField[];
    enableTrace?: boolean;
    timeout?: number;
    startStepId: string;
    steps: Step[];
  }): RuleSet {
    return {
      config: {
        name: options.name,
        version: options.version || '1.0.0',
        description: options.description,
        tags: options.tags,
        inputSchema: options.inputSchema,
        outputSchema: options.outputSchema,
        enableTrace: options.enableTrace,
        timeout: options.timeout,
      },
      startStepId: options.startStepId,
      steps: options.steps,
      metadata: {
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      },
    };
  },

  /** Create an empty ruleset */
  empty(name: string): RuleSet {
    return {
      config: {
        name,
        version: '1.0.0',
      },
      startStepId: '',
      steps: [],
      metadata: {
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      },
    };
  },
};

// ============================================================================
// RuleSet operations
// ============================================================================

/** Get a step by ID */
export function getStepById(ruleset: RuleSet, stepId: string): Step | undefined {
  return ruleset.steps.find((s) => s.id === stepId);
}

/** Get all step IDs */
export function getAllStepIds(ruleset: RuleSet): string[] {
  return ruleset.steps.map((s) => s.id);
}

/** Get the start step */
export function getStartStep(ruleset: RuleSet): Step | undefined {
  return getStepById(ruleset, ruleset.startStepId);
}

/** Get steps that have no incoming edges (orphans) */
export function getOrphanSteps(ruleset: RuleSet): Step[] {
  const referencedIds = new Set<string>([ruleset.startStepId]);

  for (const step of ruleset.steps) {
    for (const nextId of getNextStepIds(step)) {
      referencedIds.add(nextId);
    }
  }

  return ruleset.steps.filter((s) => !referencedIds.has(s.id));
}

/** Get steps that reference non-existent steps */
export function getBrokenReferences(
  ruleset: RuleSet
): Array<{ stepId: string; missingId: string }> {
  const stepIds = new Set(getAllStepIds(ruleset));
  const broken: Array<{ stepId: string; missingId: string }> = [];

  for (const step of ruleset.steps) {
    for (const nextId of getNextStepIds(step)) {
      if (nextId && !stepIds.has(nextId)) {
        broken.push({ stepId: step.id, missingId: nextId });
      }
    }
  }

  return broken;
}

/** Get all terminal steps */
export function getTerminalSteps(ruleset: RuleSet): Step[] {
  return ruleset.steps.filter((s) => s.type === 'terminal');
}

/** Build a step map for quick lookup */
export function buildStepMap(ruleset: RuleSet): Map<string, Step> {
  return new Map(ruleset.steps.map((s) => [s.id, s]));
}

/** Clone a ruleset (deep copy) */
export function cloneRuleSet(ruleset: RuleSet): RuleSet {
  return JSON.parse(JSON.stringify(ruleset));
}

/** Update ruleset metadata */
export function touchRuleSet(ruleset: RuleSet, updatedBy?: string): RuleSet {
  return {
    ...ruleset,
    metadata: {
      ...ruleset.metadata,
      updatedAt: new Date().toISOString(),
      updatedBy,
    },
  };
}

/** Add a step to the ruleset */
export function addStep(ruleset: RuleSet, step: Step): RuleSet {
  return {
    ...ruleset,
    steps: [...ruleset.steps, step],
    metadata: {
      ...ruleset.metadata,
      updatedAt: new Date().toISOString(),
    },
  };
}

/** Remove a step from the ruleset */
export function removeStep(ruleset: RuleSet, stepId: string): RuleSet {
  return {
    ...ruleset,
    steps: ruleset.steps.filter((s) => s.id !== stepId),
    startStepId: ruleset.startStepId === stepId ? '' : ruleset.startStepId,
    metadata: {
      ...ruleset.metadata,
      updatedAt: new Date().toISOString(),
    },
  };
}

/** Update a step in the ruleset */
export function updateStep(
  ruleset: RuleSet,
  stepId: string,
  updater: (step: Step) => Step
): RuleSet {
  return {
    ...ruleset,
    steps: ruleset.steps.map((s) => (s.id === stepId ? updater(s) : s)),
    metadata: {
      ...ruleset.metadata,
      updatedAt: new Date().toISOString(),
    },
  };
}
