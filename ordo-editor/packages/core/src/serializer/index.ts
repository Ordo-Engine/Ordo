/**
 * Serialization utilities
 * 序列化工具
 */

import { RuleSet, Expr, Condition, Step } from '../model';

/** Serialization format */
export type SerializationFormat = 'json' | 'yaml';

/** File format for .ordo files */
export type OrdoFileFormat = 'json' | 'yaml' | 'binary' | 'unknown';

/** Serialization options */
export interface SerializationOptions {
  /** Whether to pretty-print */
  pretty?: boolean;
  /** Indentation (for pretty-print) */
  indent?: number;
  /** Whether to include metadata */
  includeMetadata?: boolean;
}

/** File export options */
export interface FileExportOptions extends SerializationOptions {
  /** File format */
  format?: 'json' | 'yaml';
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

/**
 * Detect file format from filename or content
 */
export function detectFileFormat(filename: string, content?: string | ArrayBuffer): OrdoFileFormat {
  const ext = filename.split('.').pop()?.toLowerCase();

  // Check by extension first
  if (ext === 'json' || filename.endsWith('.ordo.json')) {
    return 'json';
  }
  if (ext === 'yaml' || ext === 'yml' || filename.endsWith('.ordo.yaml')) {
    return 'yaml';
  }

  // For .ordo files, check content
  if (ext === 'ordo' && content) {
    // Check if it's binary (starts with "ORDO" magic number)
    if (content instanceof ArrayBuffer) {
      const view = new Uint8Array(content);
      if (view[0] === 0x4f && view[1] === 0x52 && view[2] === 0x44 && view[3] === 0x4f) {
        return 'binary';
      }
    } else if (typeof content === 'string') {
      const trimmed = content.trim();
      if (trimmed.startsWith('{')) {
        return 'json';
      }
      // YAML typically starts with key: or ---
      if (trimmed.startsWith('---') || /^[a-zA-Z_][a-zA-Z0-9_]*:/.test(trimmed)) {
        return 'yaml';
      }
    }
  }

  return 'unknown';
}

/**
 * Parse YAML content to object (simple parser for basic YAML)
 * Note: For full YAML support, consider using js-yaml library
 */
function parseSimpleYaml(content: string): unknown {
  // This is a simplified YAML parser for basic structures
  // For production, use a proper YAML library like js-yaml
  try {
    // Try JSON first (YAML is a superset of JSON)
    return JSON.parse(content);
  } catch {
    // Basic YAML parsing - convert to JSON-like structure
    throw new Error(
      'YAML parsing requires js-yaml library. Please use JSON format or install js-yaml.'
    );
  }
}

/**
 * Import ruleset from file (supports JSON, YAML, and detects format)
 */
export function importRuleSetFromFile(
  content: string | ArrayBuffer,
  filename: string
): { ruleset: RuleSet; format: OrdoFileFormat } {
  const format = detectFileFormat(filename, content);

  if (format === 'binary') {
    throw new Error(
      'Binary .ordo files cannot be imported in the browser. Use the compiled executor on the server.'
    );
  }

  if (content instanceof ArrayBuffer) {
    content = new TextDecoder().decode(content);
  }

  let ruleset: RuleSet;

  if (format === 'yaml') {
    const data = parseSimpleYaml(content);
    ruleset = deserializeRuleSet(JSON.stringify(data));
  } else {
    ruleset = deserializeRuleSet(content);
  }

  // Update name from filename if not set
  if (!ruleset.config.name && filename) {
    const nameMatch = filename.match(/^(.+?)(?:\.ordo)?(?:\.(json|yaml|yml))?$/i);
    if (nameMatch) {
      ruleset.config.name = nameMatch[1];
    }
  }

  return { ruleset, format };
}

/**
 * Export ruleset to file with specified format
 */
export function exportRuleSetToFile(
  ruleset: RuleSet,
  options: FileExportOptions = {}
): { content: string; filename: string; mimeType: string } {
  const format = options.format || 'json';
  const content = serializeRuleSet(ruleset, options);
  const baseName = ruleset.config.name || 'ruleset';
  const version = ruleset.config.version || '1.0.0';

  if (format === 'yaml') {
    // For YAML, we'd need a proper YAML serializer
    // For now, output JSON with .yaml extension as a placeholder
    return {
      content,
      filename: `${baseName}-${version}.ordo.yaml`,
      mimeType: 'application/x-yaml',
    };
  }

  return {
    content,
    filename: `${baseName}-${version}.ordo.json`,
    mimeType: 'application/json',
  };
}

/**
 * Download file in browser
 */
export function downloadFile(content: string | Blob, filename: string, mimeType: string): void {
  const blob = content instanceof Blob ? content : new Blob([content], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(url);
}

/**
 * Read file from input element
 */
export function readFileAsText(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(new Error('Failed to read file'));
    reader.readAsText(file);
  });
}

/**
 * Read file as ArrayBuffer (for binary files)
 */
export function readFileAsArrayBuffer(file: File): Promise<ArrayBuffer> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as ArrayBuffer);
    reader.onerror = () => reject(new Error('Failed to read file'));
    reader.readAsArrayBuffer(file);
  });
}

/**
 * Download binary file (Uint8Array)
 */
export function downloadBinaryFile(data: Uint8Array, filename: string): void {
  const blob = new Blob([data], { type: 'application/octet-stream' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(url);
}
