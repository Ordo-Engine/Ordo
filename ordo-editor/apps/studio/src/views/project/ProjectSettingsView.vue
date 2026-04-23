<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { MessagePlugin } from 'tdesign-vue-next'
import { useAuthStore } from '@/stores/auth'
import { useOrgStore } from '@/stores/org'
import { useProjectStore } from '@/stores/project'
import { projectApi, serverApi } from '@/api/platform-client'
import type { ServerInfo } from '@/api/types'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const auth = useAuthStore()
const orgStore = useOrgStore()
const projectStore = useProjectStore()

const orgId = computed(() => route.params.orgId as string)
const projectId = computed(() => route.params.projectId as string)
const project = computed(() => projectStore.currentProject)

const nameValue = ref('')
const descValue = ref('')
const saving = ref(false)

const servers = ref<ServerInfo[]>([])
const serversLoading = ref(false)
const selectedServerId = ref('default')
const binding = ref(false)
const healthLoading = ref(false)
const healthStatus = ref<{ online: boolean; response?: string; error?: string; url: string } | null>(null)

watch(
  project,
  (value) => {
    if (!value) return
    nameValue.value = value.name
    descValue.value = value.description ?? ''
    selectedServerId.value = value.server_id ?? 'default'
  },
  { immediate: true },
)

const canEdit = computed(() => {
  if (!auth.user) return false
  return orgStore.canEdit(auth.user.id)
})

const canAdmin = computed(() => {
  if (!auth.user) return false
  return orgStore.canAdmin(auth.user.id)
})

const serverOptions = computed(() => [
  { label: t('projectSettings.serverDefault'), value: 'default' },
  ...servers.value.map((server) => ({
    label: `${server.name}${server.version ? ` (${server.version})` : ''}`,
    value: server.id,
  })),
])

const boundServer = computed(() => {
  if (selectedServerId.value === 'default') return null
  return servers.value.find((server) => server.id === selectedServerId.value) ?? null
})

const engineTagTheme = computed(() => {
  if (!boundServer.value) return 'default'
  switch (boundServer.value.status) {
    case 'online':
      return 'success'
    case 'degraded':
      return 'warning'
    default:
      return 'danger'
  }
})

const engineStatusText = computed(() => {
  if (!boundServer.value) return t('projectSettings.engineDefault')
  switch (boundServer.value.status) {
    case 'online':
      return t('projectSettings.engineConnected')
    case 'degraded':
      return t('projectSettings.engineDegraded')
    default:
      return t('projectSettings.engineDisconnected')
  }
})

function formatTimestamp(value: string | null | undefined) {
  if (!value) return t('projectSettings.serverNeverSeen')
  return new Date(value).toLocaleString()
}

async function loadServers() {
  if (!auth.token) return
  serversLoading.value = true
  try {
    servers.value = await serverApi.list(auth.token)
  } catch {
    MessagePlugin.error(t('projectSettings.serverLoadFailed'))
  } finally {
    serversLoading.value = false
  }
}

onMounted(loadServers)

async function saveGeneral() {
  if (!nameValue.value.trim()) {
    MessagePlugin.warning(t('projectSettings.nameRequired'))
    return
  }
  if (!orgStore.currentOrg || !auth.token) return

  saving.value = true
  try {
    const updated = await projectApi.update(auth.token, orgStore.currentOrg.id, projectId.value, {
      name: nameValue.value.trim(),
      description: descValue.value.trim() || undefined,
    })
    await projectStore.fetchProjects(orgStore.currentOrg.id)
    await projectStore.selectProject(updated)
    MessagePlugin.success(t('projectSettings.saveSuccess'))
  } catch {
    MessagePlugin.error(t('projectSettings.saveFailed'))
  } finally {
    saving.value = false
  }
}

async function saveServerBinding() {
  if (!orgStore.currentOrg || !auth.token) return

  binding.value = true
  try {
    await projectApi.bindServer(auth.token, orgStore.currentOrg.id, projectId.value, {
      server_id: selectedServerId.value === 'default' ? null : selectedServerId.value,
    })
    const updated = await projectApi.get(auth.token, orgStore.currentOrg.id, projectId.value)
    await projectStore.fetchProjects(orgStore.currentOrg.id)
    await projectStore.selectProject(updated)
    healthStatus.value = null
    MessagePlugin.success(t('projectSettings.serverSaveSuccess'))
  } catch {
    MessagePlugin.error(t('projectSettings.serverSaveFailed'))
  } finally {
    binding.value = false
  }
}

