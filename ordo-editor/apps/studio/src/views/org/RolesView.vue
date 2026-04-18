<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useRbacStore } from '@/stores/rbac'
import { MessagePlugin } from 'tdesign-vue-next'
import { PERMISSIONS } from '@/api/types'
import type { OrgRole } from '@/api/types'

const route = useRoute()
const { t } = useI18n()
const rbacStore = useRbacStore()

const orgId = route.params.orgId as string

const showForm = ref(false)
const editingRole = ref<OrgRole | null>(null)
const formName = ref('')
const formDesc = ref('')
const formPerms = ref<string[]>([])
const saving = ref(false)

onMounted(() => rbacStore.fetchRoles(orgId))

function startCreate() {
  editingRole.value = null
  formName.value = ''
  formDesc.value = ''
  formPerms.value = []
  showForm.value = true
}

function startEdit(role: OrgRole) {
  if (role.is_system) return
  editingRole.value = role
  formName.value = role.name
  formDesc.value = role.description ?? ''
  formPerms.value = [...role.permissions]
  showForm.value = true
}

function togglePerm(perm: string) {
  const idx = formPerms.value.indexOf(perm)
  if (idx === -1) formPerms.value.push(perm)
  else formPerms.value.splice(idx, 1)
}

async function saveForm() {
  if (!formName.value.trim()) return
  saving.value = true
  try {
    if (editingRole.value) {
      await rbacStore.updateRole(orgId, editingRole.value.id, {
        name: formName.value.trim(),
        description: formDesc.value.trim() || undefined,
        permissions: formPerms.value,
      })
      MessagePlugin.success(t('rbac.updated'))
    } else {
      await rbacStore.createRole(orgId, {
        name: formName.value.trim(),
        description: formDesc.value.trim() || undefined,
        permissions: formPerms.value,
      })
      MessagePlugin.success(t('rbac.created'))
    }
    showForm.value = false
  } catch (e: any) {
    MessagePlugin.error(e.message)
  } finally {
    saving.value = false
  }
}

async function deleteRole(role: OrgRole) {
  if (role.is_system) return
  if (!confirm(t('rbac.confirmDelete', { name: role.name }))) return
  try {
    await rbacStore.deleteRole(orgId, role.id)
    MessagePlugin.success(t('rbac.deleted'))
  } catch (e: any) {
    MessagePlugin.error(e.message)
  }
}

// Group permissions by resource for display
const permGroups = [
  { label: 'Org', perms: ['org:view', 'org:manage'] },
  { label: 'Members', perms: ['member:view', 'member:invite', 'member:remove'] },
  { label: 'Roles', perms: ['role:view', 'role:manage'] },
  { label: 'Projects', perms: ['project:view', 'project:create', 'project:manage', 'project:delete'] },
  { label: 'Rulesets', perms: ['ruleset:view', 'ruleset:edit', 'ruleset:publish'] },
  { label: 'Environments', perms: ['environment:view', 'environment:manage'] },
  { label: 'Servers', perms: ['server:view', 'server:manage'] },
  { label: 'Tests', perms: ['test:run'] },
  { label: 'Deployments', perms: ['deployment:view', 'deployment:redeploy'] },
  { label: 'Canary', perms: ['canary:manage'] },
]
</script>

