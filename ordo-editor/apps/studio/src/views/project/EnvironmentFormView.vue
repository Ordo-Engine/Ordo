<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { MessagePlugin } from 'tdesign-vue-next'
import type { ProjectEnvironment, ServerInfo } from '@/api/types'
import { useEnvironmentStore } from '@/stores/environment'
import { useServerStore } from '@/stores/server'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const envStore = useEnvironmentStore()
const serverStore = useServerStore()

const orgId = route.params.orgId as string
const projectId = route.params.projectId as string
const envId = computed(() => route.params.envId as string | undefined)
const isEdit = computed(() => !!envId.value)

const loading = ref(true)
const saving = ref(false)
const environment = ref<ProjectEnvironment | null>(null)
const serverKeyword = ref('')
const serverFilter = ref<'healthy' | 'all'>('healthy')
const natsPrefixTouched = ref(false)
const lastSelectedServerId = ref<string | null>(null)

const form = ref({
  name: '',
  server_ids: [] as string[],
  nats_subject_prefix: '',
  canary_target_env_id: '',
  canary_percentage: 0,
})

const availableCanaryTargets = computed(() =>
  envStore.environments
    .filter((env) => env.id !== envId.value)
    .map((env) => ({ label: env.name, value: env.id })),
)

const selectedServers = computed(() =>
  form.value.server_ids
    .map((id) => serverStore.getById(id))
    .filter((server): server is ServerInfo => !!server),
)

const visibleServers = computed(() => {
  const query = serverKeyword.value.trim().toLowerCase()
  return serverStore.servers.filter((server) => {
    if (serverFilter.value === 'healthy' && server.status === 'offline') return false
    if (!query) return true
    const haystack = [
      server.name,
      server.url,
      server.version ?? '',
      ...Object.entries(server.labels).map(([key, value]) => `${key}=${value}`),
    ]
      .join(' ')
      .toLowerCase()
    return haystack.includes(query)
  })
})

const tableServers = computed(() =>
  visibleServers.value
    .slice()
    .sort((left, right) => {
      const statusOrder = { online: 0, degraded: 1, offline: 2 }
      const statusDelta = statusOrder[left.status] - statusOrder[right.status]
      if (statusDelta !== 0) return statusDelta
      return left.name.localeCompare(right.name)
    }),
)

const selectableVisibleCount = computed(() =>
  tableServers.value.filter((server) => server.status !== 'offline').length,
)

const selectedVisibleCount = computed(() =>
  tableServers.value.filter((server) => server.status !== 'offline' && isSelected(server.id)).length,
)

const allVisibleSelected = computed(() =>
  selectableVisibleCount.value > 0 && selectedVisibleCount.value === selectableVisibleCount.value,
)

const selectedSummary = computed(() => {
  if (selectedServers.value.length === 0) return t('environment.selectionEmpty')
  const names = selectedServers.value.slice(0, 3).map((server) => server.name)
  const extra = selectedServers.value.length - names.length
  return extra > 0 ? `${names.join(', ')} +${extra}` : names.join(', ')
})

const degradedSelectedServers = computed(() =>
  selectedServers.value.filter((server) => server.status === 'degraded'),
)

const mismatchedSelectedServers = computed(() => {
  const targetTier = inferTier(form.value.name)
  if (!targetTier) return []
  return selectedServers.value.filter((server) => {
    const serverTier = inferTier([
      server.name,
      server.url,
      ...Object.entries(server.labels).map(([key, value]) => `${key}=${value}`),
    ].join(' '))
    return !!serverTier && serverTier !== targetTier
  })
})

function fillForm(env: ProjectEnvironment | null) {
  environment.value = env
  natsPrefixTouched.value = !!env?.nats_subject_prefix
  form.value = {
    name: env?.name ?? '',
    server_ids: env?.server_ids ? [...env.server_ids] : [],
    nats_subject_prefix: env?.nats_subject_prefix ?? '',
    canary_target_env_id: env?.canary_target_env_id ?? '',
    canary_percentage: env?.canary_percentage ?? 0,
  }
}

function inferTier(input: string) {
  const text = input.toLowerCase()
  if (/(prod|production|生产)/.test(text)) return 'prod'
  if (/(staging|stage|预发|預發)/.test(text)) return 'staging'
  if (/(test|testing|dev|develop|测试|測試|开发|開發)/.test(text)) return 'dev'
  return null
}

