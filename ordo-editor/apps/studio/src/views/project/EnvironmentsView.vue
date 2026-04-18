<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useEnvironmentStore } from '@/stores/environment'
import { useServerStore } from '@/stores/server'
import { MessagePlugin } from 'tdesign-vue-next'
import type { ProjectEnvironment } from '@/api/types'

const route = useRoute()
const { t } = useI18n()
const envStore = useEnvironmentStore()
const serverStore = useServerStore()

const orgId = route.params.orgId as string
const projectId = route.params.projectId as string

// Create/Edit form
const showForm = ref(false)
const editingEnv = ref<ProjectEnvironment | null>(null)
const formName = ref('')
const formServerId = ref<string | null>(null)
const formNatsPrefix = ref('')
const saving = ref(false)

// Canary form
const showCanaryForm = ref(false)
const canaryEnv = ref<ProjectEnvironment | null>(null)
const canaryTargetId = ref<string | null>(null)
const canaryPct = ref(0)

onMounted(async () => {
  await Promise.all([
    envStore.fetchEnvironments(orgId, projectId),
    serverStore.fetchServers(),
  ])
})

function startCreate() {
  editingEnv.value = null
  formName.value = ''
  formServerId.value = null
  formNatsPrefix.value = ''
  showForm.value = true
}

function startEdit(env: ProjectEnvironment) {
  editingEnv.value = env
  formName.value = env.name
  formServerId.value = env.server_id
  formNatsPrefix.value = env.nats_subject_prefix ?? ''
  showForm.value = true
}

async function saveForm() {
  if (!formName.value.trim()) return
  saving.value = true
  try {
    if (editingEnv.value) {
      await envStore.updateEnvironment(orgId, projectId, editingEnv.value.id, {
        name: formName.value.trim(),
        server_id: formServerId.value || null,
        nats_subject_prefix: formNatsPrefix.value.trim() || null,
      })
      MessagePlugin.success(t('environment.updated'))
    } else {
      await envStore.createEnvironment(orgId, projectId, {
        name: formName.value.trim(),
        server_id: formServerId.value || null,
        nats_subject_prefix: formNatsPrefix.value.trim() || null,
      })
      MessagePlugin.success(t('environment.created'))
    }
    showForm.value = false
  } catch (e: any) {
    MessagePlugin.error(e.message)
  } finally {
    saving.value = false
  }
}

async function deleteEnv(env: ProjectEnvironment) {
  if (!confirm(t('environment.confirmDelete', { name: env.name }))) return
  try {
    await envStore.deleteEnvironment(orgId, projectId, env.id)
    MessagePlugin.success(t('environment.deleted'))
  } catch (e: any) {
    MessagePlugin.error(e.message)
  }
}

function startCanary(env: ProjectEnvironment) {
  canaryEnv.value = env
  canaryTargetId.value = env.canary_target_env_id
  canaryPct.value = env.canary_percentage
  showCanaryForm.value = true
}

async function saveCanary() {
  if (!canaryEnv.value) return
  try {
    await envStore.setCanary(orgId, projectId, canaryEnv.value.id, {
      canary_target_env_id: canaryTargetId.value,
      canary_percentage: canaryPct.value,
    })
    MessagePlugin.success(t('environment.canaryUpdated'))
    showCanaryForm.value = false
  } catch (e: any) {
    MessagePlugin.error(e.message)
  }
}

function serverName(serverId: string | null) {
  if (!serverId) return t('environment.noServer')
  return serverStore.servers.find((s) => s.id === serverId)?.name ?? serverId
}

function canaryTargetName(envId: string | null) {
  if (!envId) return t('environment.noCanary')
  return envStore.environments.find((e) => e.id === envId)?.name ?? envId
}
</script>

