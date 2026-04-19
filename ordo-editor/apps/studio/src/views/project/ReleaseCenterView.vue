<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { MessagePlugin } from 'tdesign-vue-next'
import { releaseApi } from '@/api/platform-client'
import type { ReleaseExecution, ReleasePolicy, ReleaseRequest } from '@/api/types'
import { StudioPageHeader } from '@/components/ui'
import ReleaseNav from '@/components/project/ReleaseNav.vue'
import { labelRolloutStrategy } from '@/constants/release-center'
import { useAuthStore } from '@/stores/auth'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const auth = useAuthStore()
const requests = ref<ReleaseRequest[]>([])
const policies = ref<ReleasePolicy[]>([])
const currentExecution = ref<ReleaseExecution | null>(null)

const pendingRequests = computed(() => requests.value.filter((item) => item.status === 'pending_approval').length)
const activeExecutions = computed(() => requests.value.filter((item) => item.status === 'executing').length)
const policyCount = computed(() => policies.value.length)
const recentRequests = computed(() => [...requests.value].slice(0, 5))

const isLiveExecution = computed(() =>
  ['preparing', 'waiting_start', 'rolling_out', 'paused', 'verifying']
    .includes(currentExecution.value?.status ?? ''),
)

let pollTimer: ReturnType<typeof setInterval> | null = null

function startPolling() {
  if (pollTimer || !isLiveExecution.value) return
  pollTimer = setInterval(async () => {
    if (!auth.token) return
    try {
      const [requestData, executionData] = await Promise.all([
        releaseApi.listRequests(auth.token, route.params.orgId as string, route.params.projectId as string),
        releaseApi.getCurrentExecution(auth.token, route.params.orgId as string, route.params.projectId as string),
      ])
      requests.value = requestData
      currentExecution.value = executionData
      if (!isLiveExecution.value) {
        clearInterval(pollTimer!)
        pollTimer = null
      }
    } catch { /* silent */ }
  }, 5000)
}

onUnmounted(() => { if (pollTimer) clearInterval(pollTimer) })

function requestStatusTheme(status: string) {
  if (status === 'completed') return 'success'
  if (status === 'pending_approval' || status === 'executing') return 'warning'
  if (status === 'rejected' || status === 'failed') return 'danger'
  return 'default'
}

function goToDetail(id: string) {
  router.push({ name: 'project-release-request-detail', params: { ...route.params, releaseId: id } })
}

onMounted(async () => {
  if (!auth.token) return
  try {
    const [policyData, requestData, executionData] = await Promise.all([
      releaseApi.listPolicies(auth.token, route.params.orgId as string, route.params.projectId as string),
      releaseApi.listRequests(auth.token, route.params.orgId as string, route.params.projectId as string),
      releaseApi.getCurrentExecution(auth.token, route.params.orgId as string, route.params.projectId as string),
    ])
    policies.value = policyData
    requests.value = requestData
    currentExecution.value = executionData
    startPolling()
  } catch (e: any) {
    MessagePlugin.error(e.message || t('common.loadFailed'))
  }
})
</script>