function slugifyEnvironmentName(input: string) {
  const normalized = input
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9\u4e00-\u9fa5]+/g, '-')
    .replace(/^-+|-+$/g, '')
  return normalized || 'environment'
}

const suggestedNatsPrefix = computed(() => {
  const base = `ordo.rules.${slugifyEnvironmentName(form.value.name)}`
  const currentEnvId = envId.value
  const existing = new Set(
    envStore.environments
      .filter((env) => env.id !== currentEnvId)
      .map((env) => env.nats_subject_prefix)
      .filter((value): value is string => !!value && value.trim().length > 0),
  )

  if (!existing.has(base)) return base

  let index = 2
  let candidate = `${base}-${index}`
  while (existing.has(candidate)) {
    index += 1
    candidate = `${base}-${index}`
  }
  return candidate
})

function isSelected(serverId: string) {
  return form.value.server_ids.includes(serverId)
}

function setSelectedServers(serverIds: string[]) {
  form.value.server_ids = Array.from(new Set(serverIds))
}

function toggleServer(serverId: string, range = false) {
  if (range && lastSelectedServerId.value) {
    const visibleSelectableIds = tableServers.value
      .filter((server) => server.status !== 'offline')
      .map((server) => server.id)
    const start = visibleSelectableIds.indexOf(lastSelectedServerId.value)
    const end = visibleSelectableIds.indexOf(serverId)
    if (start !== -1 && end !== -1) {
      const [from, to] = start < end ? [start, end] : [end, start]
      const rangeIds = visibleSelectableIds.slice(from, to + 1)
      setSelectedServers([...form.value.server_ids, ...rangeIds])
      lastSelectedServerId.value = serverId
      return
    }
  }

  const next = new Set(form.value.server_ids)
  if (next.has(serverId)) {
    next.delete(serverId)
  } else {
    next.add(serverId)
  }
  setSelectedServers(Array.from(next))
  lastSelectedServerId.value = serverId
}

function selectVisibleServers() {
  setSelectedServers([
    ...form.value.server_ids,
    ...tableServers.value.filter((server) => server.status !== 'offline').map((server) => server.id),
  ])
}

function clearVisibleServers() {
  const visibleIds = new Set(tableServers.value.map((server) => server.id))
  setSelectedServers(form.value.server_ids.filter((serverId) => !visibleIds.has(serverId)))
}

function toggleVisibleServers(checked: boolean) {
  if (checked) {
    selectVisibleServers()
  } else {
    clearVisibleServers()
  }
}

function markNatsPrefixTouched() {
  natsPrefixTouched.value = true
}

function statusTheme(status: ServerInfo['status']) {
  if (status === 'online') return 'success'
  if (status === 'degraded') return 'warning'
  return 'danger'
}

function statusLabel(status: ServerInfo['status']) {
  return t(`releaseCenter.serverStatusMap.${status}`)
}

function labelsText(server: ServerInfo) {
  const labels = Object.entries(server.labels)
  if (labels.length === 0) return t('environment.labelsEmpty')
  return labels.map(([key, value]) => `${key}=${value}`).join(', ')
}

watch(
  () => form.value.name,
  (name) => {
    if (natsPrefixTouched.value) return
    form.value.nats_subject_prefix = name.trim() ? suggestedNatsPrefix.value : ''
  },
)

onMounted(async () => {
  try {
    await Promise.all([envStore.fetchEnvironments(orgId, projectId), serverStore.fetchServers()])
    const current = isEdit.value
      ? envStore.environments.find((env) => env.id === envId.value) ?? null
      : null
    if (isEdit.value && !current) {
      MessagePlugin.error(t('common.loadFailed'))
      router.replace({ name: 'project-environments', params: { orgId, projectId } })
      return
    }
    fillForm(current)
  } catch (e: any) {
    MessagePlugin.error(e.message || t('common.loadFailed'))
  } finally {
    loading.value = false
  }
})