async function checkServerHealth() {
  if (!auth.token || !boundServer.value) return

  healthLoading.value = true
  try {
    healthStatus.value = await serverApi.getHealth(auth.token, boundServer.value.id)
  } catch {
    MessagePlugin.error(t('projectSettings.serverHealthFailed'))
  } finally {
    healthLoading.value = false
  }
}

const showDeleteDialog = ref(false)
const deleting = ref(false)

async function confirmDelete() {
  if (!orgStore.currentOrg) return
  deleting.value = true
  try {
    await projectStore.deleteProject(orgStore.currentOrg.id, projectId.value)
    MessagePlugin.success(t('projectSettings.deleteSuccess'))
    router.push(`/orgs/${orgId.value}/projects`)
  } catch {
    MessagePlugin.error(t('common.saveFailed'))
  } finally {
    deleting.value = false
    showDeleteDialog.value = false
  }
}
</script>

<template>
  <div class="project-settings-page">
    <t-breadcrumb class="breadcrumb">
      <t-breadcrumb-item @click="router.push('/dashboard')">{{ t('breadcrumb.home') }}</t-breadcrumb-item>
      <t-breadcrumb-item @click="router.push(`/orgs/${orgId}/projects`)">{{ t('breadcrumb.projects') }}</t-breadcrumb-item>
      <t-breadcrumb-item>{{ project?.name }}</t-breadcrumb-item>
      <t-breadcrumb-item>{{ t('breadcrumb.projectSettings') }}</t-breadcrumb-item>
    </t-breadcrumb>

    <h1 class="page-title">{{ t('projectSettings.title') }}</h1>

    <div class="settings-layout">
      <t-card :bordered="false" class="settings-card">
        <h2 class="card-title">{{ t('projectSettings.general') }}</h2>
        <t-form label-align="top" :colon="false" class="settings-form">
          <t-form-item :label="t('projectSettings.nameLabel')">
            <t-input v-model="nameValue" :disabled="!canEdit" :placeholder="t('project.namePlaceholder')" />
          </t-form-item>
          <t-form-item :label="t('projectSettings.descLabel')">
            <t-textarea
              v-model="descValue"
              :disabled="!canEdit"
              :placeholder="t('projectSettings.descPlaceholder')"
              :autosize="{ minRows: 2, maxRows: 4 }"
            />
          </t-form-item>
          <t-form-item v-if="canEdit">
            <t-button theme="primary" :loading="saving" @click="saveGeneral">
              {{ t('projectSettings.saveBtn') }}
            </t-button>
          </t-form-item>
        </t-form>
      </t-card>

      <t-card :bordered="false" class="settings-card">
        <h2 class="card-title">{{ t('projectSettings.members') }}</h2>
        <p class="card-desc">{{ t('projectSettings.membersDesc') }}</p>
        <t-button variant="outline" size="small" @click="router.push(`/orgs/${orgStore.currentOrg?.id}/members`)">
          <t-icon name="user" />
          {{ t('projectSettings.viewMembers') }}
        </t-button>
      </t-card>

      <t-card :bordered="false" class="settings-card">
        <h2 class="card-title">{{ t('projectSettings.engineStatus') }}</h2>
        <div class="engine-status">
          <t-tag :theme="engineTagTheme" variant="light">
            <t-icon :name="boundServer ? 'server' : 'link'" />
            {{ engineStatusText }}
          </t-tag>
          <span class="engine-target">
            {{ boundServer ? boundServer.name : t('projectSettings.serverDefault') }}
          </span>
          <span class="engine-id">ID: {{ projectId }}</span>
        </div>
        <div v-if="boundServer" class="engine-meta">
          <span>{{ boundServer.url }}</span>
          <span>{{ t('projectSettings.serverLastSeen') }}: {{ formatTimestamp(boundServer.last_seen) }}</span>
        </div>
      </t-card>

      <t-card :bordered="false" class="settings-card">
        <div class="card-header">
          <div>
            <h2 class="card-title">{{ t('projectSettings.serverBinding') }}</h2>
            <p class="card-desc">{{ t('projectSettings.serverBindingDesc') }}</p>
          </div>
          <t-button variant="text" size="small" :loading="serversLoading" @click="loadServers">
            {{ t('projectSettings.serverRefresh') }}
          </t-button>
        </div>

        <t-form label-align="top" :colon="false" class="settings-form">
          <t-form-item :label="t('projectSettings.serverLabel')">
            <t-select
              v-model="selectedServerId"
              :disabled="!canAdmin"
              :options="serverOptions"
              :loading="serversLoading"
            />
          </t-form-item>
          <t-form-item v-if="canAdmin">
            <t-button theme="primary" :loading="binding" @click="saveServerBinding">
              {{ t('projectSettings.serverSave') }}
            </t-button>
          </t-form-item>
        </t-form>

        <div v-if="boundServer" class="server-detail">
          <div class="server-detail__row">
            <span class="server-detail__label">{{ t('projectSettings.serverVersion') }}</span>
            <span>{{ boundServer.version || '-' }}</span>
          </div>
          <div class="server-detail__row">
            <span class="server-detail__label">{{ t('projectSettings.serverUrl') }}</span>
            <span class="server-url">{{ boundServer.url }}</span>
          </div>
          <div class="server-detail__row">
            <span class="server-detail__label">{{ t('projectSettings.serverLastSeen') }}</span>
            <span>{{ formatTimestamp(boundServer.last_seen) }}</span>
          </div>
          <t-button variant="outline" size="small" :loading="healthLoading" @click="checkServerHealth">
            {{ t('projectSettings.serverCheckHealth') }}
          </t-button>
          <div v-if="healthStatus" class="health-box">
            <t-tag :theme="healthStatus.online ? 'success' : 'danger'" variant="light">
              {{ healthStatus.online ? t('projectSettings.serverHealthOnline') : t('projectSettings.serverHealthOffline') }}
            </t-tag>
            <pre class="health-response">{{ healthStatus.response || healthStatus.error || healthStatus.url }}</pre>
          </div>
        </div>
        <p v-else class="card-desc server-unbound">{{ t('projectSettings.serverUnbound') }}</p>
      </t-card>

      <t-card v-if="canAdmin" :bordered="false" class="settings-card danger-card">
        <h2 class="card-title danger-title">{{ t('projectSettings.danger') }}</h2>
        <div class="danger-row">
          <div>
            <div class="danger-label">{{ t('projectSettings.deleteLabel') }}</div>
            <div class="danger-desc">{{ t('projectSettings.deleteDesc') }}</div>
          </div>
          <t-button theme="danger" variant="outline" @click="showDeleteDialog = true">
            {{ t('projectSettings.deleteBtn') }}
          </t-button>
        </div>
      </t-card>
    </div>

    <t-dialog
      v-model:visible="showDeleteDialog"
      :header="t('projectSettings.deleteDialog')"
      :confirm-btn="{ content: t('projectSettings.deleteBtn'), theme: 'danger', loading: deleting }"
      :cancel-btn="t('common.cancel')"
      @confirm="confirmDelete"
      @cancel="showDeleteDialog = false"
    >
      <p>{{ t('projectSettings.deleteConfirm', { name: project?.name }) }}</p>
    </t-dialog>
  </div>
