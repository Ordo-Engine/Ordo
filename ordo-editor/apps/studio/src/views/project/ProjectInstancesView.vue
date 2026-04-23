<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { MessagePlugin } from 'tdesign-vue-next'
import { useAuthStore } from '@/stores/auth'
import { useOrgStore } from '@/stores/org'
import { useProjectStore } from '@/stores/project'
import { useServerStore } from '@/stores/server'
import { projectApi } from '@/api/platform-client'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const auth = useAuthStore()
const orgStore = useOrgStore()
const projectStore = useProjectStore()
const serverStore = useServerStore()

const orgId = computed(() => route.params.orgId as string)
const projectId = computed(() => route.params.projectId as string)
const project = computed(() => projectStore.currentProject)

const selectedServerId = ref<string>('default')
const binding = ref(false)
const healthLoading = ref(false)
const healthStatus = ref<{ online: boolean; response?: string; error?: string; url: string } | null>(null)

const serverOptions = computed(() => [
  { label: t('projectInstances.useDefault'), value: 'default' },
  ...serverStore.servers.map((s) => ({
    label: `${s.name}${s.version ? ` (${s.version})` : ''}`,
    value: s.id,
  })),
])

const boundServer = computed(() => {
  if (selectedServerId.value === 'default') return null
  return serverStore.getById(selectedServerId.value) ?? null
})

const statusTheme = computed(() => {
  if (!boundServer.value) return 'default'
  switch (boundServer.value.status) {
    case 'online': return 'success'
    case 'degraded': return 'warning'
    default: return 'danger'
  }
})

function formatTs(v: string | null | undefined) {
  if (!v) return t('projectInstances.neverSeen')
  return new Date(v).toLocaleString()
}

const canAdmin = computed(() => !!auth.user && orgStore.canAdmin(auth.user.id))

onMounted(async () => {
  await serverStore.fetchServers()
  if (project.value) {
    selectedServerId.value = project.value.server_id ?? 'default'
  }
})

async function saveBinding() {
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
    MessagePlugin.success(t('projectInstances.saveSuccess'))
  } catch {
    MessagePlugin.error(t('projectInstances.saveFailed'))
  } finally {
    binding.value = false
  }
}

async function checkHealth() {
  if (!auth.token || !boundServer.value) return
  healthLoading.value = true
  try {
    const { serverApi } = await import('@/api/platform-client')
    healthStatus.value = await serverApi.getHealth(auth.token, boundServer.value.id)
  } catch {
    MessagePlugin.error(t('projectInstances.healthFailed'))
  } finally {
    healthLoading.value = false
  }
}
</script>

