<script setup lang="ts">
import { computed, onMounted } from 'vue'
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

onMounted(() => rbacStore.fetchRoles(orgId))

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

  // permissions not in any defined group
  const known = new Set(RBAC_PERMISSION_GROUPS.flatMap((g) => g.permissions))
  const other = perms.filter((p) => !known.has(p))
  if (other.length > 0) {
    groups.push({ key: '_other', label: t('common.other', 'Other'), perms: other })
  }

  return groups
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
        <t-card v-for="role in rbacStore.roles" :key="role.id" :bordered="false" class="role-card">
          <div class="role-header">
            <div class="role-name-row">
              <span class="role-name">{{ role.name }}</span>
              <t-tag v-if="role.is_system" theme="warning" variant="light">{{ $t('rbac.system') }}</t-tag>
              <t-tag v-else theme="success" variant="light">{{ $t('rbac.custom') }}</t-tag>
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

          <p v-if="role.description" class="role-desc">{{ role.description }}</p>

          <div v-if="role.permissions.length > 0" class="perm-groups">
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
                  class="perm-tag"
                >
                  {{ permissionLabel(perm) }}
                </t-tag>
              </div>
            </div>
          </div>
          <p v-else class="no-perms">{{ $t('rbac.noPermissions', 'No permissions assigned') }}</p>
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
  gap: 12px;
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
  gap: 12px;
}

.role-card :deep(.t-card__body) {
  padding: 16px;
}

.role-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.role-name-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.role-name {
  font-size: 15px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.role-actions {
  display: flex;
  align-items: center;
}

.role-desc {
  margin: 8px 0 0;
  font-size: 13px;
  color: var(--ordo-text-secondary);
}

.perm-groups {
  margin-top: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
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
  letter-spacing: 0.05em;
  min-width: 60px;
}

.perm-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.perm-tag {
  font-size: 12px;
}

.no-perms {
  margin-top: 8px;
  font-size: 13px;
  color: var(--ordo-text-secondary);
}

@media (max-width: 900px) {
  .view-page {
    padding: 20px;
  }

  .role-header {
    flex-direction: column;
    align-items: flex-start;
  }
}
</style>