</template>

<style scoped>
.project-settings-page {
  padding: 32px;
  overflow-y: auto;
  height: 100%;
}

.breadcrumb {
  margin-bottom: 16px;
}

.page-title {
  margin: 0 0 24px;
  font-size: 20px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.settings-layout {
  display: flex;
  flex-direction: column;
  gap: 20px;
  max-width: 600px;
}

.settings-card {
  background: var(--ordo-bg-panel);
  border-radius: 8px;
}

.card-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.card-title {
  margin: 0 0 16px;
  font-size: 14px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.card-desc {
  margin: 0 0 14px;
  font-size: 13px;
  color: var(--ordo-text-secondary);
}

.settings-form {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.engine-status {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
}

.engine-target {
  font-size: 13px;
  color: var(--ordo-text-primary);
}

.engine-meta {
  margin-top: 12px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.engine-id {
  font-size: 12px;
  color: var(--ordo-text-tertiary);
  font-family: 'JetBrains Mono', monospace;
}

.server-detail {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.server-detail__row {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  font-size: 13px;
  color: var(--ordo-text-primary);
}

.server-detail__label {
  color: var(--ordo-text-secondary);
}

.server-url {
  font-family: 'JetBrains Mono', monospace;
  font-size: 12px;
}

.server-unbound {
  margin-bottom: 0;
}

.health-box {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  border-radius: 8px;
  background: var(--ordo-bg-secondary);
}

.health-response {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  font-size: 12px;
  color: var(--ordo-text-secondary);
  font-family: 'JetBrains Mono', monospace;
}

.danger-card {
  border: 1px solid rgba(245, 63, 63, 0.3) !important;
}

.danger-title {
  color: #f53f3f;
}

.danger-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
}

.danger-label {
  font-size: 13px;
  font-weight: 500;
  color: var(--ordo-text-primary);
}

.danger-desc {
  margin-top: 4px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
}
</style>