<template>
  <div class="environments-view">
    <div class="page-header">
      <h2 class="page-title">{{ $t('environment.title') }}</h2>
      <button class="btn-primary" @click="startCreate">{{ $t('environment.add') }}</button>
    </div>

    <div v-if="envStore.loading" class="loading">{{ $t('common.loading') }}</div>

    <div v-else-if="envStore.environments.length === 0" class="empty">
      {{ $t('environment.noEnvs') }}
    </div>

    <div v-else class="env-list">
      <div v-for="env in envStore.environments" :key="env.id" class="env-card">
        <div class="env-header">
          <div class="env-name-row">
            <span class="env-name">{{ env.name }}</span>
            <span v-if="env.is_default" class="badge-default">{{ $t('environment.default') }}</span>
          </div>
          <div class="env-actions">
            <button class="btn-text" @click="startCanary(env)">{{ $t('environment.canary') }}</button>
            <button class="btn-text" @click="startEdit(env)">{{ $t('environment.edit') }}</button>
            <button
              v-if="!env.is_default"
              class="btn-text btn-danger"
              @click="deleteEnv(env)"
            >{{ $t('environment.delete') }}</button>
          </div>
        </div>
        <div class="env-meta">
          <span class="meta-label">{{ $t('environment.server') }}:</span>
          <span>{{ serverName(env.server_id) }}</span>
        </div>
        <div v-if="env.nats_subject_prefix" class="env-meta">
          <span class="meta-label">{{ $t('environment.natsPrefix') }}:</span>
          <code>{{ env.nats_subject_prefix }}</code>
        </div>
        <div v-if="env.canary_percentage > 0" class="env-meta">
          <span class="meta-label">{{ $t('environment.canary') }}:</span>
          <span>{{ env.canary_percentage }}% → {{ canaryTargetName(env.canary_target_env_id) }}</span>
        </div>
      </div>
    </div>

    <!-- Create/Edit dialog -->
    <div v-if="showForm" class="dialog-overlay" @click.self="showForm = false">
      <div class="dialog">
        <h3>{{ editingEnv ? $t('environment.edit') : $t('environment.add') }}</h3>
        <div class="field">
          <label>{{ $t('environment.name') }}</label>
          <input v-model="formName" class="input" type="text" />
        </div>
        <div class="field">
          <label>{{ $t('environment.server') }}</label>
          <select v-model="formServerId" class="input">
            <option :value="null">{{ $t('environment.noServer') }}</option>
            <option v-for="srv in serverStore.servers" :key="srv.id" :value="srv.id">
              {{ srv.name }}
            </option>
          </select>
        </div>
        <div class="field">
          <label>{{ $t('environment.natsPrefix') }}</label>
          <input v-model="formNatsPrefix" class="input" type="text" placeholder="ordo.rules" />
        </div>
        <div class="dialog-actions">
          <button class="btn-secondary" @click="showForm = false">{{ $t('environment.cancel') }}</button>
          <button class="btn-primary" :disabled="saving" @click="saveForm">{{ $t('environment.save') }}</button>
        </div>
      </div>
    </div>

    <!-- Canary dialog -->
    <div v-if="showCanaryForm" class="dialog-overlay" @click.self="showCanaryForm = false">
      <div class="dialog">
        <h3>{{ $t('environment.canary') }}: {{ canaryEnv?.name }}</h3>
        <div class="field">
          <label>{{ $t('environment.canaryTarget') }}</label>
          <select v-model="canaryTargetId" class="input">
            <option :value="null">{{ $t('environment.noCanary') }}</option>
            <option
              v-for="env in envStore.environments.filter((e) => e.id !== canaryEnv?.id)"
              :key="env.id"
              :value="env.id"
            >
              {{ env.name }}
            </option>
          </select>
        </div>
        <div class="field">
          <label>{{ $t('environment.canaryPct') }}: {{ canaryPct }}%</label>
          <input v-model.number="canaryPct" type="range" min="0" max="100" class="range-input" />
        </div>
        <div class="dialog-actions">
          <button class="btn-secondary" @click="showCanaryForm = false">{{ $t('environment.cancel') }}</button>
          <button class="btn-primary" @click="saveCanary">{{ $t('environment.save') }}</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.environments-view {
  padding: 24px;
  max-width: 800px;
}
.page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 20px;
}
.page-title {
  margin: 0;
  font-size: 20px;
  font-weight: 600;
}
.loading, .empty {
  color: var(--text-secondary, #a6adc8);
  font-size: 14px;
}
.env-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.env-card {
  background: var(--surface-color, #1e1e2e);
  border: 1px solid var(--border-color, #313244);
  border-radius: 6px;
  padding: 14px 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.env-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.env-name-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.env-name {
  font-weight: 600;
  font-size: 15px;
}
.badge-default {
  font-size: 10px;
  padding: 2px 6px;
  border-radius: 8px;
  background: var(--accent-color, #cba6f7);
  color: #1e1e2e;
  font-weight: 600;
}
.env-actions {
  display: flex;
  gap: 8px;
}
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
.env-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: var(--text-secondary, #a6adc8);
}
.meta-label { font-weight: 600; }
code {
  font-family: monospace;
  font-size: 12px;
  background: var(--code-bg, #181825);
  padding: 2px 6px;
  border-radius: 3px;
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
  width: 400px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.field { display: flex; flex-direction: column; gap: 6px; }
.field label { font-size: 12px; color: var(--text-secondary, #a6adc8); }
.input {
  background: var(--input-bg, #313244);
  border: 1px solid var(--border-color, #45475a);
  border-radius: 4px;
  padding: 8px 10px;
  color: inherit;
  font-size: 13px;
  outline: none;
}
.range-input { width: 100%; }
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
