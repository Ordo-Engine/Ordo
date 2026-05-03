import { convertFromEngineFormat, type RuleSet } from '@ordo-engine/editor-core';

type UnknownRecord = Record<string, unknown>;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null;
}

function isEditorRuleset(value: unknown): value is RuleSet {
  if (!isRecord(value) || !isRecord(value.config)) {
    return false;
  }
  return Array.isArray(value.steps);
}

export function isEngineRuleset(value: unknown): value is {
  config: Record<string, unknown> & { entry_step?: unknown };
  steps: Record<string, unknown>;
} {
  if (!isRecord(value) || !isRecord(value.config) || !isRecord(value.steps)) {
    return false;
  }
  return 'entry_step' in value.config && !Array.isArray(value.steps);
}

export function normalizeRuleset(input: unknown, fallbackName = 'untitled'): RuleSet {
  if (isEditorRuleset(input)) {
    return {
      ...input,
      steps: input.steps,
      groups: Array.isArray(input.groups) ? input.groups : [],
      metadata: isRecord(input.metadata) ? input.metadata : undefined,
    };
  }

  if (isEngineRuleset(input)) {
    const normalized = convertFromEngineFormat(input as any);
    return {
      ...normalized,
      groups: Array.isArray(normalized.groups) ? normalized.groups : [],
    };
  }

  return {
    config: {
      name: fallbackName,
      version: '1.0.0',
      description: '',
      metadata: {},
    },
    startStepId: '',
    steps: [],
    groups: [],
  };
}
