/**
 * Rule Document types — unified document model for Ordo rule definitions.
 *
 * Two independent document types:
 * - FlowDocument (type: 'flow')   — ordered step graph with execution paths
 * - DecisionTableDocument (type: 'decision-table') — flat decision table, no execution order
 *
 * Both compile to the same RuleSet (steps) for the Ordo engine to execute.
 * A flow can embed a DecisionTable inside a Decision step node (future).
 */

import type { RuleSet, RuleSetConfig } from './ruleset';
import type { Step } from './step';
import type { StepGroup } from './group';
import type { DecisionTable } from './decision-table';
import { compileTableToSteps } from './decision-table-compiler';

// ============================================================================
// Document type discriminator
// ============================================================================

export type DocumentType = 'flow' | 'decision-table';

// ============================================================================
// Flow Document — ordered step graph (current RuleSet + type tag)
// ============================================================================

export interface FlowDocument {
  type: 'flow';
  config: RuleSetConfig;
  startStepId: string;
  steps: Step[];
  groups?: StepGroup[];
  metadata?: {
    createdAt?: string;
    updatedAt?: string;
    createdBy?: string;
    updatedBy?: string;
  };
}

// ============================================================================
// Decision Table Document — standalone decision table
// ============================================================================

export interface DecisionTableDocument {
  type: 'decision-table';
  config: RuleSetConfig;
  table: DecisionTable;
  metadata?: {
    createdAt?: string;
    updatedAt?: string;
    createdBy?: string;
    updatedBy?: string;
  };
}

// ============================================================================
// Union type
// ============================================================================

export type RuleDocument = FlowDocument | DecisionTableDocument;

// ============================================================================
// Type guards
// ============================================================================

export function isFlowDocument(doc: RuleDocument): doc is FlowDocument {
  return doc.type === 'flow';
}

export function isDecisionTableDocument(doc: RuleDocument): doc is DecisionTableDocument {
  return doc.type === 'decision-table';
}

// ============================================================================
// Conversion helpers
// ============================================================================

/** Convert a FlowDocument to a RuleSet (trivial — same shape minus `type`). */
export function flowDocumentToRuleSet(doc: FlowDocument): RuleSet {
  return {
    config: doc.config,
    startStepId: doc.startStepId,
    steps: doc.steps,
    groups: doc.groups,
    metadata: doc.metadata,
  };
}

/** Wrap a RuleSet as a FlowDocument. */
export function ruleSetToFlowDocument(rs: RuleSet): FlowDocument {
  return {
    type: 'flow',
    config: rs.config,
    startStepId: rs.startStepId,
    steps: rs.steps,
    groups: rs.groups,
    metadata: rs.metadata,
  };
}

/** Compile a DecisionTableDocument into a RuleSet for engine execution. */
export function tableDocumentToRuleSet(doc: DecisionTableDocument): RuleSet {
  const { steps, startStepId } = compileTableToSteps(doc.table);
  return {
    config: doc.config,
    startStepId,
    steps,
    metadata: doc.metadata,
  };
}

/** Convert any RuleDocument to a RuleSet. */
export function documentToRuleSet(doc: RuleDocument): RuleSet {
  if (isFlowDocument(doc)) return flowDocumentToRuleSet(doc);
  return tableDocumentToRuleSet(doc);
}

/** Create an empty FlowDocument. */
export function createEmptyFlowDocument(name = 'New Rule'): FlowDocument {
  return {
    type: 'flow',
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
}

/** Create an empty DecisionTableDocument. */
export function createEmptyTableDocument(name = 'Decision Table'): DecisionTableDocument {
  return {
    type: 'decision-table',
    config: {
      name,
      version: '1.0.0',
    },
    table: {
      name,
      hitPolicy: 'first',
      inputColumns: [],
      outputColumns: [],
      rows: [],
    },
    metadata: {
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    },
  };
}

/** Detect document type from a parsed JSON object. */
export function detectDocumentType(data: Record<string, unknown>): DocumentType {
  if (data.type === 'decision-table') return 'decision-table';
  // Default: if it has steps array or no type field, treat as flow
  return 'flow';
}
