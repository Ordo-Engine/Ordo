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
export function getBrokenReferences(ruleset: RuleSet): Array<{ stepId: string; missingId: string }> {
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
export function updateStep(ruleset: RuleSet, stepId: string, updater: (step: Step) => Step): RuleSet {
  return {
    ...ruleset,
    steps: ruleset.steps.map((s) => (s.id === stepId ? updater(s) : s)),
    metadata: {
      ...ruleset.metadata,
      updatedAt: new Date().toISOString(),
    },
  };
}

