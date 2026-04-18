export interface PermissionGroup {
  key: string
  permissions: string[]
}

export const RBAC_PERMISSION_GROUPS: PermissionGroup[] = [
  { key: 'org', permissions: ['org:view', 'org:manage'] },
  { key: 'members', permissions: ['member:view', 'member:invite', 'member:remove'] },
  { key: 'roles', permissions: ['role:view', 'role:manage'] },
  { key: 'projects', permissions: ['project:view', 'project:create', 'project:manage', 'project:delete'] },
  { key: 'rulesets', permissions: ['ruleset:view', 'ruleset:edit', 'ruleset:publish'] },
  { key: 'environments', permissions: ['environment:view', 'environment:manage'] },
  { key: 'servers', permissions: ['server:view', 'server:manage'] },
  { key: 'tests', permissions: ['test:run'] },
  { key: 'deployments', permissions: ['deployment:view', 'deployment:redeploy'] },
  { key: 'canary', permissions: ['canary:manage'] },
]

export function permissionI18nKey(permission: string) {
  return permission.replace(':', '_')
}
