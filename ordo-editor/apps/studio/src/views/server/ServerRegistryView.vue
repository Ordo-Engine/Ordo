<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { MessagePlugin } from 'tdesign-vue-next'
import { serverApi } from '@/api/platform-client'
import type { ServerInfo } from '@/api/types'
import { useAuthStore } from '@/stores/auth'

interface HealthPayload {
  online: boolean
  response?: string
  error?: string
  url: string
}

interface MetricSample {
  name: string
  value: string
  labels: string[]
}

interface ServerPanelState {
  healthOpen: boolean
  metricsOpen: boolean
  healthRawOpen: boolean
  metricsRawOpen: boolean
}

const { t } = useI18n()
const router = useRouter()
const auth = useAuthStore()

const loadingServers = ref(false)
const servers = ref<ServerInfo[]>([])
const panelState = reactive<Record<string, ServerPanelState>>({})
const healthLoading = reactive<Record<string, boolean>>({})
const metricsLoading = reactive<Record<string, boolean>>({})
const healthContent = reactive<Record<string, HealthPayload | undefined>>({})
const metricsRaw = reactive<Record<string, string | undefined>>({})
const metricsParsed = reactive<Record<string, MetricSample[] | undefined>>({})

function ensurePanelState(serverId: string) {
  if (!panelState[serverId]) {
    panelState[serverId] = {
      healthOpen: false,
      metricsOpen: false,
      healthRawOpen: false,
      metricsRawOpen: false,
    }
  }
  return panelState[serverId]
}

function formatTimestamp(value: string | null | undefined) {
  if (!value) return t('settings.serverRegistry.neverSeen')
  return new Date(value).toLocaleString()
}

function serverTheme(status: ServerInfo['status']) {
  switch (status) {
    case 'online':
      return 'success'
    case 'degraded':
      return 'warning'
    default:
      return 'danger'
  }
}

function parseMetrics(raw: string): MetricSample[] {
  return raw
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && !line.startsWith('#'))
    .map((line) => {
      const match = line.match(/^([^\s{]+)(?:\{([^}]*)\})?\s+(.+)$/)
      if (!match) {
        return {
          name: line,
          value: '',
          labels: [],
        }
      }

      const [, name, labelsPart, value] = match
      const labels = labelsPart
        ? labelsPart
            .split(/,(?=[a-zA-Z_][a-zA-Z0-9_]*=)/)
            .map((label) => label.trim())
            .filter(Boolean)
        : []

      return { name, value: value.trim(), labels }
    })
}

async function loadServers() {
  if (!auth.token) return
  loadingServers.value = true
  try {
    servers.value = await serverApi.list(auth.token)
    servers.value.forEach((server) => ensurePanelState(server.id))
  } catch {
    MessagePlugin.error(t('settings.serverRegistry.loadFailed'))
  } finally {
    loadingServers.value = false
  }
}

async function loadHealth(server: ServerInfo) {
  if (!auth.token) return
  healthLoading[server.id] = true
  try {
    healthContent[server.id] = await serverApi.getHealth(auth.token, server.id)
  } catch (error: any) {
    healthContent[server.id] = {
      online: false,
      error: error.message || t('settings.serverRegistry.healthFailed'),
      url: server.url,
    }
  } finally {
    healthLoading[server.id] = false
  }
}

async function loadMetrics(server: ServerInfo) {
  if (!auth.token) return
  metricsLoading[server.id] = true
  try {
    const raw = await serverApi.getMetrics(auth.token, server.id)
    metricsRaw[server.id] = raw
    metricsParsed[server.id] = parseMetrics(raw)
  } catch (error: any) {
    metricsRaw[server.id] = error.message || t('settings.serverRegistry.metricsFailed')
    metricsParsed[server.id] = []
  } finally {
    metricsLoading[server.id] = false
  }
}

async function toggleHealth(server: ServerInfo) {
  const state = ensurePanelState(server.id)
  state.healthOpen = !state.healthOpen
  if (state.healthOpen && healthContent[server.id] === undefined) {
    await loadHealth(server)
  }
}

