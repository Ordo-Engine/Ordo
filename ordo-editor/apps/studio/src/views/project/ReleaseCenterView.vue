<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { MessagePlugin } from 'tdesign-vue-next';
import { releaseApi } from '@/api/platform-client';
import type {
  ReleaseExecution,
  ReleaseExecutionEvent,
  ReleasePolicy,
  ReleaseRequest,
} from '@/api/types';
import { StudioPageHeader } from '@/components/ui';
import ReleaseNav from '@/components/project/ReleaseNav.vue';
import { useRolloutStrategyLabel } from '@/constants/release-center';
import { useAuthStore } from '@/stores/auth';

const route = useRoute();
const router = useRouter();
const { t } = useI18n();
const labelRolloutStrategy = useRolloutStrategyLabel();
const auth = useAuthStore();
const requests = ref<ReleaseRequest[]>([]);
const policies = ref<ReleasePolicy[]>([]);
const currentExecution = ref<ReleaseExecution | null>(null);
const executionEvents = ref<ReleaseExecutionEvent[]>([]);
const showEventLog = ref(false);

const pendingRequests = computed(
  () => requests.value.filter((item) => item.status === 'pending_approval').length
);
const activeExecutions = computed(
  () => requests.value.filter((item) => item.status === 'executing').length
);
const policyCount = computed(() => policies.value.length);
const recentRequests = computed(() => [...requests.value].slice(0, 5));

const isLiveExecution = computed(() =>
  ['preparing', 'waiting_start', 'rolling_out', 'paused', 'verifying'].includes(
    currentExecution.value?.status ?? ''
  )
);

const failedInstances = computed(
  () => currentExecution.value?.instances.filter((i) => i.status === 'failed') ?? []
);