<template>
  <div class="roles-view">
    <div class="page-header">
      <h2 class="page-title">{{ $t('rbac.title') }}</h2>
      <button class="btn-primary" @click="startCreate">{{ $t('rbac.addRole') }}</button>
    </div>

    <div v-if="rbacStore.loading" class="loading">{{ $t('common.loading') }}</div>

    <div v-else-if="rbacStore.roles.length === 0" class="empty">
      {{ $t('rbac.noRoles') }}
    </div>

    <div v-else class="role-list">
      <div v-for="role in rbacStore.roles" :key="role.id" class="role-card">
        <div class="role-header">
          <div class="role-name-row">
            <span class="role-name">{{ role.name }}</span>
            <span v-if="role.is_system" class="badge-system">{{ $t('rbac.system') }}</span>
            <span v-else class="badge-custom">{{ $t('rbac.custom') }}</span>
          </div>
          <div class="role-actions">
            <button
              v-if="!role.is_system"
              class="btn-text"
              @click="startEdit(role)"
            >{{ $t('rbac.editRole') }}</button>
            <button
              v-if="!role.is_system"
              class="btn-text btn-danger"
              @click="deleteRole(role)"
            >{{ $t('rbac.deleteRole') }}</button>
          </div>
        </div>
        <div v-if="role.description" class="role-desc">{{ role.description }}</div>
        <div class="perm-tags">
          <span
            v-for="perm in role.permissions"
            :key="perm"
            class="perm-tag"
          >{{ perm }}</span>
        </div>
      </div>
    </div>

    <!-- Create/Edit dialog -->
    <div v-if="showForm" class="dialog-overlay" @click.self="showForm = false">
      <div class="dialog">
        <h3>{{ editingRole ? $t('rbac.editRole') : $t('rbac.addRole') }}</h3>
        <div class="field">
          <label>{{ $t('rbac.roleName') }}</label>
          <input v-model="formName" class="input" type="text" />
        </div>
        <div class="field">
          <label>{{ $t('rbac.roleDesc') }}</label>
          <input v-model="formDesc" class="input" type="text" />
        </div>
        <div class="field">
          <label>{{ $t('rbac.permissions') }}</label>
          <div class="perm-groups">
            <div v-for="group in permGroups" :key="group.label" class="perm-group">
              <div class="group-label">{{ group.label }}</div>
              <div class="group-perms">
                <label
                  v-for="perm in group.perms"
                  :key="perm"
                  class="perm-checkbox"
                >
                  <input
                    type="checkbox"
                    :checked="formPerms.includes(perm)"
                    @change="togglePerm(perm)"
                  />
                  <span>{{ perm.split(':')[1] }}</span>
                </label>
              </div>
            </div>
          </div>
        </div>
        <div class="dialog-actions">
          <button class="btn-secondary" @click="showForm = false">{{ $t('rbac.cancel') }}</button>
          <button class="btn-primary" :disabled="saving" @click="saveForm">{{ $t('rbac.save') }}</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.roles-view { padding: 24px; max-width: 900px; }
.page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 20px;
}
.page-title { margin: 0; font-size: 20px; font-weight: 600; }
.loading, .empty { color: var(--text-secondary, #a6adc8); font-size: 14px; }
.role-list { display: flex; flex-direction: column; gap: 10px; }
.role-card {
  background: var(--surface-color, #1e1e2e);
  border: 1px solid var(--border-color, #313244);
  border-radius: 6px;
  padding: 14px 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.role-header { display: flex; align-items: center; justify-content: space-between; }
.role-name-row { display: flex; align-items: center; gap: 8px; }
.role-name { font-weight: 600; font-size: 15px; }
.badge-system, .badge-custom {
  font-size: 10px;
  padding: 2px 6px;
  border-radius: 8px;
  font-weight: 600;
}
.badge-system { background: #585b7066; color: #a6adc8; }
.badge-custom { background: #cba6f733; color: #cba6f7; }
.role-actions { display: flex; gap: 8px; }
.btn-text {
  font-size: 12px;
  background: none;
  border: none;
  color: var(--text-secondary, #a6adc8);
  cursor: pointer;
  padding: 2px 4px;
}
.btn-text:hover { color: var(--text-primary, #cdd6f4); }
.btn-danger { color: var(--error-color, #f38ba8) !important; }
.role-desc { font-size: 13px; color: var(--text-secondary, #a6adc8); }
.perm-tags { display: flex; flex-wrap: wrap; gap: 4px; }
.perm-tag {
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 10px;
  background: var(--code-bg, #181825);
  border: 1px solid var(--border-color, #313244);
  color: var(--text-secondary, #a6adc8);
}
.dialog-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0,0,0,0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}
.dialog {
  background: var(--surface-color, #1e1e2e);
  border: 1px solid var(--border-color, #313244);
  border-radius: 8px;
  padding: 24px;
  width: 560px;
  max-width: 95vw;
  max-height: 85vh;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.field { display: flex; flex-direction: column; gap: 6px; }
.field > label { font-size: 12px; color: var(--text-secondary, #a6adc8); }
.input {
  background: var(--input-bg, #313244);
  border: 1px solid var(--border-color, #45475a);
  border-radius: 4px;
  padding: 8px 10px;
  color: inherit;
  font-size: 13px;
  outline: none;
}
.perm-groups { display: flex; flex-direction: column; gap: 10px; }
.perm-group { display: flex; flex-direction: column; gap: 4px; }
.group-label {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-secondary, #a6adc8);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}
.group-perms { display: flex; flex-wrap: wrap; gap: 8px; }
.perm-checkbox {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  cursor: pointer;
}
.dialog-actions { display: flex; justify-content: flex-end; gap: 8px; }
.btn-primary {
  padding: 8px 16px; border-radius: 4px; border: none; cursor: pointer;
  background: var(--accent-color, #cba6f7); color: #1e1e2e; font-weight: 600; font-size: 13px;
}
.btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }
.btn-secondary {
  padding: 8px 16px; border-radius: 4px;
  border: 1px solid var(--border-color, #45475a);
  cursor: pointer; background: transparent; color: inherit; font-size: 13px;
}
</style>