async function toggleMetrics(server: ServerInfo) {
  const state = ensurePanelState(server.id)
  state.metricsOpen = !state.metricsOpen
  if (state.metricsOpen && metricsRaw[server.id] === undefined) {
    await loadMetrics(server)
  }
}

function toggleRaw(serverId: string, section: 'health' | 'metrics') {
  const state = ensurePanelState(serverId)
  if (section === 'health') {
    state.healthRawOpen = !state.healthRawOpen
    return
  }
  state.metricsRawOpen = !state.metricsRawOpen
}

function healthFacts(server: ServerInfo) {
  const payload = healthContent[server.id]
  if (!payload) return []

  const facts = [
    { label: t('settings.serverRegistry.endpoint'), value: payload.url || server.url },
    { label: t('settings.serverRegistry.lastSeen'), value: formatTimestamp(server.last_seen) },
  ]

  if (payload.response) {
    facts.push({ label: t('settings.serverRegistry.response'), value: payload.response })
  }

  if (payload.error) {
    facts.push({ label: t('settings.serverRegistry.error'), value: payload.error })
  }

  return facts
}

onMounted(loadServers)
</script>

<template>
  <div class="registry-page">
    <t-breadcrumb class="breadcrumb">
      <t-breadcrumb-item @click="router.push('/dashboard')">{{ t('breadcrumb.home') }}</t-breadcrumb-item>
      <t-breadcrumb-item>{{ t('settings.serverRegistry.title') }}</t-breadcrumb-item>
    </t-breadcrumb>

    <div class="page-header">
      <div>
        <h1 class="page-title">{{ t('settings.serverRegistry.title') }}</h1>
        <p class="page-desc">{{ t('settings.serverRegistry.desc') }}</p>
      </div>
      <t-button variant="outline" :loading="loadingServers" @click="loadServers">
        {{ t('settings.serverRegistry.refresh') }}
      </t-button>
    </div>

    <section class="registry-panel">
      <div v-if="servers.length" class="server-list">
        <article v-for="server in servers" :key="server.id" class="server-item">
          <div class="server-item__header">
            <div>
              <div class="server-item__name">{{ server.name }}</div>
              <div class="server-item__meta">{{ server.url }}</div>
            </div>
            <t-tag :theme="serverTheme(server.status)" variant="light">
              {{ server.status }}
            </t-tag>
          </div>

          <div class="server-item__facts">
            <span>{{ t('settings.serverRegistry.version') }}: {{ server.version || '-' }}</span>
            <span>{{ t('settings.serverRegistry.lastSeen') }}: {{ formatTimestamp(server.last_seen) }}</span>
            <span>{{ t('settings.serverRegistry.registeredAt') }}: {{ formatTimestamp(server.registered_at) }}</span>
          </div>

          <div class="server-item__labels">
            <span class="server-item__labels-title">{{ t('settings.serverRegistry.labels') }}</span>
            <template v-if="Object.keys(server.labels).length > 0">
              <span
                v-for="(value, key) in server.labels"
                :key="key"
                class="server-item__label-chip"
              >
                {{ key }}={{ value }}
              </span>
            </template>
            <span v-else class="server-item__label-empty">{{ t('settings.serverRegistry.noLabels') }}</span>
          </div>

          <div class="server-item__actions">
            <t-button
              size="small"
              variant="outline"
              :class="{ 'server-action--active': panelState[server.id]?.healthOpen }"
              @click="toggleHealth(server)"
            >
              {{ t('settings.serverRegistry.healthAction') }}
            </t-button>
            <t-button
              size="small"
              variant="outline"
              :class="{ 'server-action--active': panelState[server.id]?.metricsOpen }"
              @click="toggleMetrics(server)"
            >
              {{ t('settings.serverRegistry.metricsAction') }}
            </t-button>
          </div>

          <section v-if="panelState[server.id]?.healthOpen" class="detail-panel">
            <div class="detail-panel__header">
              <div class="detail-panel__title">{{ t('settings.serverRegistry.healthAction') }}</div>
              <t-button size="small" variant="text" :loading="healthLoading[server.id]" @click="loadHealth(server)">
                {{ t('settings.serverRegistry.refresh') }}
              </t-button>
            </div>

            <div v-if="healthLoading[server.id]" class="detail-loading">
              <t-loading size="small" />
              <span>{{ t('settings.serverRegistry.loadingHealth') }}</span>
            </div>

            <template v-else-if="healthContent[server.id]">
              <div class="health-summary">
                <t-tag :theme="healthContent[server.id]?.online ? 'success' : 'danger'" variant="light">
                  {{
                    healthContent[server.id]?.online
                      ? t('settings.serverRegistry.healthOnline')
                      : t('settings.serverRegistry.healthOffline')
                  }}
                </t-tag>
              </div>

              <div class="detail-grid">
                <div v-for="fact in healthFacts(server)" :key="fact.label" class="detail-row">
                  <span>{{ fact.label }}</span>
                  <strong>{{ fact.value }}</strong>
                </div>
              </div>

              <div class="raw-toggle">
                <t-button size="small" variant="text" @click="toggleRaw(server.id, 'health')">
                  {{
                    panelState[server.id]?.healthRawOpen
                      ? t('settings.serverRegistry.hideRaw')
                      : t('settings.serverRegistry.showRaw')
                  }}
                </t-button>
              </div>

              <pre v-if="panelState[server.id]?.healthRawOpen" class="detail-pre">{{
                JSON.stringify(healthContent[server.id], null, 2)
              }}</pre>
            </template>
          </section>

          <section v-if="panelState[server.id]?.metricsOpen" class="detail-panel">
            <div class="detail-panel__header">
              <div class="detail-panel__title">{{ t('settings.serverRegistry.metricsAction') }}</div>
              <t-button size="small" variant="text" :loading="metricsLoading[server.id]" @click="loadMetrics(server)">
                {{ t('settings.serverRegistry.refresh') }}
              </t-button>
            </div>

            <div v-if="metricsLoading[server.id]" class="detail-loading">
              <t-loading size="small" />
              <span>{{ t('settings.serverRegistry.loadingMetrics') }}</span>
            </div>

            <template v-else>
              <div v-if="metricsParsed[server.id]?.length" class="metrics-table">
                <div class="metrics-table__head">
                  <span>{{ t('settings.serverRegistry.metricName') }}</span>
                  <span>{{ t('settings.serverRegistry.metricValue') }}</span>
                  <span>{{ t('settings.serverRegistry.labels') }}</span>
                </div>
                <div v-for="sample in metricsParsed[server.id]" :key="`${sample.name}-${sample.value}-${sample.labels.join('|')}`" class="metrics-table__row">
                  <span class="metrics-table__name">{{ sample.name }}</span>
                  <strong class="metrics-table__value">{{ sample.value }}</strong>
                  <span class="metrics-table__labels">
                    {{ sample.labels.length ? sample.labels.join(', ') : t('settings.serverRegistry.noLabels') }}
                  </span>
                </div>
              </div>

              <div v-else class="detail-empty">
                {{ t('settings.serverRegistry.noMetrics') }}
              </div>

              <div class="raw-toggle">
                <t-button size="small" variant="text" @click="toggleRaw(server.id, 'metrics')">
                  {{
                    panelState[server.id]?.metricsRawOpen
                      ? t('settings.serverRegistry.hideRaw')
                      : t('settings.serverRegistry.showRaw')
                  }}
                </t-button>
              </div>

              <pre v-if="panelState[server.id]?.metricsRawOpen" class="detail-pre">{{
                metricsRaw[server.id] || ''
              }}</pre>
            </template>
          </section>
        </article>
      </div>

      <div v-else class="registry-empty">
        <strong>{{ t('settings.serverRegistry.empty') }}</strong>
      </div>
    </section>
  </div>
