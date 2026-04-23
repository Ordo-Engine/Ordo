<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { DialogPlugin, MessagePlugin } from 'tdesign-vue-next'
import { useRbacStore } from '@/stores/rbac'
import { StudioPageHeader } from '@/components/ui'
import { RBAC_PERMISSION_GROUPS, permissionI18nKey } from '@/constants/rbac'
import type { OrgRole } from '@/api/types'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const rbacStore = useRbacStore()

const orgId = route.params.orgId as string
const expandedRoles = ref<Set<string>>(new Set())

onMounted(() => rbacStore.fetchRoles(orgId))

function toggleExpand(roleId: string) {
  if (expandedRoles.value.has(roleId)) {
    expandedRoles.value.delete(roleId)
  } else {
    expandedRoles.value.add(roleId)
  }
}

function startCreate() {
  router.push(`/orgs/${orgId}/roles/new`)
}

function startEdit(role: OrgRole) {
  if (role.is_system) return
  router.push(`/orgs/${orgId}/roles/${role.id}/edit`)
}

function deleteRole(role: OrgRole) {
  if (role.is_system) return
  const dialog = DialogPlugin.confirm({
    header: t('rbac.deleteRole'),
    body: t('rbac.confirmDelete', { name: role.name }),
    confirmBtn: { content: t('common.delete'), theme: 'danger' },
    cancelBtn: t('common.cancel'),
    onConfirm: async () => {
      try {
        await rbacStore.deleteRole(orgId, role.id)
        MessagePlugin.success(t('rbac.deleted'))
        dialog.hide()
      } catch (e: any) {
        MessagePlugin.error(e.message)
      }
    },
  })
}

function permissionLabel(perm: string) {
  return t(`rbac.permissionLabels.${permissionI18nKey(perm)}`)
}

function groupLabel(key: string) {
  return t(`rbac.permissionGroups.${key}`)
}

function groupedPermissions(perms: string[]) {
  const permSet = new Set(perms)
  const groups: { key: string; label: string; perms: string[] }[] = []

  for (const group of RBAC_PERMISSION_GROUPS) {
    const matched = group.permissions.filter((p) => permSet.has(p))
    if (matched.length > 0) {
      groups.push({ key: group.key, label: groupLabel(group.key), perms: matched })
    }
  }

  const known = new Set(RBAC_PERMISSION_GROUPS.flatMap((g) => g.permissions))
  const other = perms.filter((p) => !known.has(p))
  if (other.length > 0) {
    groups.push({ key: '_other', label: t('common.other', 'Other'), perms: other })
  }

  return groups
}

// Summary: "组织 ×2 · 规则集 ×3 · 发布中心 ×8"
function permissionSummary(perms: string[]) {
  return groupedPermissions(perms)
    .map((g) => `${g.label} ×${g.perms.length}`)
    .join(' · ')
}
</script>

