<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { MessagePlugin } from 'tdesign-vue-next'
import { StudioPageHeader } from '@/components/ui'
import { RBAC_PERMISSION_GROUPS, permissionI18nKey } from '@/constants/rbac'
import { useRbacStore } from '@/stores/rbac'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const rbacStore = useRbacStore()

const orgId = computed(() => route.params.orgId as string)
const roleId = computed(() => route.params.roleId as string | undefined)
const isEdit = computed(() => Boolean(roleId.value))

const loading = ref(false)
const saving = ref(false)
const formName = ref('')
const formDesc = ref('')
const formPerms = ref<string[]>([])
const permissionFilter = ref<'all' | 'selected' | 'unselected'>('all')

const editingRole = computed(() =>
  roleId.value ? rbacStore.roles.find((role) => role.id === roleId.value) ?? null : null,
)

const pageTitle = computed(() => (isEdit.value ? t('rbac.editRole') : t('rbac.addRole')))
const pageSubtitle = computed(() => (isEdit.value ? t('rbac.editSubtitle') : t('rbac.createSubtitle')))
const allPermissions = computed(() =>
  RBAC_PERMISSION_GROUPS.flatMap((group) => group.permissions),
)
const visibleGroups = computed(() =>
  RBAC_PERMISSION_GROUPS.map((group) => ({
    ...group,
    visiblePermissions: group.permissions.filter((perm) => {
      if (permissionFilter.value === 'selected') return formPerms.value.includes(perm)
      if (permissionFilter.value === 'unselected') return !formPerms.value.includes(perm)
      return true
    }),
  })).filter((group) => group.visiblePermissions.length > 0),
)

onMounted(async () => {
  loading.value = true
  try {
    await rbacStore.fetchRoles(orgId.value)
    if (editingRole.value) {
      if (editingRole.value.is_system) {
        MessagePlugin.warning(t('rbac.systemRoleReadonly'))
        backToList()
        return
      }
      formName.value = editingRole.value.name
      formDesc.value = editingRole.value.description ?? ''
      formPerms.value = [...editingRole.value.permissions]
    }
  } finally {
    loading.value = false
  }
})

async function handleSave() {
  const trimmedName = formName.value.trim()
  if (!trimmedName) {
    MessagePlugin.warning(t('rbac.roleNameRequired'))
    return
  }

  saving.value = true
  try {
    if (isEdit.value && editingRole.value) {
      await rbacStore.updateRole(orgId.value, editingRole.value.id, {
        name: trimmedName,
        description: formDesc.value.trim() || undefined,
        permissions: formPerms.value,
      })
      MessagePlugin.success(t('rbac.updated'))
    } else {
      await rbacStore.createRole(orgId.value, {
        name: trimmedName,
        description: formDesc.value.trim() || undefined,
        permissions: formPerms.value,
      })
      MessagePlugin.success(t('rbac.created'))
    }
    backToList()
  } catch (e: any) {
    MessagePlugin.error(e.message)
  } finally {
    saving.value = false
  }
}

function selectedCountForGroup(perms: string[]) {
  return perms.filter((perm) => formPerms.value.includes(perm)).length
}

function selectAllPermissions() {
  formPerms.value = [...allPermissions.value]
}

function clearAllPermissions() {
  formPerms.value = []
}

function selectGroupPermissions(perms: string[]) {
  const merged = new Set([...formPerms.value, ...perms])
  formPerms.value = Array.from(merged)
}

function clearGroupPermissions(perms: string[]) {
  const target = new Set(perms)
  formPerms.value = formPerms.value.filter((perm) => !target.has(perm))
}

function setFilter(mode: 'all' | 'selected' | 'unselected') {
  permissionFilter.value = mode
}

function permissionLabel(perm: string) {
  return t(`rbac.permissionLabels.${permissionI18nKey(perm)}`)
}

function permissionDesc(perm: string) {
  return t(`rbac.permissionDescriptions.${permissionI18nKey(perm)}`)
}

function groupLabel(groupKey: string) {
  return t(`rbac.permissionGroups.${groupKey}`)
}

function backToList() {
  router.push(`/orgs/${orgId.value}/roles`)
}
</script>