async function save() {
  const name = form.value.name.trim()
  if (!name) {
    MessagePlugin.warning(t('environment.name'))
    return
  }

  saving.value = true
  try {
    const payload = {
      name,
      server_ids: form.value.server_ids,
      nats_subject_prefix: form.value.nats_subject_prefix.trim() || null,
    }

    let saved: ProjectEnvironment
    if (isEdit.value && environment.value) {
      saved = await envStore.updateEnvironment(orgId, projectId, environment.value.id, payload)
      MessagePlugin.success(t('environment.updated'))
    } else {
      saved = await envStore.createEnvironment(orgId, projectId, payload)
      MessagePlugin.success(t('environment.created'))
    }

    await envStore.setCanary(orgId, projectId, saved.id, {
      canary_target_env_id: form.value.canary_target_env_id || null,
      canary_percentage: form.value.canary_percentage,
    })

    router.replace({ name: 'project-environments', params: { orgId, projectId } })
  } catch (e: any) {
    MessagePlugin.error(e.message || t('common.saveFailed'))
  } finally {
    saving.value = false
  }
}

function cancel() {
  router.push({ name: 'project-environments', params: { orgId, projectId } })
}
</script>

<template>
  <div class="view-page">
    <div class="page-header">
      <t-button variant="text" @click="cancel">
        <t-icon name="chevron-left" />
        {{ t('environment.title') }}
      </t-button>
      <h2 class="page-title">
        {{ isEdit ? t('environment.editTitle') : t('environment.createTitle') }}
      </h2>
    </div>

    <div v-if="loading" class="loading-state">
      <t-skeleton theme="paragraph" animation="gradient" :row-col="[{}, {}, {}, {}]" />
    </div>

    <div v-else class="page-body">
      <div class="form-col">
        <t-form label-align="top" :colon="false">
          <div class="section-label">{{ t('environment.basicSection') }}</div>

          <t-form-item :label="t('environment.name')" required>
            <t-input v-model="form.name" />
          </t-form-item>

          <t-form-item :label="t('environment.serverNodes')">
            <div class="server-picker">
              <div class="server-picker__toolbar">
                <div class="server-picker__search">
                  <t-input
                    v-model="serverKeyword"
                    :placeholder="t('environment.serverSearchPlaceholder')"
                  />
                </div>
                <div class="server-picker__actions">
                  <t-radio-group v-model="serverFilter" variant="default-filled">
                    <t-radio-button value="healthy">{{ t('environment.serverFilterHealthy') }}</t-radio-button>
                    <t-radio-button value="all">{{ t('environment.serverFilterAll') }}</t-radio-button>
                  </t-radio-group>
                </div>
              </div>

              <div class="server-picker__selection">
                <span class="server-picker__selection-label">{{ t('environment.selectedNodesTitle') }}</span>
                <strong class="server-picker__selection-value">{{ selectedSummary }}</strong>
              </div>

              <div class="server-picker__hint">
                {{ t('environment.serverHint') }}
                {{ t('environment.serverRangeHint') }}
              </div>

              <div v-if="visibleServers.length === 0" class="server-picker__empty">
                {{ t('environment.noServer') }}
              </div>

              <div v-else class="server-grid">
                <table class="server-table">
                  <thead>
                    <tr>
                      <th class="col-check">
                        <t-checkbox
                          :checked="allVisibleSelected"
                          :indeterminate="selectedVisibleCount > 0 && !allVisibleSelected"
                          :disabled="selectableVisibleCount === 0"
                          @change="(checked: boolean) => toggleVisibleServers(checked)"
                        />
                      </th>
                      <th>{{ t('settings.serverRegistry.serverName') }}</th>
                      <th>{{ t('settings.serverRegistry.status') }}</th>
                      <th>{{ t('settings.serverRegistry.endpoint') }}</th>
                      <th>{{ t('settings.serverRegistry.version') }}</th>
                      <th>{{ t('settings.serverRegistry.labels') }}</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr
                      v-for="server in tableServers"
                      :key="server.id"
                      class="server-row"
                      :class="{
                        'server-row--selected': isSelected(server.id),
                        'server-row--disabled': server.status === 'offline',
                      }"
                      @click="server.status !== 'offline' && toggleServer(server.id, $event.shiftKey)"
                    >
                      <td class="col-check">
                        <t-checkbox
                          :checked="isSelected(server.id)"
                          :disabled="server.status === 'offline'"
                        />
                      </td>
                      <td>
                        <div class="server-cell-main">{{ server.name }}</div>
                      </td>
                      <td>
                        <t-tag :theme="statusTheme(server.status)" variant="light" size="small">
                          {{ statusLabel(server.status) }}
                        </t-tag>
                      </td>
                      <td>
                        <div class="server-cell-sub">{{ server.url }}</div>
                      </td>
                      <td>{{ server.version || '—' }}</td>
                      <td>
                        <div class="server-cell-sub">{{ labelsText(server) }}</div>
                      </td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </div>
          </t-form-item>

          <div v-if="degradedSelectedServers.length" class="server-warning">
            {{ t('environment.degradedWarning', { names: degradedSelectedServers.map((server) => server.name).join(', ') }) }}
          </div>

          <div v-if="mismatchedSelectedServers.length" class="server-warning server-warning--danger">
            {{ t('environment.mismatchWarning', { names: mismatchedSelectedServers.map((server) => server.name).join(', ') }) }}
          </div>

          <t-form-item>
            <template #label>
              <span class="field-label-with-help">
                <span>{{ t('environment.natsPrefix') }}</span>
                <t-popup :content="t('environment.natsPrefixHelp')" placement="top">
                  <button type="button" class="field-help" aria-label="NATS prefix help">?</button>
                </t-popup>
              </span>
            </template>
            <t-input
              v-model="form.nats_subject_prefix"
              placeholder="ordo.rules.production"
              @change="markNatsPrefixTouched"
              @blur="markNatsPrefixTouched"
            />
          </t-form-item>

          <div class="section-label">{{ t('environment.canarySection') }}</div>

          <t-form-item :label="t('environment.canaryTarget')">
            <t-select
              v-model="form.canary_target_env_id"
              clearable
              :options="availableCanaryTargets"
              :placeholder="t('common.optional')"
            />
          </t-form-item>

          <t-form-item :label="`${t('environment.canaryPct')}: ${form.canary_percentage}%`">
            <t-slider v-model="form.canary_percentage" :min="0" :max="100" />
          </t-form-item>
        </t-form>

        <div class="form-actions">
          <t-button variant="outline" @click="cancel">{{ t('common.cancel') }}</t-button>
          <t-button theme="primary" :loading="saving" @click="save">{{ t('environment.save') }}</t-button>
        </div>
      </div>

      <div class="preview-col">
        <t-card :bordered="false" class="preview-card">
          <div class="preview-header">{{ t('environment.previewTitle') }}</div>
          <div class="preview-title">{{ form.name || '—' }}</div>
          <div class="preview-list">
            <div class="preview-item">
              <span>{{ t('environment.serverNodes') }}</span>
              <strong>{{ form.server_ids.length }}</strong>
            </div>
            <div class="preview-item">
              <span>{{ t('environment.selectedNodesTitle') }}</span>
              <strong>{{ selectedServers.map((server) => server.name).join(', ') || '—' }}</strong>
            </div>
            <div class="preview-item">
              <span>{{ t('environment.natsPrefix') }}</span>
              <strong>{{ form.nats_subject_prefix || '—' }}</strong>
            </div>
            <div class="preview-item">
              <span>{{ t('environment.canaryTarget') }}</span>
              <strong>{{ availableCanaryTargets.find((item) => item.value === form.canary_target_env_id)?.label || '—' }}</strong>
            </div>
            <div class="preview-item">
              <span>{{ t('environment.canaryPct') }}</span>
              <strong>{{ form.canary_percentage }}%</strong>
            </div>
          </div>
        </t-card>
      </div>
    </div>
  </div>
