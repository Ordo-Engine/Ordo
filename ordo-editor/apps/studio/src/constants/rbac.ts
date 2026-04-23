export interface PermissionGroup {
  key: string;
  permissions: string[];
}

export const RBAC_PERMISSION_GROUPS: PermissionGroup[] = [
  { key: 'org', permissions: ['org:view', 'org:manage'] },
  { key: 'members', permissions: ['member:view', 'member:invite', 'member:remove'] },
  { key: 'roles', permissions: ['role:view', 'role:manage'] },
  {
    key: 'projects',
    permissions: ['project:view', 'project:create', 'project:manage', 'project:delete'],
  },
  { key: 'rulesets', permissions: ['ruleset:view', 'ruleset:edit', 'ruleset:publish'] },
  { key: 'environments', permissions: ['environment:view', 'environment:manage'] },
  { key: 'servers', permissions: ['server:view', 'server:manage'] },
  { key: 'tests', permissions: ['test:run'] },
  { key: 'deployments', permissions: ['deployment:view', 'deployment:redeploy'] },
  { key: 'canary', permissions: ['canary:manage'] },
  {
    key: 'releases',
    permissions: [
      'release:policy.manage',
      'release:request.create',
      'release:request.view',
      'release:request.approve',
      'release:request.reject',
      'release:execute',
      'release:pause',
      'release:resume',
      'release:rollback',
      'release:instance.view',
    ],
  },
];

export function permissionI18nKey(permission: string) {
  return permission.replace(':', '_').replace(/\./g, '_');
}

export function permissionGroup(permission: string): string | undefined {
  return RBAC_PERMISSION_GROUPS.find((g) => g.permissions.includes(permission))?.key;
}