<template>
  <div class="view-page">
    <div class="content-shell">
      <StudioPageHeader :title="pageTitle" :subtitle="pageSubtitle">
        <template #actions>
          <t-button variant="outline" @click="backToList">{{ $t('common.back') }}</t-button>
        </template>
      </StudioPageHeader>

      <div class="page-tip">
        {{ $t('rbac.permissionsHelp') }}
      </div>

      <t-card :bordered="false" class="form-card">
        <t-form v-if="!loading" label-align="top" :colon="false">
          <t-form-item :label="$t('rbac.roleName')" required>
            <t-input v-model="formName" :placeholder="$t('rbac.roleNamePlaceholder')" />
          </t-form-item>

          <t-form-item :label="$t('rbac.roleDesc')">
            <t-input v-model="formDesc" :placeholder="$t('rbac.roleDescPlaceholder')" />
          </t-form-item>

          <t-form-item :label="`${$t('rbac.permissions')} (${formPerms.length} ${$t('rbac.selected')})`">
            <div class="perm-table-shell">
              <div class="perm-table-tools">
                <t-space size="8px" break-line>
                  <t-button size="small" variant="outline" @click="selectAllPermissions">
                    {{ $t('rbac.selectAll') }}
                  </t-button>
                  <t-button size="small" variant="outline" @click="clearAllPermissions">
                    {{ $t('rbac.clearAll') }}
                  </t-button>
                  <t-button
                    size="small"
                    variant="outline"
                    :theme="permissionFilter === 'selected' ? 'primary' : 'default'"
                    @click="setFilter(permissionFilter === 'selected' ? 'all' : 'selected')"
                  >
                    {{ $t('rbac.filterSelected') }}
                  </t-button>
                  <t-button
                    size="small"
                    variant="outline"
                    :theme="permissionFilter === 'unselected' ? 'primary' : 'default'"
                    @click="setFilter(permissionFilter === 'unselected' ? 'all' : 'unselected')"
                  >
                    {{ $t('rbac.filterUnselected') }}
                  </t-button>
                </t-space>
              </div>
              <t-checkbox-group v-model="formPerms">
                <table class="perm-table">
                  <thead>
                    <tr>
                      <th class="col-check"></th>
                      <th class="col-name">{{ $t('rbac.permissionName') }}</th>
                      <th class="col-desc">{{ $t('rbac.permissionDesc') }}</th>
                    </tr>
                  </thead>
                  <tbody v-for="group in visibleGroups" :key="group.key">
                    <tr class="group-row">
                      <th colspan="3">
                        <div class="group-row__inner">
                          <div class="group-row__title">
                            <span>{{ groupLabel(group.key) }}</span>
                            <t-tag size="small" variant="light">
                              {{ selectedCountForGroup(group.permissions) }}/{{ group.permissions.length }}
                            </t-tag>
                          </div>
                          <div class="group-row__actions">
                            <t-button size="small" variant="text" @click="selectGroupPermissions(group.permissions)">
                              {{ $t('rbac.selectGroup') }}
                            </t-button>
                            <t-button size="small" variant="text" @click="clearGroupPermissions(group.permissions)">
                              {{ $t('rbac.clearGroup') }}
                            </t-button>
                          </div>
                        </div>
                      </th>
                    </tr>
                    <tr v-for="perm in group.visiblePermissions" :key="perm" class="perm-row">
                      <td class="col-check">
                        <t-checkbox :value="perm" />
                      </td>
                      <td class="col-name">{{ permissionLabel(perm) }}</td>
                      <td class="col-desc">{{ permissionDesc(perm) }}</td>
                    </tr>
                  </tbody>
                </table>
              </t-checkbox-group>
            </div>
          </t-form-item>

          <div class="form-actions">
            <t-button variant="outline" @click="backToList">{{ $t('common.cancel') }}</t-button>
            <t-button theme="primary" :loading="saving" @click="handleSave">{{ $t('rbac.save') }}</t-button>
          </div>
        </t-form>

        <div v-else class="list-skeleton">
          <t-skeleton theme="paragraph" animation="gradient" :row-col="[{ width: '40%' }, 1, 1, 1]" />
        </div>
      </t-card>
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
  max-width: 1100px;
  margin: 0 auto;
}

.form-card :deep(.t-card__body) {
  padding: 22px;
}

.page-tip {
  margin-bottom: 12px;
  padding: 10px 12px;
  border-radius: var(--ordo-radius-md);
  border: 1px solid var(--ordo-border-color);
  background: var(--ordo-bg-panel);
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.perm-table-shell {
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  overflow: hidden;
  background: var(--ordo-bg-panel);
}

.perm-table {
  width: 100%;
  border-collapse: collapse;
  table-layout: fixed;
}

.perm-table-tools {
  display: flex;
  justify-content: flex-end;
  padding: 10px 12px;
  border-bottom: 1px solid var(--ordo-border-light);
  background: var(--ordo-bg-panel);
}

.perm-table-shell :deep(.t-checkbox-group) {
  display: block;
  width: 100%;
}

.perm-table th,
.perm-table td {
  border-bottom: 1px solid var(--ordo-border-light);
  padding: 10px 12px;
  vertical-align: middle;
}

.perm-table thead th {
  background: var(--ordo-bg-secondary);
  color: var(--ordo-text-secondary);
  font-size: 12px;
  font-weight: 700;
  text-align: left;
}

.group-row th {
  padding: 8px 12px;
  background: var(--ordo-bg-secondary);
  border-top: 1px solid var(--ordo-border-color);
}

.group-row__inner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  font-size: 12px;
  font-weight: 700;
  color: var(--ordo-text-primary);
}

.group-row__title {
  display: flex;
  align-items: center;
  gap: 8px;
}

.group-row__actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.perm-row:hover {
  background: var(--ordo-hover-bg);
}

.col-check {
  width: 48px;
  text-align: center;
}

.col-name {
  width: 260px;
  font-size: 13px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.col-desc {
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.form-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 18px;
  padding-top: 16px;
  border-top: 1px solid var(--ordo-border-color);
}

.list-skeleton {
  display: grid;
  gap: 12px;
}

@media (max-width: 900px) {
  .view-page {
    padding: 20px;
  }

  .form-card :deep(.t-card__body) {
    padding: 16px;
  }

}
</style>