</template>

<style scoped>
.view-page {
  padding: 24px 32px 40px;
  height: 100%;
  overflow-y: auto;
}

.page-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 24px;
}

.page-title {
  font-size: 18px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  margin: 0;
}

.loading-state {
  max-width: 720px;
}

.page-body {
  display: grid;
  grid-template-columns: 1fr 320px;
  gap: 32px;
  align-items: start;
}

.form-col {
  display: flex;
  flex-direction: column;
}

.section-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--ordo-text-tertiary, #9ca3af);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin: 20px 0 12px;
}

.section-label:first-child {
  margin-top: 0;
}

.form-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 28px;
  padding-top: 20px;
  border-top: 1px solid var(--ordo-border-color);
}

.server-picker {
  display: grid;
  gap: 10px;
  border: 1px solid var(--ordo-border-color);
  border-radius: 18px;
  padding: 16px;
  background: var(--ordo-bg-panel);
  box-shadow: 0 1px 2px rgba(15, 23, 42, 0.04);
}

.server-picker__toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  position: sticky;
  top: 0;
  z-index: 2;
  background: var(--ordo-bg-panel);
}

.server-picker__search {
  flex: 1 1 auto;
  min-width: 0;
}

.server-picker__actions {
  display: flex;
  align-items: center;
  gap: 10px;
  flex: 0 0 auto;
  white-space: nowrap;
}