<template>
  <div class="view-page">
    <StudioPageHeader :title="t('releaseCenter.title')" :subtitle="t('releaseCenter.subtitle')" />
    <ReleaseNav />

    <div class="metric-grid">
      <t-card class="metric-card" :bordered="false">
        <div class="metric-label">{{ t('releaseCenter.pendingApprovals') }}</div>
        <div class="metric-value">{{ pendingRequests }}</div>
        <div class="metric-foot">{{ t('releaseCenter.pendingApprovalsHint') }}</div>
      </t-card>
      <t-card class="metric-card" :bordered="false">
        <div class="metric-label">{{ t('releaseCenter.activeExecutions') }}</div>
        <div class="metric-value">{{ activeExecutions }}</div>
        <div class="metric-foot">{{ t('releaseCenter.activeExecutionsHint') }}</div>
      </t-card>
      <t-card class="metric-card" :bordered="false">
        <div class="metric-label">{{ t('releaseCenter.policyCount') }}</div>
        <div class="metric-value">{{ policyCount }}</div>
        <div class="metric-foot">{{ t('releaseCenter.policyCountHint') }}</div>
      </t-card>
    </div>

    <div class="overview-grid">
      <t-card :bordered="false" class="overview-card">
        <template #header>{{ t('releaseCenter.currentExecution') }}</template>
        <div v-if="currentExecution" class="execution-header">
          <div>
            <div class="execution-title">{{ t('releaseCenter.batchProgress') }}</div>
            <div class="execution-subtitle">
              {{ currentExecution.current_batch }} / {{ currentExecution.total_batches }}
              · {{ labelRolloutStrategy(currentExecution.strategy) }}
            </div>
          </div>
          <t-tag theme="warning" variant="light">{{ t('releaseCenter.statusRollingOut') }}</t-tag>
        </div>
        <template v-if="currentExecution">
          <t-progress :percentage="Math.round((currentExecution.current_batch / currentExecution.total_batches) * 100)" />
          <div class="instance-summary">
            <div class="instance-pill success">
              <strong>{{ currentExecution.summary.succeeded_instances }}</strong>
              <span>{{ t('releaseCenter.succeeded') }}</span>
            </div>
            <div class="instance-pill warning">
              <strong>{{ currentExecution.summary.pending_instances }}</strong>
              <span>{{ t('releaseCenter.pending') }}</span>
            </div>
            <div class="instance-pill danger">
              <strong>{{ currentExecution.summary.failed_instances }}</strong>
              <span>{{ t('releaseCenter.failed') }}</span>
            </div>
          </div>
        </template>
        <t-empty v-else :title="t('releaseCenter.executionEmpty')" />
      </t-card>

      <t-card :bordered="false" class="overview-card">
        <template #header>{{ t('releaseCenter.recentRequestsTitle') }}</template>
        <div v-if="recentRequests.length" class="recent-list">
          <div
            v-for="req in recentRequests"
            :key="req.id"
            class="recent-item"
            @click="goToDetail(req.id)"
          >
            <div class="recent-item-main">
              <span class="recent-item-title">{{ req.title }}</span>
              <span class="recent-item-meta">{{ req.ruleset_name }} · v{{ req.version }}</span>
            </div>
            <t-tag :theme="requestStatusTheme(req.status)" variant="light" size="small">
              {{ t(`releaseCenter.statusMap.${req.status}`) }}
            </t-tag>
          </div>
        </div>
        <t-empty v-else :title="t('releaseCenter.requestEmpty')" />
      </t-card>
    </div>

    <t-card v-if="currentExecution" :bordered="false" class="table-card">
      <template #header>{{ t('releaseCenter.instancePreview') }}</template>
      <t-table
        row-key="id"
        size="small"
        :data="currentExecution.instances"
        :columns="[
          { colKey: 'instance_name', title: t('releaseCenter.instanceName') },
          { colKey: 'zone', title: t('releaseCenter.zone') },
          { colKey: 'current_version', title: t('releaseCenter.currentVersion') },
          { colKey: 'target_version', title: t('releaseCenter.targetVersion') },
          { colKey: 'status', title: t('releaseCenter.status') },
          { colKey: 'message', title: t('releaseCenter.message') },
          { colKey: 'metric_summary', title: t('releaseCenter.metrics') },
        ]"
      />
    </t-card>
  </div>
</template>

<style scoped>
.view-page {
  padding: 24px 32px 32px;
  height: 100%;
  overflow-y: auto;
}

.metric-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 16px;
}

.metric-card :deep(.t-card__body) {
  display: grid;
  gap: 6px;
}

.metric-label,
.metric-foot,
.execution-subtitle {
  color: var(--ordo-text-secondary);
  font-size: 12px;
}

.metric-value {
  font-size: 28px;
  font-weight: 700;
  color: var(--ordo-text-primary);
}

.overview-grid {
  display: grid;
  grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr);
  gap: 16px;
  margin-top: 16px;
}

.overview-card :deep(.t-card__body) {
  display: grid;
  gap: 14px;
}

.execution-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.execution-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.instance-summary {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}

.instance-pill {
  min-width: 110px;
  padding: 12px 14px;
  border-radius: 14px;
  display: grid;
  gap: 3px;
}

.instance-pill.success { background: rgba(0, 168, 112, 0.08); }
.instance-pill.warning { background: rgba(237, 108, 2, 0.08); }
.instance-pill.danger { background: rgba(214, 48, 49, 0.08); }

.instance-pill strong {
  font-size: 18px;
  color: var(--ordo-text-primary);
}

.instance-pill span {
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.recent-list {
  display: grid;
  gap: 6px;
}

.recent-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px;
  border-radius: 10px;
  cursor: pointer;
  transition: background 0.15s;
}

.recent-item:hover {
  background: var(--ordo-hover-bg);
}

.recent-item-main {
  display: grid;
  gap: 2px;
  min-width: 0;
}

.recent-item-title {
  font-size: 13px;
  font-weight: 500;
  color: var(--ordo-text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.recent-item-meta {
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.table-card {
  margin-top: 16px;
}

@media (max-width: 980px) {
  .view-page {
    padding: 20px;
  }

  .metric-grid,
  .overview-grid {
    grid-template-columns: 1fr;
  }
}
</style>
