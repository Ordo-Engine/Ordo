import type { RulesetHistoryEntry } from '@/api/types';

const CHECKPOINT_SOURCES = new Set(['create', 'save', 'restore', 'publish']);

export interface CheckpointVersionEntry {
  entry: RulesetHistoryEntry;
  version: string;
  subversion: number;
  display_version: string;
}

export function stripVersionSuffix(version?: string | null) {
  return (version ?? '').replace(/\(\d+\)\s*$/, '').trim();
}

export function extractRulesetVersion(snapshot?: Record<string, any> | null) {
  const version = stripVersionSuffix(snapshot?.config?.version);
  return version || 'draft';
}

export function formatRulesetVersion(version?: string | null, subversion?: number | null) {
  const baseVersion = stripVersionSuffix(version) || 'draft';
  if (!subversion || subversion < 1) return baseVersion;
  return `${baseVersion}(${subversion})`;
}

export function buildCheckpointVersionEntries(entries: RulesetHistoryEntry[]) {
  const checkpoints = entries
    .filter((entry) => CHECKPOINT_SOURCES.has(entry.source))
    .map((entry) => ({
      entry,
      version: extractRulesetVersion(entry.snapshot as Record<string, any>),
    }))
    .reverse();

  const counters = new Map<string, number>();

  const withSubversion = checkpoints.map(({ entry, version }) => {
    const nextSubversion = (counters.get(version) ?? 0) + 1;
    counters.set(version, nextSubversion);
    return {
      entry,
      version,
      subversion: nextSubversion,
      display_version: formatRulesetVersion(version, nextSubversion),
    } satisfies CheckpointVersionEntry;
  });

  return withSubversion.reverse();
}

export function getCurrentVersionDisplay(entries: RulesetHistoryEntry[], version?: string | null) {
  const baseVersion = stripVersionSuffix(version);
  if (!baseVersion) return 'draft';

  const matches = buildCheckpointVersionEntries(entries).filter(
    (item) => item.version === baseVersion
  );
  if (matches.length === 0) return baseVersion;
  return matches[0].display_version;
}