function formatDurationMs(ms: number) {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

let pollTimer: ReturnType<typeof setInterval> | null = null;

async function fetchExecutionEvents() {
  if (!auth.token || !currentExecution.value) return;
  const exec = currentExecution.value;
  // Find the associated release request
  const req = requests.value.find((r) => r.id === exec.request_id);
  if (!req) return;
  try {
    executionEvents.value = await releaseApi.getExecutionEvents(
      auth.token,
      route.params.orgId as string,
      route.params.projectId as string,
      req.id,
      exec.id
    );
  } catch {
    /* silent */
  }
}

function startPolling() {
  if (pollTimer || !isLiveExecution.value) return;
  const interval = isLiveExecution.value ? 2000 : 10000;
  pollTimer = setInterval(async () => {
    if (!auth.token) return;
    try {
      const [requestData, executionData] = await Promise.all([
        releaseApi.listRequests(
          auth.token,
          route.params.orgId as string,
          route.params.projectId as string
        ),
        releaseApi.getCurrentExecution(
          auth.token,
          route.params.orgId as string,
          route.params.projectId as string
        ),
      ]);
      requests.value = requestData;
      currentExecution.value = executionData;
      if (!isLiveExecution.value) {
        clearInterval(pollTimer!);
        pollTimer = null;
        fetchExecutionEvents();
      }
    } catch {
      /* silent */
    }
  }, interval);
}

onUnmounted(() => {
  if (pollTimer) clearInterval(pollTimer);
});

function requestStatusTheme(status: string) {
  if (status === 'completed') return 'success';
  if (status === 'pending_approval' || status === 'executing') return 'warning';
  if (status === 'rejected' || status === 'failed') return 'danger';
  return 'default';
}

function executionStatusTheme(status: string) {
  if (status === 'completed') return 'success';
  if (status === 'failed') return 'danger';
  if (status === 'paused') return 'default';
  if (status === 'rollback_in_progress') return 'warning';
  return 'warning';
}

function goToDetail(id: string) {
  router.push({
    name: 'project-release-request-detail',
    params: { ...route.params, releaseId: id },
  });
}

onMounted(async () => {
  if (!auth.token) return;
  try {
    const [policyData, requestData, executionData] = await Promise.all([
      releaseApi.listPolicies(
        auth.token,
        route.params.orgId as string,
        route.params.projectId as string
      ),
      releaseApi.listRequests(
        auth.token,
        route.params.orgId as string,
        route.params.projectId as string
      ),
      releaseApi.getCurrentExecution(
        auth.token,
        route.params.orgId as string,
        route.params.projectId as string
      ),
    ]);
    policies.value = policyData;
    requests.value = requestData;
    currentExecution.value = executionData;
    startPolling();
  } catch (e: any) {
    MessagePlugin.error(e.message || t('common.loadFailed'));
  }
});
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
              {{ currentExecution.current_batch }} / {{ currentExecution.total_batches }} ·
              {{ labelRolloutStrategy(currentExecution.strategy) }}
            </div>
          </div>
          <t-tag :theme="executionStatusTheme(currentExecution.status)" variant="light">
            {{
              t(
                `releaseCenter.executionStatusMap.${currentExecution.status}`,
                t('releaseCenter.statusRollingOut')
              )
            }}
          </t-tag>
        </div>
        <template v-if="currentExecution">
          <t-progress
            :percentage="
              Math.round((currentExecution.current_batch / currentExecution.total_batches) * 100)
            "
          />
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
      <template #header>
        <div class="table-card-header">
          <span>{{ t('releaseCenter.instancePreview') }}</span>
          <t-button
            variant="text"
            size="small"
            @click="
              showEventLog = !showEventLog;
              if (showEventLog) fetchExecutionEvents();
            "
          >
            {{
              showEventLog
                ? t('releaseCenter.monitoring.hideEventLog')
                : t('releaseCenter.monitoring.showEventLog')
            }}
          </t-button>
        </div>
      </template>

      <!-- Failure analysis -->
      <div v-if="failedInstances.length > 0" class="failure-analysis">
        <div class="failure-analysis__title">
          <t-icon name="error-circle" size="14px" />
          {{ t('releaseCenter.monitoring.failureAnalysis') }}
        </div>
        <div v-for="inst in failedInstances" :key="inst.id" class="failure-item">
          <strong>{{ inst.instance_name }}</strong>
          <span>{{ inst.message }}</span>
        </div>
      </div>

      <t-table
        row-key="id"
        size="small"
        :data="currentExecution.instances"
        :columns="[
          { colKey: 'instance_name', title: t('releaseCenter.instanceName'), width: 180 },
          { colKey: 'status', title: t('releaseCenter.status'), width: 110 },
          { colKey: 'batch', title: t('releaseCenter.monitoring.batchIndex'), width: 80 },
          { colKey: 'duration', title: t('releaseCenter.monitoring.duration'), width: 90 },
          { colKey: 'applied_at', title: t('releaseCenter.monitoring.appliedAt'), width: 160 },
          { colKey: 'message', title: t('releaseCenter.message') },
        ]"
      >
        <template #status="{ row }">
          <t-tag
            :theme="
              row.status === 'success'
                ? 'success'
                : row.status === 'failed'
                  ? 'danger'
                  : row.status === 'updating'
                    ? 'warning'
                    : 'default'
            "
            variant="light"
            size="small"
          >
            {{ row.status }}
          </t-tag>
        </template>
        <template #batch="{ row }">
          <span v-if="row.metric_summary?.batch_index">
            {{ row.metric_summary.batch_index }}/{{
              row.metric_summary.total_batches ?? currentExecution?.total_batches
            }}
          </span>
          <span v-else>—</span>
        </template>
        <template #duration="{ row }">
          <span v-if="row.metric_summary?.duration_ms">
            {{ formatDurationMs(row.metric_summary.duration_ms) }}
          </span>
          <span v-else>—</span>
        </template>
        <template #applied_at="{ row }">
          <span v-if="row.metric_summary?.applied_at">
            {{ new Date(row.metric_summary.applied_at).toLocaleTimeString() }}
          </span>
          <span v-else>—</span>
        </template>
      </t-table>

      <!-- Event log -->
      <div v-if="showEventLog" class="event-log">
        <div class="event-log__title">{{ t('releaseCenter.monitoring.eventLog') }}</div>
        <div v-if="executionEvents.length === 0" class="event-log__empty">
          {{ t('releaseCenter.monitoring.noEvents') }}
        </div>
        <div v-for="ev in executionEvents" :key="ev.id" class="event-log__item">
          <span class="event-log__time">{{ new Date(ev.created_at).toLocaleTimeString() }}</span>
          <span class="event-log__type">{{ ev.event_type }}</span>
          <span v-if="ev.instance_id" class="event-log__instance">{{
            ev.instance_id.slice(0, 8)
          }}</span>
        </div>
      </div>
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

.instance-pill.success {
  background: rgba(0, 168, 112, 0.08);
}
.instance-pill.warning {
  background: rgba(237, 108, 2, 0.08);
}
.instance-pill.danger {
  background: rgba(214, 48, 49, 0.08);
}

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

.table-card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
}

.failure-analysis {
  padding: 12px 16px;
  border-radius: 8px;
  background: color-mix(in srgb, #ff4d4f 6%, var(--ordo-bg-card));
  border: 1px solid color-mix(in srgb, #ff4d4f 25%, transparent);
  margin-bottom: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.failure-analysis__title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 600;
  color: #ff4d4f;
}

.failure-item {
  display: flex;
  gap: 12px;
  font-size: 13px;
}

.failure-item strong {
  flex-shrink: 0;
  font-weight: 600;
}

.failure-item span {
  color: var(--ordo-text-secondary);
}

.event-log {
  margin-top: 16px;
  border-top: 1px solid var(--ordo-border);
  padding-top: 12px;
}

.event-log__title {
  font-size: 12px;
  font-weight: 600;
  color: var(--ordo-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin-bottom: 8px;
}

.event-log__empty {
  font-size: 13px;
  color: var(--ordo-text-tertiary);
}

.event-log__item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 4px 0;
  font-size: 12px;
  font-family: monospace;
  border-bottom: 1px solid var(--ordo-border);
}

.event-log__time {
  color: var(--ordo-text-tertiary);
  flex-shrink: 0;
}

.event-log__type {
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.event-log__instance {
  color: var(--ordo-text-secondary);
  font-size: 11px;
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