</template>

<style scoped>
.registry-page {
  height: 100%;
  overflow-y: auto;
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 18px;
}

.breadcrumb {
  margin-bottom: -4px;
}

.page-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.page-title {
  margin: 0;
  font-size: 22px;
  font-weight: 600;
  color: #1f2328;
}

.page-desc {
  margin: 6px 0 0;
  font-size: 13px;
  line-height: 1.5;
  color: #6b7280;
}

.registry-panel {
  background: #ffffff;
  border: 1px solid #e7e3d8;
  border-radius: 10px;
  padding: 16px;
}

.server-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.server-item {
  padding: 14px 16px;
  border-radius: 10px;
  background: #fcfbf8;
  border: 1px solid #ece8dd;
}

.server-item__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.server-item__name {
  font-size: 14px;
  font-weight: 600;
  color: #1f2328;
}

.server-item__meta {
  margin-top: 4px;
  font-size: 12px;
  color: #6b7280;
  font-family: 'JetBrains Mono', monospace;
  word-break: break-word;
}

.server-item__facts {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  margin-top: 10px;
  font-size: 12px;
  color: #6b7280;
}

.server-item__labels {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  margin-top: 12px;
}

.server-item__labels-title {
  font-size: 12px;
  font-weight: 600;
  color: #4b5563;
}

