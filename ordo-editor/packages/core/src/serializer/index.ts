/**
 * Serialization utilities
 * 序列化工具
 */

import { RuleSet, Expr, Condition, Step } from '../model';

/** Serialization format */
export type SerializationFormat = 'json' | 'yaml';

/** Serialization options */
export interface SerializationOptions {
  /** Whether to pretty-print */
  pretty?: boolean;
  /** Indentation (for pretty-print) */
  indent?: number;
  /** Whether to include metadata */
  includeMetadata?: boolean;
}

const DEFAULT_OPTIONS: SerializationOptions = {
  pretty: true,
  indent: 2,
  includeMetadata: true,
};

/**
 * Serialize a ruleset to JSON string
 */
export function serializeRuleSet(ruleset: RuleSet, options: SerializationOptions = {}): string {
  const opts = { ...DEFAULT_OPTIONS, ...options };

  const data = opts.includeMetadata
    ? ruleset
    : {
        config: ruleset.config,
        startStepId: ruleset.startStepId,
        steps: ruleset.steps,
      };

  if (opts.pretty) {
    return JSON.stringify(data, null, opts.indent);
  }
  return JSON.stringify(data);
}

/**
 * Deserialize a ruleset from JSON string
 */
export function deserializeRuleSet(json: string): RuleSet {
  const data = JSON.parse(json);

  // Validate basic structure
  if (!data.config || typeof data.config !== 'object') {
    throw new Error('Invalid ruleset: missing config');
  }
  if (typeof data.startStepId !== 'string') {
    throw new Error('Invalid ruleset: missing startStepId');
  }
  if (!Array.isArray(data.steps)) {
    throw new Error('Invalid ruleset: missing steps array');
  }

  return {
    config: {
      name: data.config.name || '',
      version: data.config.version,
      description: data.config.description,
      tags: data.config.tags,
      inputSchema: data.config.inputSchema,
      outputSchema: data.config.outputSchema,
      enableTrace: data.config.enableTrace,
      timeout: data.config.timeout,
    },
    startStepId: data.startStepId,
    steps: data.steps.map(deserializeStep),
    metadata: data.metadata,
  };
}

/**
 * Deserialize a step from JSON
 */
function deserializeStep(data: unknown): Step {
  const step = data as Record<string, unknown>;

  if (!step.id || !step.type) {
    throw new Error('Invalid step: missing id or type');
  }

  const base = {
    id: step.id as string,
    name: (step.name as string) || '',
    description: step.description as string | undefined,
    position: step.position as { x: number; y: number } | undefined,
  };

  switch (step.type) {
    case 'decision':
      return {
        ...base,
        type: 'decision',
        branches: Array.isArray(step.branches)
          ? step.branches.map((b: unknown) => {
              const branch = b as Record<string, unknown>;
              return {
                id: branch.id as string,
                label: branch.label as string | undefined,
                condition: branch.condition as Condition,
                nextStepId: branch.nextStepId as string,
              };
            })
          : [],
        defaultNextStepId: step.defaultNextStepId as string,
      };

    case 'action':
      return {
        ...base,
        type: 'action',
        assignments: step.assignments as { name: string; value: Expr }[] | undefined,
        externalCalls: step.externalCalls as Step['type'] extends 'action'
          ? NonNullable<Extract<Step, { type: 'action' }>['externalCalls']>
          : never,
        logging: step.logging as { message: Expr; level?: string } | undefined,
        nextStepId: step.nextStepId as string,
      };

    case 'terminal':
      return {
        ...base,
        type: 'terminal',
        code: step.code as string,
        message: step.message as Expr | undefined,
        output: step.output as { name: string; value: Expr }[] | undefined,
      };

    default:
      throw new Error(`Invalid step type: ${step.type}`);
  }
}

/**
 * Clone a ruleset (deep copy via serialization)
 */
export function cloneRuleSetViaSerialization(ruleset: RuleSet): RuleSet {
  return deserializeRuleSet(serializeRuleSet(ruleset, { includeMetadata: true }));
}

/**
 * Export ruleset to downloadable format
 */
export function exportRuleSet(
  ruleset: RuleSet,
  options: SerializationOptions = {}
): { content: string; filename: string; mimeType: string } {
  const content = serializeRuleSet(ruleset, options);
  const filename = `${ruleset.config.name || 'ruleset'}-${ruleset.config.version || '1.0.0'}.json`;

  return {
    content,
    filename,
    mimeType: 'application/json',
  };
}

/**
 * Import ruleset from file content
 */
export function importRuleSet(content: string, filename?: string): RuleSet {
  try {
    const ruleset = deserializeRuleSet(content);

    // Update metadata if filename provided
    if (filename && !ruleset.config.name) {
      const nameMatch = filename.match(/^(.+?)(-\d+\.\d+\.\d+)?\.json$/);
      if (nameMatch) {
        ruleset.config.name = nameMatch[1];
      }
    }

    return ruleset;
  } catch (error) {
    throw new Error(
      `Failed to import ruleset: ${error instanceof Error ? error.message : 'Unknown error'}`
    );
  }
}

/**
 * Merge two rulesets (for partial updates)
 */
export function mergeRuleSets(base: RuleSet, patch: Partial<RuleSet>): RuleSet {
  return {
    config: {
      ...base.config,
      ...patch.config,
    },
    startStepId: patch.startStepId ?? base.startStepId,
    steps: patch.steps ?? base.steps,
    metadata: {
      ...base.metadata,
      ...patch.metadata,
      updatedAt: new Date().toISOString(),
    },
  };
}