<template>
  <div class="view-page">
    <div class="content-shell">
      <StudioPageHeader :title="$t('rbac.title')" :subtitle="$t('shell.rolesSubtitle')">
        <template #actions>
          <t-button theme="primary" @click="startCreate">
            <t-icon name="add" />
            {{ $t('rbac.addRole') }}
          </t-button>
        </template>
      </StudioPageHeader>

      <div v-if="rbacStore.loading" class="list-skeleton">
        <t-skeleton
          v-for="i in 3"
          :key="i"
          theme="paragraph"
          animation="gradient"
          :row-col="[{ width: '30%' }, { width: '55%' }, { width: '80%' }]"
        />
      </div>

      <div v-else-if="rbacStore.roles.length === 0" class="state-center">
        <t-empty :title="$t('rbac.noRoles')" />
      </div>

      <div v-else class="role-list">
        <t-card
          v-for="role in rbacStore.roles"
          :key="role.id"
          :bordered="false"
          class="role-card"
        >
          <!-- Header row -->
          <div class="role-header">
            <div class="role-name-row">
              <span class="role-name">{{ role.name }}</span>
              <t-tag v-if="role.is_system" theme="warning" variant="light" size="small">
                {{ $t('rbac.system') }}
              </t-tag>
              <t-tag v-else theme="success" variant="light" size="small">
                {{ $t('rbac.custom') }}
              </t-tag>
              <span v-if="role.description" class="role-desc-inline">{{ role.description }}</span>
            </div>
            <div class="role-actions">
              <t-button v-if="!role.is_system" size="small" variant="text" @click="startEdit(role)">
                {{ $t('rbac.editRole') }}
              </t-button>
              <t-button
                v-if="!role.is_system"
                size="small"
                variant="text"
                theme="danger"
                @click="deleteRole(role)"
              >
                {{ $t('rbac.deleteRole') }}
              </t-button>
            </div>
          </div>

          <!-- Collapsed: one-line summary -->
          <div
            v-if="role.permissions.length > 0"
            class="perm-summary-row"
            @click="toggleExpand(role.id)"
          >
            <span class="perm-summary-text">{{ permissionSummary(role.permissions) }}</span>
            <t-icon
              :name="expandedRoles.has(role.id) ? 'chevron-up' : 'chevron-down'"
              class="expand-icon"
            />
          </div>
          <p v-else class="no-perms">{{ $t('rbac.noPermissions', 'No permissions') }}</p>

          <!-- Expanded: grouped tags -->
          <div v-if="expandedRoles.has(role.id) && role.permissions.length > 0" class="perm-groups">
            <div
              v-for="group in groupedPermissions(role.permissions)"
              :key="group.key"
              class="perm-group"
            >
              <span class="perm-group-label">{{ group.label }}</span>
              <div class="perm-tags">
                <t-tag
                  v-for="perm in group.perms"
                  :key="perm"
                  variant="light"
                  theme="default"
                  size="small"
                >
                  {{ permissionLabel(perm) }}
                </t-tag>
              </div>
            </div>
          </div>
        </t-card>
      </div>
    </div>
  </div>
</template>

<style scoped>
.view-page {
  padding: 28px 36px 36px;
  height: 100%;
  overflow-y: auto;
}

.content-shell {
  max-width: 1080px;
  margin: 0 auto;
}

.list-skeleton {
  display: grid;
  gap: 8px;
}

.state-center {
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 240px;
}

.role-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.role-card :deep(.t-card__body) {
  padding: 12px 16px;
}

.role-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-height: 28px;
}

.role-name-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  flex: 1;
  min-width: 0;
}

.role-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.role-desc-inline {
  font-size: 12px;
  color: var(--ordo-text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 320px;
}

.role-actions {
  display: flex;
  align-items: center;
  flex-shrink: 0;
}

/* Collapsed summary row */
.perm-summary-row {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 6px;
  cursor: pointer;
  user-select: none;
}

.perm-summary-row:hover .perm-summary-text {
  color: var(--td-brand-color);
}

.perm-summary-text {
  font-size: 12px;
  color: var(--ordo-text-secondary);
  transition: color 0.15s;
}

.expand-icon {
  font-size: 12px;
  color: var(--ordo-text-tertiary, #9ca3af);
  flex-shrink: 0;
}

/* Expanded grouped tags */
.perm-groups {
  margin-top: 10px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding-top: 8px;
  border-top: 1px solid var(--td-component-border, #e5e7eb);
}

.perm-group {
  display: flex;
  align-items: baseline;
  gap: 8px;
  flex-wrap: wrap;
}

.perm-group-label {
  flex-shrink: 0;
  font-size: 11px;
  font-weight: 600;
  color: var(--ordo-text-tertiary, #9ca3af);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  min-width: 56px;
}

.perm-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.no-perms {
  margin-top: 6px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

@media (max-width: 900px) {
  .view-page {
    padding: 16px;
  }

  .role-header {
    flex-direction: column;
    align-items: flex-start;
  }

  .role-desc-inline {
    max-width: 100%;
  }
}
</style>