.server-picker__search :deep(.t-input) {
  width: 100%;
}

.server-picker__toolbar :deep(.t-radio-group__container) {
  display: flex;
  align-items: center;
  flex-wrap: nowrap;
}

.server-picker__selection {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  padding: 2px 0 4px;
}

.server-picker__selection-label {
  font-size: 12px;
  color: var(--ordo-text-secondary);
  flex: 0 0 auto;
}

.server-picker__selection-value {
  font-size: 13px;
  color: var(--ordo-text-primary);
  font-weight: 600;
  word-break: break-word;
}

.server-picker__hint {
  font-size: 12px;
  color: var(--ordo-text-secondary);
  padding-bottom: 4px;
}

.server-picker__empty {
  padding: 18px;
  border: 1px dashed var(--ordo-border-color);
  border-radius: 12px;
  color: var(--ordo-text-tertiary);
  font-size: 13px;
}

.server-grid {
  max-height: 420px;
  overflow-y: auto;
  border: 1px solid var(--ordo-border-color);
  border-radius: 14px;
  background: var(--ordo-bg-panel);
}

.server-grid::-webkit-scrollbar {
  width: 8px;
}

.server-grid::-webkit-scrollbar-thumb {
  background: var(--td-component-stroke);
  border-radius: 999px;
}

.server-grid::-webkit-scrollbar-track {
  background: transparent;
}

.server-table {
  width: 100%;
  border-collapse: collapse;
  table-layout: fixed;
}

.server-table th,
.server-table td {
  padding: 12px 16px;
  border-bottom: 1px solid var(--ordo-border-color);
  vertical-align: middle;
  text-align: left;
}

.server-table th {
  position: sticky;
  top: 0;
  z-index: 1;
  background: #fafbfc;
  font-size: 11px;
  font-weight: 600;
  color: var(--ordo-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.server-row {
  cursor: pointer;
}

.server-row:hover {
  background: #f8fafc;
}

.server-row--selected {
  background: #eef5ff;
}

.server-row--disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.server-row--disabled:hover {
  background: transparent;
}

.server-table tr:last-child td {
  border-bottom: none;
}

.col-check {
  width: 52px;
}

.server-cell-main {
  font-size: 15px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  word-break: break-word;
}

.server-cell-sub {
  font-size: 12px;
  color: var(--ordo-text-secondary);
  word-break: break-word;
  line-height: 1.5;
}

.server-warning {
  margin-top: -4px;
  margin-bottom: 12px;
  padding: 10px 12px;
  border-radius: 10px;
  background: var(--td-warning-color-1);
  color: var(--td-warning-color-8);
  font-size: 12px;
}

.server-warning--danger {
  background: var(--td-error-color-1);
  color: var(--td-error-color-7);
}

.field-label-with-help {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.field-help {
  width: 18px;
  height: 18px;
  border-radius: 999px;
  border: 1px solid var(--ordo-border-color);
  background: var(--ordo-bg-panel);
  color: var(--ordo-text-secondary);
  font-size: 11px;
  font-weight: 700;
  line-height: 1;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: help;
  padding: 0;
}

.field-help:hover {
  color: var(--td-brand-color);
  border-color: var(--td-brand-color);
}

.preview-col {
  position: sticky;
  top: 0;
}

.preview-card {
  background: var(--ordo-bg-panel);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-lg);
}

.preview-header {
  font-size: 11px;
  font-weight: 600;
  color: var(--ordo-text-tertiary, #9ca3af);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin-bottom: 10px;
}

.preview-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  margin-bottom: 16px;
}

.preview-list {
  display: grid;
  gap: 12px;
}

.preview-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.preview-item span {
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.preview-item strong {
  font-size: 13px;
  color: var(--ordo-text-primary);
  font-weight: 500;
  line-height: 1.5;
  word-break: break-word;
}
</style>
