export type SubRuleAssetScope = 'project' | 'org';

export interface SubRuleAssetOption {
  name: string;
  scope: SubRuleAssetScope;
  displayName?: string | null;
  description?: string | null;
}

export function subRuleAssetOptionKey(option: SubRuleAssetOption): string {
  return `${option.scope}:${option.name}`;
}