<template>
  <div class="instances-page">
    <t-breadcrumb class="breadcrumb">
      <t-breadcrumb-item @click="router.push('/dashboard')">{{ t('breadcrumb.home') }}</t-breadcrumb-item>
      <t-breadcrumb-item @click="router.push(`/orgs/${orgId}/projects`)">{{ t('breadcrumb.projects') }}</t-breadcrumb-item>
      <t-breadcrumb-item>{{ project?.name }}</t-breadcrumb-item>
      <t-breadcrumb-item>{{ t('projectNav.instances') }}</t-breadcrumb-item>
    </t-breadcrumb>

    <h1 class="page-title">{{ t('projectNav.instances') }}</h1>

    <div class="instances-layout">
      <!-- Bound server status -->
      <t-card :bordered="false" class="card">
        <h2 class="card-title">{{ t('projectInstances.currentStatus') }}</h2>
        <div v-if="boundServer" class="status-row">
          <t-tag :theme="statusTheme" variant="light">{{ boundServer.status }}</t-tag>
          <span class="server-name">{{ boundServer.name }}</span>
          <span class="server-url">{{ boundServer.url }}</span>
        </div>
        <div v-else class="status-row">
          <t-tag theme="default" variant="light">{{ t('projectInstances.defaultEngine') }}</t-tag>
          <span class="hint">{{ t('projectInstances.defaultHint') }}</span>
        </div>

        <div v-if="boundServer" class="meta-grid">
          <div class="meta-row">
            <span>{{ t('projectInstances.version') }}</span>
            <strong>{{ boundServer.version || '-' }}</strong>
          </div>
          <div class="meta-row">
            <span>{{ t('projectInstances.lastSeen') }}</span>
            <strong>{{ formatTs(boundServer.last_seen) }}</strong>
          </div>
          <div class="meta-row">
            <span>{{ t('projectInstances.registeredAt') }}</span>
            <strong>{{ formatTs(boundServer.registered_at) }}</strong>
          </div>
        </div>

        <div v-if="boundServer" class="health-section">
          <t-button variant="outline" size="small" :loading="healthLoading" @click="checkHealth">
            {{ t('projectInstances.checkHealth') }}
          </t-button>
          <div v-if="healthStatus" class="health-result">
            <t-tag :theme="healthStatus.online ? 'success' : 'danger'" variant="light">
              {{ healthStatus.online ? t('projectInstances.online') : t('projectInstances.offline') }}
            </t-tag>
            <pre v-if="healthStatus.response || healthStatus.error" class="health-pre">{{
              healthStatus.response || healthStatus.error
            }}</pre>
          </div>
        </div>
      </t-card>

      <!-- Binding -->
      <t-card :bordered="false" class="card">
        <div class="card-hd">
          <div>
            <h2 class="card-title">{{ t('projectInstances.binding') }}</h2>
            <p class="card-desc">{{ t('projectInstances.bindingDesc') }}</p>
          </div>
          <t-button variant="text" size="small" :loading="serverStore.loading" @click="serverStore.fetchServers()">
            {{ t('projectInstances.refresh') }}
          </t-button>
        </div>
        <t-form label-align="top" :colon="false">
          <t-form-item :label="t('projectInstances.serverLabel')">
            <t-select
              v-model="selectedServerId"
              :disabled="!canAdmin"
              :options="serverOptions"
              :loading="serverStore.loading"
            />
          </t-form-item>
          <t-form-item v-if="canAdmin">
            <t-button theme="primary" :loading="binding" @click="saveBinding">
              {{ t('projectInstances.saveBtn') }}
            </t-button>
          </t-form-item>
        </t-form>
      </t-card>

      <!-- Link to registry -->
      <t-card :bordered="false" class="card registry-link-card">
        <div class="registry-link-row">
          <div>
            <div class="card-title">{{ t('projectInstances.allServers') }}</div>
            <p class="card-desc">{{ t('projectInstances.allServersDesc') }}</p>
          </div>
          <t-button variant="outline" size="small" @click="router.push(`/orgs/${orgId}/servers`)">
            {{ t('projectInstances.goToRegistry') }}
          </t-button>
        </div>
      </t-card>
    </div>
  </div>
</template>

<style scoped>
.instances-page {
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

.instances-layout {
  display: flex;
  flex-direction: column;
  gap: 20px;
  max-width: 640px;
}

.card {
  background: var(--ordo-bg-panel);
  border-radius: 8px;
}

.card-hd {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 16px;
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

.status-row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
}

.server-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--ordo-text-primary);
}

.server-url {
  font-size: 12px;
  font-family: 'JetBrains Mono', monospace;
  color: var(--ordo-text-secondary);
  word-break: break-all;
}

.hint {
  font-size: 13px;
  color: var(--ordo-text-tertiary);
}

.meta-grid {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-top: 16px;
}

.meta-row {
  display: grid;
  grid-template-columns: 120px 1fr;
  gap: 12px;
  font-size: 12px;
}

.meta-row span {
  color: var(--ordo-text-secondary);
}

.meta-row strong {
  color: var(--ordo-text-primary);
  word-break: break-word;
}

.health-section {
  margin-top: 16px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.health-result {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.health-pre {
  margin: 0;
  padding: 10px 12px;
  border-radius: 6px;
  background: var(--ordo-bg-secondary);
  font-size: 12px;
  font-family: 'JetBrains Mono', monospace;
  color: var(--ordo-text-secondary);
  white-space: pre-wrap;
  word-break: break-word;
}

.registry-link-card .card-title {
  margin-bottom: 4px;
}

.registry-link-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}
</style>
