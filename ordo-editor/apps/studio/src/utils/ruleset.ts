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
    return stripRuntimeGeneratedArtifacts({
      ...input,
      steps: input.steps,
      groups: Array.isArray(input.groups) ? input.groups : [],
      metadata: isRecord(input.metadata) ? input.metadata : undefined,
    });
  }

  if (isEngineRuleset(input)) {
    const normalized = convertFromEngineFormat(input as any);
    return stripRuntimeGeneratedArtifacts({
      ...normalized,
      groups: Array.isArray(normalized.groups) ? normalized.groups : [],
    });
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

function cloneRuleset<T>(value: T): T {
  return JSON.parse(JSON.stringify(value));
}

function safeRuntimeName(value: string) {
  return (
    value
      .trim()
      .replace(/[^a-zA-Z0-9_]+/g, '_')
      .replace(/^_+|_+$/g, '') || 'value'
  );
}

function runtimeRefSuffixForStep(stepId: string) {
  return `__${safeRuntimeName(stepId)}_terminal_return`;
}

function collectRuntimePatterns(steps: any[]) {
  const exactStepIds = new Set<string>();
  const terminalPrefixes: string[] = [];
  const subRuleSuffixes: string[] = [];

  for (const step of steps) {
    if (!step || step.type !== 'sub_rule' || typeof step.id !== 'string') continue;
    exactStepIds.add(`${step.id}__return_dispatch`);
    exactStepIds.add(`${step.id}__return_to_parent`);
    terminalPrefixes.push(`${step.id}__terminal_`);
    subRuleSuffixes.push(runtimeRefSuffixForStep(step.id));
  }

  return { exactStepIds, terminalPrefixes, subRuleSuffixes };
}

function isRuntimeGeneratedStep(
  step: any,
  patterns: ReturnType<typeof collectRuntimePatterns>
): boolean {
  if (!step || typeof step !== 'object') return false;
  if (step.systemGenerated === 'sub_rule_runtime') return true;
  const id = typeof step.id === 'string' ? step.id : '';
  return (
    patterns.exactStepIds.has(id) ||
    patterns.terminalPrefixes.some((prefix) => id.startsWith(prefix))
  );
}

function restoreSubRuleStep(step: any): any {
  if (!step || step.type !== 'sub_rule' || typeof step.refName !== 'string') return step;

  const suffix = runtimeRefSuffixForStep(String(step.id ?? ''));
  if (!step.refName.endsWith(suffix)) return step;

  return {
    ...step,
    refName: step.refName.slice(0, -suffix.length),
    returnPolicy: 'propagate_terminal',
    nextStepId: '',
    outputs: undefined,
  };
}

export function stripRuntimeGeneratedArtifacts(ruleset: RuleSet): RuleSet {
  const cloned = cloneRuleset(ruleset);
  const patterns = collectRuntimePatterns(cloned.steps ?? []);

  cloned.steps = (cloned.steps ?? [])
    .filter((step: any) => !isRuntimeGeneratedStep(step, patterns))
    .map(restoreSubRuleStep);

  if (cloned.subRules) {
    cloned.subRules = Object.fromEntries(
      Object.entries(cloned.subRules)
        .filter(([name]) => !patterns.subRuleSuffixes.some((suffix) => name.endsWith(suffix)))
        .map(([name, graph]: [string, any]) => [
          name,
          stripRuntimeGeneratedArtifactsFromGraph(graph),
        ])
    );
  }

  return cloned;
}

function stripRuntimeGeneratedArtifactsFromGraph(graph: any) {
  const graphPatterns = collectRuntimePatterns(graph.steps ?? []);
  return {
    ...graph,
    steps: (graph.steps ?? [])
      .filter((step: any) => !isRuntimeGeneratedStep(step, graphPatterns))
      .map(restoreSubRuleStep),
  };
}