.server-item__label-chip,
.server-item__label-empty {
  padding: 4px 8px;
  border-radius: 999px;
  font-size: 11px;
  color: #6b7280;
  background: #f3f1ea;
}

.server-item__actions {
  display: flex;
  gap: 8px;
  margin-top: 12px;
}

.server-action--active {
  border-color: #cdd8ff;
  color: #335eea;
  background: #f4f7ff;
}

.detail-panel {
  margin-top: 12px;
  padding: 14px;
  border-radius: 8px;
  border: 1px solid #e6e0d3;
  background: #ffffff;
}

.detail-panel__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.detail-panel__title {
  font-size: 13px;
  font-weight: 600;
  color: #1f2328;
}

.detail-loading,
.detail-empty {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 12px;
  font-size: 12px;
  color: #6b7280;
}

.health-summary {
  margin-top: 12px;
}

.detail-grid {
  display: grid;
  gap: 10px;
  margin-top: 12px;
}

.detail-row {
  display: grid;
  grid-template-columns: 120px minmax(0, 1fr);
  gap: 12px;
  align-items: start;
  font-size: 12px;
}

.detail-row span {
  color: #6b7280;
}

.detail-row strong {
  color: #1f2328;
  line-height: 1.5;
  word-break: break-word;
}

.metrics-table {
  margin-top: 12px;
  border: 1px solid #ece8dd;
  border-radius: 8px;
  overflow: hidden;
}

.metrics-table__head,
.metrics-table__row {
  display: grid;
  grid-template-columns: minmax(0, 1.3fr) 120px minmax(0, 1fr);
  gap: 12px;
  padding: 10px 12px;
  font-size: 12px;
  align-items: start;
}

.metrics-table__head {
  background: #f7f5ef;
  font-weight: 600;
  color: #4b5563;
}

.metrics-table__row + .metrics-table__row {
  border-top: 1px solid #ece8dd;
}

.metrics-table__name,
.metrics-table__value {
  font-family: 'JetBrains Mono', monospace;
  color: #1f2328;
  word-break: break-word;
}

.metrics-table__labels {
  color: #6b7280;
  word-break: break-word;
}

.raw-toggle {
  margin-top: 12px;
}

.detail-pre {
  margin: 8px 0 0;
  max-height: 320px;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-word;
  padding: 12px;
  border-radius: 8px;
  background: #f7f5ef;
  color: #4b5563;
  font-size: 12px;
  font-family: 'JetBrains Mono', monospace;
}

.registry-empty {
  min-height: 240px;
  display: flex;
  align-items: center;
  justify-content: center;
  text-align: center;
  color: #6b7280;
}

@media (max-width: 900px) {
  .page-header {
    flex-direction: column;
    align-items: stretch;
  }

  .detail-row,
  .metrics-table__head,
  .metrics-table__row {
    grid-template-columns: 1fr;
  }
}
</style>
