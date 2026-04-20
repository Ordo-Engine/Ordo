<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { MessagePlugin } from 'tdesign-vue-next'
import { releaseApi } from '@/api/platform-client'
import type { ReleaseExecution, ReleaseRequest } from '@/api/types'
import { StudioPageHeader } from '@/components/ui'
import ReleaseNav from '@/components/project/ReleaseNav.vue'
import { labelRolloutStrategy } from '@/constants/release-center'
import { useAuthStore } from '@/stores/auth'
import { useRbacStore } from '@/stores/rbac'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const auth = useAuthStore()
const rbacStore = useRbacStore()

const loading = ref(false)
const reviewLoading = ref(false)
const executeLoading = ref(false)
const controlLoading = ref<'pause' | 'resume' | 'rollback' | null>(null)
const request = ref<ReleaseRequest | null>(null)
const execution = ref<ReleaseExecution | null>(null)
const elapsedSeconds = ref(0)
const activeTab = ref('overview')

let pollTimer: ReturnType<typeof setInterval> | null = null
let clockTimer: ReturnType<typeof setInterval> | null = null

const reviewDialog = ref<{ visible: boolean; mode: 'approve' | 'reject'; comment: string }>({
  visible: false, mode: 'approve', comment: '',
})

const canApprove = computed(() => rbacStore.can('release:request.approve'))
const canReject = computed(() => rbacStore.can('release:request.reject'))
const canExecute = computed(() => rbacStore.can('release:execute'))
const canPause = computed(() => rbacStore.can('release:pause'))
const canResume = computed(() => rbacStore.can('release:resume'))
const canRollback = computed(() => rbacStore.can('release:rollback'))
const canReview = computed(() =>
  !!request.value?.approvals.some(
    (a) => a.reviewer_id === auth.user?.id && a.decision === 'pending',
  ),
)

const isLiveExecution = computed(() =>
  ['preparing', 'waiting_start', 'rolling_out', 'paused']
    .includes(execution.value?.status ?? ''),
)

const hasExecution = computed(() => execution.value !== null)
const canPauseExecution = computed(() =>
  !!execution.value && ['preparing', 'waiting_start', 'rolling_out', 'verifying'].includes(execution.value.status),
)
const canResumeExecution = computed(() =>
  execution.value?.status === 'paused',
)
const canRollbackExecution = computed(() =>
  !!execution.value && ['completed', 'failed', 'paused'].includes(execution.value.status),
)

const executionTabDot = computed(() => {
  if (!execution.value) return null
  if (execution.value.status === 'completed') return 'success'
  if (['failed', 'rollback_in_progress'].includes(execution.value.status)) return 'danger'
  if (isLiveExecution.value) return 'live'
  return null
})

const approvedCount = computed(() =>
  request.value?.approvals.filter((a) => a.decision === 'approved').length ?? 0,
)

const approvalProgress = computed(() => {
  const total = request.value?.approvals.length ?? 0
  const min = request.value?.request_snapshot.min_approvals ?? 0
  return { approved: approvedCount.value, total, min }
})

const hasDiffChanges = computed(() => {
  const d = request.value?.content_diff
  if (!d) return false
  return (
    (d.added_steps?.length ?? 0) > 0 ||
    (d.modified_steps?.length ?? 0) > 0 ||
    (d.removed_steps?.length ?? 0) > 0 ||
    (d.added_groups?.length ?? 0) > 0 ||
    (d.modified_groups?.length ?? 0) > 0 ||
    (d.removed_groups?.length ?? 0) > 0 ||
    d.input_schema_changed ||
    d.output_schema_changed ||
    d.tags_changed ||
    d.description_changed
  )
})

function statusTheme(status: string) {
  if (['approved', 'completed'].includes(status)) return 'success'
  if (['pending_approval', 'executing'].includes(status)) return 'warning'
  if (['rejected', 'failed'].includes(status)) return 'danger'
  return 'default'
}

function instanceTheme(status: string) {
  if (status === 'success') return 'success'
  if (['failed', 'rolled_back'].includes(status)) return 'danger'
  if (['updating', 'dispatching', 'verifying'].includes(status)) return 'warning'
  return 'default'
}

function approvalDecisionTheme(decision: string) {
  if (decision === 'approved') return 'success'
  if (decision === 'rejected') return 'danger'
  return 'default'
}

function formatElapsed(s: number) {
  const m = Math.floor(s / 60)
  return m > 0 ? `${m}m ${s % 60}s` : `${s}s`
}

function formatDatetime(iso: string | undefined | null) {
  if (!iso) return '—'
  return new Date(iso).toLocaleString(undefined, {
    month: 'short', day: 'numeric', year: 'numeric',
    hour: '2-digit', minute: '2-digit',
  })
}

function startClock() {
  if (!execution.value?.started_at) return
  const t0 = new Date(execution.value.started_at).getTime()
  clockTimer = setInterval(() => { elapsedSeconds.value = Math.floor((Date.now() - t0) / 1000) }, 1000)
}

function stopClock() {
  if (clockTimer) { clearInterval(clockTimer); clockTimer = null }
}

function startPolling() {
  if (pollTimer) return
  pollTimer = setInterval(async () => {
    if (!auth.token || !request.value) return
    try {
      const ex = await releaseApi.getRequestExecution(
        auth.token, route.params.orgId as string,
        route.params.projectId as string, request.value.id,
      )
      execution.value = ex
      if (ex && !isLiveExecution.value) {
        stopPolling(); stopClock()
        if (ex.started_at)
          elapsedSeconds.value = Math.floor((Date.now() - new Date(ex.started_at).getTime()) / 1000)
        request.value = await releaseApi.getRequest(
          auth.token, route.params.orgId as string,
          route.params.projectId as string, request.value.id,
        )
      }
    } catch { /* silent */ }
  }, 2000)
}

function stopPolling() {
  if (pollTimer) { clearInterval(pollTimer); pollTimer = null }
}

onMounted(async () => {
  if (!auth.token) return
  loading.value = true
  try {
    await Promise.all([
      rbacStore.fetchRoles(route.params.orgId as string),
      rbacStore.fetchMyRoles(route.params.orgId as string),
    ])
    request.value = await releaseApi.getRequest(
      auth.token, route.params.orgId as string,
      route.params.projectId as string, route.params.releaseId as string,
    )
    const ex = await releaseApi.getRequestExecution(
      auth.token, route.params.orgId as string,
      route.params.projectId as string, route.params.releaseId as string,
    )
    execution.value = ex
    if (ex?.started_at)
      elapsedSeconds.value = Math.floor((Date.now() - new Date(ex.started_at).getTime()) / 1000)
    if (ex) { activeTab.value = 'execution' }
    if (isLiveExecution.value) { startClock(); startPolling() }
  } catch (e: any) {
    MessagePlugin.error(e.message || t('common.loadFailed'))
  } finally {
    loading.value = false
  }
})

onUnmounted(() => { stopPolling(); stopClock() })

function openReview(mode: 'approve' | 'reject') {
  reviewDialog.value = { visible: true, mode, comment: '' }
}

async function submitReview() {
  if (!auth.token || !request.value) return
  reviewLoading.value = true
  try {
    request.value = reviewDialog.value.mode === 'approve'
      ? await releaseApi.approveRequest(auth.token, route.params.orgId as string,
          route.params.projectId as string, request.value.id,
          { comment: reviewDialog.value.comment || undefined })
      : await releaseApi.rejectRequest(auth.token, route.params.orgId as string,
          route.params.projectId as string, request.value.id,
          { comment: reviewDialog.value.comment || undefined })
    reviewDialog.value.visible = false
    MessagePlugin.success(reviewDialog.value.mode === 'approve'
      ? t('releaseCenter.requestApproved') : t('releaseCenter.requestRejected'))
  } catch (e: any) {
    MessagePlugin.error(e.message || t('common.saveFailed'))
  } finally {
    reviewLoading.value = false
  }
}

async function executeRelease() {
  if (!auth.token || !request.value) return
  executeLoading.value = true
  try {
    const ex = await releaseApi.executeRequest(
      auth.token, route.params.orgId as string,
      route.params.projectId as string, request.value.id,
    )
    execution.value = ex
    elapsedSeconds.value = 0
    activeTab.value = 'execution'
    request.value = await releaseApi.getRequest(
      auth.token, route.params.orgId as string,
      route.params.projectId as string, request.value.id,
    )
    startClock(); startPolling()
    MessagePlugin.success(t('releaseCenter.executionStarted'))
  } catch (e: any) {
    MessagePlugin.error(e.message || t('releaseCenter.executionStartFailed'))
  } finally {
    executeLoading.value = false
  }
}

async function controlExecution(action: 'pause' | 'resume' | 'rollback') {
  if (!auth.token || !request.value) return
  controlLoading.value = action
  try {
    const ex = action === 'pause'
      ? await releaseApi.pauseExecution(auth.token, route.params.orgId as string, route.params.projectId as string, request.value.id)
      : action === 'resume'
        ? await releaseApi.resumeExecution(auth.token, route.params.orgId as string, route.params.projectId as string, request.value.id)
        : await releaseApi.rollbackExecution(auth.token, route.params.orgId as string, route.params.projectId as string, request.value.id)
    execution.value = ex
    request.value = await releaseApi.getRequest(
      auth.token, route.params.orgId as string,
      route.params.projectId as string, request.value.id,
    )
    if (action === 'pause') {
      stopClock()
      stopPolling()
    } else if (action === 'resume') {
      startClock()
      startPolling()
    } else if (action === 'rollback') {
      activeTab.value = 'execution'
      if (isLiveExecution.value) {
        startClock()
        startPolling()
      } else {
        stopClock()
        stopPolling()
      }
    }
    MessagePlugin.success(t(`releaseCenter.${action}ActionSuccess`))
  } catch (e: any) {
    MessagePlugin.error(e.message || t(`releaseCenter.${action}ActionFailed`))
  } finally {
    controlLoading.value = null
  }
}
</script>

<template>
  <div class="view-page">
    <StudioPageHeader
      :title="request?.title || t('releaseCenter.requestDetailTitle')"
      :subtitle="request ? `${request.ruleset_name} · ${request.environment_name} · v${request.version}` : t('releaseCenter.requestDetailSubtitle')"
    >
      <template #actions>
        <t-button variant="outline" @click="router.push({ name: 'project-release-requests', params: route.params })">
          {{ t('releaseCenter.backToRequests') }}
        </t-button>
        <template v-if="request?.status === 'pending_approval' && canReview">
          <t-button v-if="canReject" variant="outline" theme="danger" @click="openReview('reject')">
            {{ t('releaseCenter.rejectAction') }}
          </t-button>
          <t-button v-if="canApprove" theme="primary" @click="openReview('approve')">
            {{ t('releaseCenter.approveAction') }}
          </t-button>
        </template>
        <t-button
          v-if="request?.status === 'approved' && canExecute"
          theme="primary"
          :loading="executeLoading"
          @click="executeRelease"
        >
          {{ t('releaseCenter.executeAction') }}
        </t-button>
        <t-button
          v-if="canPause && canPauseExecution"
          variant="outline"
          :loading="controlLoading === 'pause'"
          @click="controlExecution('pause')"
        >
          {{ t('releaseCenter.pauseAction') }}
        </t-button>
        <t-button
          v-if="canResume && canResumeExecution"
          theme="primary"
          variant="outline"
          :loading="controlLoading === 'resume'"
          @click="controlExecution('resume')"
        >
          {{ t('releaseCenter.resumeAction') }}
        </t-button>
        <t-button
          v-if="canRollback && canRollbackExecution"
          variant="outline"
          theme="danger"
          :loading="controlLoading === 'rollback'"
          @click="controlExecution('rollback')"
        >
          {{ t('releaseCenter.rollbackAction') }}
        </t-button>
      </template>
    </StudioPageHeader>

    <ReleaseNav />

    <div v-if="loading" class="skeleton-list">
      <t-skeleton theme="paragraph" animation="gradient"
        :row-col="[{ width: '40%' }, { width: '100%' }, { width: '80%' }]" />
    </div>

    <template v-else-if="request">
      <!-- Status + version strip -->
      <t-card :bordered="false" class="strip-card">
        <div class="strip">
          <t-tag :theme="statusTheme(request.status)" variant="light">
            {{ t(`releaseCenter.statusMap.${request.status}`) }}
          </t-tag>
          <div class="strip-divider" />
          <div class="strip-item">
            <span>{{ t('releaseCenter.currentVersion') }}</span>
            <strong>{{ request.version_diff.from_version || 'Unreleased' }}</strong>
          </div>
          <span class="strip-arrow">→</span>
          <div class="strip-item">
            <span>{{ t('releaseCenter.targetVersion') }}</span>
            <strong>v{{ request.version_diff.to_version }}</strong>
          </div>
          <div class="strip-divider" />
          <div class="strip-item">
            <span>{{ t('releaseCenter.rollbackBaseline') }}</span>
            <strong>{{ request.version_diff.rollback_version || request.rollback_version || '—' }}</strong>
          </div>
          <div class="strip-item">
            <span>{{ t('releaseCenter.rolloutStrategy') }}</span>
            <strong>{{ labelRolloutStrategy(request.request_snapshot.rollout_strategy || request.rollout_strategy) }}</strong>
          </div>
          <div class="strip-divider" />
          <div class="strip-item">
            <span>{{ t('releaseCenter.createdAt') }}</span>
            <strong>{{ formatDatetime(request.created_at) }}</strong>
          </div>
        </div>
      </t-card>

      <!-- Tabs -->
      <t-tabs v-model="activeTab" class="detail-tabs">

        <!-- ── Overview ─────────────────────────────────────────────────── -->
        <t-tab-panel value="overview" :label="t('releaseCenter.tabOverview')">
          <div class="panel">

            <!-- Request info -->
            <t-card :bordered="false">
              <div class="card-section-title">{{ t('releaseCenter.requestInfo') }}</div>
              <div class="kv-grid kv-grid--3">
                <div class="kv">
                  <span>{{ t('releaseCenter.requesterLabel') }}</span>
                  <strong>{{ request.created_by_name || request.request_snapshot.requester_name || request.created_by }}</strong>
                </div>
                <div class="kv">
                  <span>{{ t('releaseCenter.requesterEmail') }}</span>
                  <strong>{{ request.created_by_email || request.request_snapshot.requester_email || '—' }}</strong>
                </div>
                <div class="kv">
                  <span>{{ t('releaseCenter.policyField') }}</span>
                  <strong>{{ request.request_snapshot.policy_name || '—' }}</strong>
                </div>
                <div class="kv">
                  <span>{{ t('releaseCenter.affectedInstances') }}</span>
                  <strong>{{ request.request_snapshot.affected_instance_count ?? request.affected_instance_count }}</strong>
                </div>
                <div class="kv">
                  <span>{{ t('releaseCenter.environmentField') }}</span>
                  <strong>{{ request.request_snapshot.environment_name || request.environment_name || '—' }}</strong>
                </div>
                <div class="kv">
                  <span>{{ t('releaseCenter.createdAt') }}</span>
                  <strong>{{ formatDatetime(request.created_at) }}</strong>
                </div>
              </div>
            </t-card>

            <!-- Change summary + release note -->
            <t-card v-if="request.change_summary || request.release_note" :bordered="false">
              <div class="text-sections">
                <div v-if="request.change_summary" class="text-block">
                  <div class="text-block-label">{{ t('releaseCenter.summaryField') }}</div>
                  <div class="text-block-body">{{ request.change_summary }}</div>
                </div>
                <div v-if="request.release_note" class="text-block" :class="{ 'text-block--bordered': request.change_summary }">
                  <div class="text-block-label">{{ t('publish.releaseNote') }}</div>
                  <div class="text-block-body">{{ request.release_note }}</div>
                </div>
              </div>
            </t-card>

            <!-- Rollout + rollback policy -->
            <div class="two-col">
              <t-card :bordered="false">
                <div class="card-section-title">{{ t('releaseCenter.rolloutStrategyDetails') }}</div>
                <div class="kv-grid kv-grid--2">
                  <div class="kv">
                    <span>{{ t('releaseCenter.rolloutStrategy') }}</span>
                    <strong>{{ labelRolloutStrategy(request.request_snapshot.rollout_strategy || request.rollout_strategy) }}</strong>
                  </div>
                  <div v-if="request.request_snapshot.rollout_strategy?.batch_size" class="kv">
                    <span>{{ t('releaseCenter.batchSizeField') }}</span>
                    <strong>{{ request.request_snapshot.rollout_strategy.batch_size }}</strong>
                  </div>
                  <div v-if="request.request_snapshot.rollout_strategy?.batch_interval_seconds" class="kv">
                    <span>{{ t('releaseCenter.batchIntervalField') }}</span>
                    <strong>{{ request.request_snapshot.rollout_strategy.batch_interval_seconds }}s</strong>
                  </div>
                  <div v-if="request.request_snapshot.rollout_strategy?.pause_on_error_rate" class="kv">
                    <span>{{ t('releaseCenter.pauseOnErrorRate') }}</span>
                    <strong>{{ (request.request_snapshot.rollout_strategy.pause_on_error_rate * 100).toFixed(0) }}%</strong>
                  </div>
                </div>
              </t-card>

              <t-card :bordered="false">
                <div class="card-section-title">{{ t('releaseCenter.rollbackPolicyDetails') }}</div>
                <div class="kv-grid kv-grid--2">
                  <div class="kv">
                    <span>{{ t('releaseCenter.autoRollback') }}</span>
                    <strong>{{ request.request_snapshot.rollback_policy?.auto_rollback ? t('common.enabled') : t('common.disabled') }}</strong>
                  </div>
                  <div class="kv">
                    <span>{{ t('releaseCenter.maxFailedInstancesField') }}</span>
                    <strong>{{ request.request_snapshot.rollback_policy?.max_failed_instances ?? '—' }}</strong>
                  </div>
                  <div v-if="request.request_snapshot.rollback_policy?.metric_guard" class="kv">
                    <span>{{ t('releaseCenter.metricGuard') }}</span>
                    <strong>{{ request.request_snapshot.rollback_policy.metric_guard }}</strong>
                  </div>
                  <div class="kv">
                    <span>{{ t('releaseCenter.rollbackBaseline') }}</span>
                    <strong>{{ request.rollback_version || '—' }}</strong>
                  </div>
                </div>
              </t-card>
            </div>

          </div>
        </t-tab-panel>

        <!-- ── Approval ─────────────────────────────────────────────────── -->
        <t-tab-panel value="approval" :label="t('releaseCenter.tabApproval')">
          <div class="panel">

            <!-- Progress summary -->
            <t-card :bordered="false" class="approval-summary-card">
              <div class="approval-summary">
                <div class="approval-progress-text">
                  <span class="approval-count">{{ approvalProgress.approved }}</span>
                  <span class="approval-sep">/</span>
                  <span class="approval-total">{{ approvalProgress.min || approvalProgress.total }}</span>
                  <span class="approval-label">{{ t('releaseCenter.approvalsRequired') }}</span>
                </div>
                <t-progress
                  :percentage="approvalProgress.min
                    ? Math.min(100, Math.round((approvalProgress.approved / approvalProgress.min) * 100))
                    : (approvalProgress.total > 0 ? Math.round((approvalProgress.approved / approvalProgress.total) * 100) : 0)"
                  :status="request.status === 'approved' || request.status === 'completed' ? 'success'
                    : request.status === 'rejected' ? 'error' : undefined"
                  theme="line"
                  style="flex:1; min-width: 160px;"
                />
                <div class="approval-meta-pills">
                  <span class="meta-pill">
                    {{ t('releaseCenter.minApprovals') }}: <strong>{{ request.request_snapshot.min_approvals ?? approvalProgress.total }}</strong>
                  </span>
                  <span class="meta-pill">
                    {{ t('releaseCenter.policyField') }}: <strong>{{ request.request_snapshot.policy_name || '—' }}</strong>
                  </span>
                </div>
              </div>
            </t-card>

            <!-- My pending action -->
            <t-card v-if="canReview && request.status === 'pending_approval'" :bordered="false" class="my-action-card">
              <div class="my-action">
                <div class="my-action-text">
                  <div class="my-action-title">{{ t('releaseCenter.awaitingYourReview') }}</div>
                  <div class="my-action-sub">{{ t('releaseCenter.awaitingYourReviewSub') }}</div>
                </div>
                <div class="my-action-btns">
                  <t-button v-if="canReject" variant="outline" theme="danger" @click="openReview('reject')">
                    {{ t('releaseCenter.rejectAction') }}
                  </t-button>
                  <t-button v-if="canApprove" theme="primary" @click="openReview('approve')">
                    {{ t('releaseCenter.approveAction') }}
                  </t-button>
                </div>
              </div>
            </t-card>

            <!-- Approval chain -->
            <t-card :bordered="false">
              <div class="card-section-title">{{ t('releaseCenter.approvalChain') }}</div>
              <div class="approval-chain">
                <div
                  v-for="approval in request.approvals"
                  :key="approval.id"
                  class="approval-row"
                  :class="{
                    'approval-row--mine': approval.reviewer_id === auth.user?.id,
                    'approval-row--pending': approval.decision === 'pending',
                  }"
                >
                  <div class="approval-stage-badge">
                    <span>{{ approval.stage }}</span>
                  </div>
                  <div class="approval-body">
                    <div class="approval-head">
                      <div class="approval-who">
                        <strong>{{ approval.reviewer_name }}</strong>
                        <span v-if="approval.reviewer_email">{{ approval.reviewer_email }}</span>
                        <t-tag v-if="approval.reviewer_id === auth.user?.id" size="small" variant="light" theme="primary">
                          {{ t('releaseCenter.you') }}
                        </t-tag>
                      </div>
                      <div class="approval-right">
                        <t-tag size="small" variant="light" :theme="approvalDecisionTheme(approval.decision)">
                          {{ t(`releaseCenter.approvalMap.${approval.decision}`) }}
                        </t-tag>
                        <span v-if="approval.decided_at" class="approval-time">{{ formatDatetime(approval.decided_at) }}</span>
                      </div>
                    </div>
                    <div v-if="approval.comment" class="approval-comment">
                      <t-icon name="chat" size="12px" style="flex-shrink:0; margin-top:1px;" />
                      <span>{{ approval.comment }}</span>
                    </div>
                  </div>
                </div>
              </div>
            </t-card>

          </div>
        </t-tab-panel>

        <!-- ── Diff ────────────────────────────────────────────────────── -->
        <t-tab-panel value="diff" :label="t('releaseCenter.tabDiff')">
          <div class="panel">

            <!-- Diff header: baseline + counts + schema flags -->
            <t-card :bordered="false">
              <div class="diff-header">
                <div class="diff-stat-row">
                  <div class="diff-stat">
                    <span>{{ t('releaseCenter.diffBaselineVersion') }}</span>
                    <strong>{{ request.content_diff.baseline_version || request.version_diff.from_version || 'Unreleased' }}</strong>
                  </div>
                  <div class="diff-stat-sep" />
                  <div class="diff-stat">
                    <span>{{ t('releaseCenter.diffStepsBefore') }}</span>
                    <strong>{{ request.content_diff.step_count_before }}</strong>
                  </div>
                  <div class="diff-stat-arrow">→</div>
                  <div class="diff-stat">
                    <span>{{ t('releaseCenter.diffStepsAfter') }}</span>
                    <strong :class="{ 'c-changed': request.content_diff.step_count_after !== request.content_diff.step_count_before }">
                      {{ request.content_diff.step_count_after }}
                    </strong>
                  </div>
                  <div class="diff-stat-sep" />
                  <div class="diff-stat">
                    <span>{{ t('releaseCenter.diffGroupsBefore') }}</span>
                    <strong>{{ request.content_diff.group_count_before }}</strong>
                  </div>
                  <div class="diff-stat-arrow">→</div>
                  <div class="diff-stat">
                    <span>{{ t('releaseCenter.diffGroupsAfter') }}</span>
                    <strong :class="{ 'c-changed': request.content_diff.group_count_after !== request.content_diff.group_count_before }">
                      {{ request.content_diff.group_count_after }}
                    </strong>
                  </div>
                </div>
                <div v-if="request.content_diff.input_schema_changed || request.content_diff.output_schema_changed || request.content_diff.tags_changed || request.content_diff.description_changed" class="diff-flags">
                  <t-tag v-if="request.content_diff.input_schema_changed" theme="warning" variant="light" size="small">{{ t('releaseCenter.diffInputSchemaChanged') }}</t-tag>
                  <t-tag v-if="request.content_diff.output_schema_changed" theme="warning" variant="light" size="small">{{ t('releaseCenter.diffOutputSchemaChanged') }}</t-tag>
                  <t-tag v-if="request.content_diff.tags_changed" theme="primary" variant="light" size="small">{{ t('releaseCenter.diffTagsChanged') }}</t-tag>
                  <t-tag v-if="request.content_diff.description_changed" theme="primary" variant="light" size="small">{{ t('releaseCenter.diffDescriptionChanged') }}</t-tag>
                </div>
              </div>
            </t-card>

            <!-- No changes -->
            <t-card v-if="!hasDiffChanges" :bordered="false">
              <p class="diff-empty-center">{{ t('releaseCenter.diffNothingChanged') }}</p>
            </t-card>

            <!-- Step changes -->
            <template v-if="(request.content_diff.added_steps?.length ?? 0) > 0 || (request.content_diff.modified_steps?.length ?? 0) > 0 || (request.content_diff.removed_steps?.length ?? 0) > 0">
              <div class="diff-columns">
                <t-card :bordered="false">
                  <div class="diff-col-head added">
                    <span class="diff-col-sign">+</span>
                    <span>{{ t('releaseCenter.diffAddedSteps') }}</span>
                    <span class="diff-col-count">{{ request.content_diff.added_steps?.length ?? 0 }}</span>
                  </div>
                  <div v-if="request.content_diff.added_steps?.length" class="diff-item-list">
                    <div v-for="item in request.content_diff.added_steps" :key="item.id" class="diff-item diff-item--added">
                      <strong>{{ item.name }}</strong>
                      <span v-if="item.step_type">{{ item.step_type }}</span>
                    </div>
                  </div>
                  <p v-else class="diff-empty">{{ t('releaseCenter.diffEmpty') }}</p>
                </t-card>

                <t-card :bordered="false">
                  <div class="diff-col-head modified">
                    <span class="diff-col-sign">~</span>
                    <span>{{ t('releaseCenter.diffModifiedSteps') }}</span>
                    <span class="diff-col-count">{{ request.content_diff.modified_steps?.length ?? 0 }}</span>
                  </div>
                  <div v-if="request.content_diff.modified_steps?.length" class="diff-item-list">
                    <div v-for="item in request.content_diff.modified_steps" :key="item.id" class="diff-item diff-item--modified">
                      <strong>{{ item.name }}</strong>
                      <span v-if="item.step_type">{{ item.step_type }}</span>
                    </div>
                  </div>
                  <p v-else class="diff-empty">{{ t('releaseCenter.diffEmpty') }}</p>
                </t-card>

                <t-card :bordered="false">
                  <div class="diff-col-head removed">
                    <span class="diff-col-sign">-</span>
                    <span>{{ t('releaseCenter.diffRemovedSteps') }}</span>
                    <span class="diff-col-count">{{ request.content_diff.removed_steps?.length ?? 0 }}</span>
                  </div>
                  <div v-if="request.content_diff.removed_steps?.length" class="diff-item-list">
                    <div v-for="item in request.content_diff.removed_steps" :key="item.id" class="diff-item diff-item--removed">
                      <strong>{{ item.name }}</strong>
                      <span v-if="item.step_type">{{ item.step_type }}</span>
                    </div>
                  </div>
                  <p v-else class="diff-empty">{{ t('releaseCenter.diffEmpty') }}</p>
                </t-card>
              </div>
            </template>

            <!-- Group changes -->
            <template v-if="(request.content_diff.added_groups?.length ?? 0) > 0 || (request.content_diff.modified_groups?.length ?? 0) > 0 || (request.content_diff.removed_groups?.length ?? 0) > 0">
              <div class="diff-columns">
                <t-card :bordered="false">
                  <div class="diff-col-head added">
                    <span class="diff-col-sign">+</span>
                    <span>{{ t('releaseCenter.diffAddedGroups') }}</span>
                    <span class="diff-col-count">{{ request.content_diff.added_groups?.length ?? 0 }}</span>
                  </div>
                  <div v-if="request.content_diff.added_groups?.length" class="diff-item-list">
                    <div v-for="name in request.content_diff.added_groups" :key="name" class="diff-item diff-item--added">
                      <strong>{{ name }}</strong>
                    </div>
                  </div>
                  <p v-else class="diff-empty">{{ t('releaseCenter.diffEmpty') }}</p>
                </t-card>

                <t-card :bordered="false">
                  <div class="diff-col-head modified">
                    <span class="diff-col-sign">~</span>
                    <span>{{ t('releaseCenter.diffModifiedGroups') }}</span>
                    <span class="diff-col-count">{{ request.content_diff.modified_groups?.length ?? 0 }}</span>
                  </div>
                  <div v-if="request.content_diff.modified_groups?.length" class="diff-item-list">
                    <div v-for="name in request.content_diff.modified_groups" :key="name" class="diff-item diff-item--modified">
                      <strong>{{ name }}</strong>
                    </div>
                  </div>
                  <p v-else class="diff-empty">{{ t('releaseCenter.diffEmpty') }}</p>
                </t-card>

                <t-card :bordered="false">
                  <div class="diff-col-head removed">
                    <span class="diff-col-sign">-</span>
                    <span>{{ t('releaseCenter.diffRemovedGroups') }}</span>
                    <span class="diff-col-count">{{ request.content_diff.removed_groups?.length ?? 0 }}</span>
                  </div>
                  <div v-if="request.content_diff.removed_groups?.length" class="diff-item-list">
                    <div v-for="name in request.content_diff.removed_groups" :key="name" class="diff-item diff-item--removed">
                      <strong>{{ name }}</strong>
                    </div>
                  </div>
                  <p v-else class="diff-empty">{{ t('releaseCenter.diffEmpty') }}</p>
                </t-card>
              </div>
            </template>

          </div>
        </t-tab-panel>

        <!-- ── Execution ─────────────────────────────────────────────── -->
        <t-tab-panel value="execution" :disabled="!hasExecution">
          <template #label>
            <span class="exec-tab-label">
              {{ t('releaseCenter.tabExecution') }}
              <span v-if="executionTabDot" class="exec-dot" :class="`exec-dot--${executionTabDot}`" />
            </span>
          </template>

          <div class="panel">
            <template v-if="execution">
              <t-card v-if="execution.status === 'completed'" :bordered="false" class="banner-card banner-card--success">
                ✓ {{ t('releaseCenter.executionCompleted') }}
              </t-card>
              <t-card v-else-if="execution.status === 'failed'" :bordered="false" class="banner-card banner-card--danger">
                ✗ {{ t('releaseCenter.executionFailed') }}
              </t-card>
              <t-card v-else-if="execution.status === 'rollback_in_progress'" :bordered="false" class="banner-card banner-card--warning">
                ↩ {{ t('releaseCenter.executionRolledBack') }}
              </t-card>

              <t-card :bordered="false">
                <div class="exec-stats">
                  <div class="exec-stat">
                    <span>{{ t('releaseCenter.status') }}</span>
                    <t-tag :theme="instanceTheme(execution.status)" variant="light" size="small">
                      {{ t(`releaseCenter.executionStatusMap.${execution.status}`) }}
                      <span v-if="isLiveExecution" class="live-dot" />
                    </t-tag>
                  </div>
                  <div class="exec-stat">
                    <span>{{ t('releaseCenter.elapsedTime') }}</span>
                    <strong>{{ formatElapsed(elapsedSeconds) }}</strong>
                  </div>
                  <div class="exec-stat">
                    <span>{{ t('releaseCenter.batchLabel') }}</span>
                    <strong>{{ execution.current_batch }} / {{ execution.total_batches }}</strong>
                  </div>
                  <div class="exec-stat">
                    <span>{{ t('releaseCenter.succeeded') }}</span>
                    <strong class="c-success">{{ execution.summary?.succeeded_instances ?? 0 }}</strong>
                  </div>
                  <div class="exec-stat">
                    <span>{{ t('releaseCenter.pending') }}</span>
                    <strong>{{ execution.summary?.pending_instances ?? 0 }}</strong>
                  </div>
                  <div class="exec-stat">
                    <span>{{ t('releaseCenter.failed') }}</span>
                    <strong class="c-danger">{{ execution.summary?.failed_instances ?? 0 }}</strong>
                  </div>
                  <div class="exec-stat">
                    <span>{{ t('releaseCenter.startedAt') }}</span>
                    <strong>{{ formatDatetime(execution.started_at) }}</strong>
                  </div>
                </div>
                <t-progress
                  v-if="execution.total_batches > 0"
                  :percentage="Math.round((execution.current_batch / execution.total_batches) * 100)"
                  :status="execution.status === 'completed' ? 'success' : execution.status === 'failed' ? 'error' : 'active'"
                  style="margin-top: 16px;"
                />
              </t-card>

              <t-card v-if="execution.instances.length" :bordered="false">
                <table class="instance-table">
                  <thead>
                    <tr>
                      <th>{{ t('releaseCenter.instanceName') }}</th>
                      <th>{{ t('releaseCenter.zone') }}</th>
                      <th>{{ t('releaseCenter.currentVersion') }}</th>
                      <th>{{ t('releaseCenter.targetVersion') }}</th>
                      <th>{{ t('releaseCenter.status') }}</th>
                      <th>{{ t('releaseCenter.message') }}</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr v-for="inst in execution.instances" :key="inst.id">
                      <td class="td-name">{{ inst.instance_name }}</td>
                      <td class="td-secondary">{{ inst.zone || '—' }}</td>
                      <td class="td-mono">{{ inst.current_version || '—' }}</td>
                      <td class="td-mono">{{ inst.target_version }}</td>
                      <td>
                        <t-tag :theme="instanceTheme(inst.status)" variant="light" size="small">
                          {{ t(`releaseCenter.instanceStatusMap.${inst.status}`) }}
                        </t-tag>
                      </td>
                      <td class="td-secondary td-msg">{{ inst.message || '—' }}</td>
                    </tr>
                  </tbody>
                </table>
              </t-card>
            </template>
            <t-empty v-else :title="t('releaseCenter.executionEmpty')" />
          </div>
        </t-tab-panel>

      </t-tabs>
    </template>

    <t-empty v-else-if="!loading" :title="t('releaseCenter.requestNotFound')" />

    <t-dialog
      v-model:visible="reviewDialog.visible"
      :header="reviewDialog.mode === 'approve' ? t('releaseCenter.approveAction') : t('releaseCenter.rejectAction')"
      :footer="false"
      width="520px"
    >
      <t-form label-align="top" :colon="false">
        <t-form-item :label="t('releaseCenter.reviewComment')">
          <t-textarea v-model="reviewDialog.comment" :autosize="{ minRows: 3, maxRows: 5 }" />
        </t-form-item>
        <div v-if="request" class="review-preview">
          <div class="review-preview__title">{{ request.title }}</div>
          <div class="review-preview__meta">{{ request.ruleset_name }} · {{ request.environment_name }} · v{{ request.version }}</div>
        </div>
      </t-form>
      <div class="dialog-actions">
        <t-button variant="outline" @click="reviewDialog.visible = false">{{ t('common.cancel') }}</t-button>
        <t-button
          :theme="reviewDialog.mode === 'approve' ? 'primary' : 'danger'"
          :loading="reviewLoading"
          @click="submitReview"
        >
          {{ reviewDialog.mode === 'approve' ? t('releaseCenter.approveAction') : t('releaseCenter.rejectAction') }}
        </t-button>
      </div>
    </t-dialog>
  </div>
</template>

<style scoped>
.view-page {
  padding: 24px 32px 32px;
  height: 100%;
  overflow-y: auto;
}

.skeleton-list { display: grid; gap: 12px; }

/* ── strip ─────────────────────────────────────────────────────────────── */
.strip-card :deep(.t-card__body) { padding: 12px 16px; }
.strip-card { margin-bottom: 4px; }
.strip { display: flex; align-items: center; gap: 16px; flex-wrap: wrap; }
.strip-divider { width: 1px; height: 28px; background: var(--ordo-border-color); flex-shrink: 0; }
.strip-item { display: flex; flex-direction: column; gap: 2px; }
.strip-item span { font-size: 11px; color: var(--ordo-text-secondary); }
.strip-item strong { font-size: 13px; color: var(--ordo-text-primary); font-family: monospace; font-weight: 500; }
.strip-arrow { color: var(--ordo-text-tertiary); font-weight: 700; }

/* ── tabs ──────────────────────────────────────────────────────────────── */
.detail-tabs { margin-top: 16px; }
.panel { display: flex; flex-direction: column; gap: 12px; padding-top: 16px; }

/* ── shared ────────────────────────────────────────────────────────────── */
.card-section-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--ordo-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  margin-bottom: 14px;
}

.kv-grid { display: grid; gap: 16px; }
.kv-grid--2 { grid-template-columns: repeat(2, minmax(0, 1fr)); }
.kv-grid--3 { grid-template-columns: repeat(3, minmax(0, 1fr)); }
.kv { display: flex; flex-direction: column; gap: 4px; }
.kv span { font-size: 12px; color: var(--ordo-text-secondary); }
.kv strong { font-size: 14px; color: var(--ordo-text-primary); font-weight: 500; }

.two-col { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }

/* ── text blocks ───────────────────────────────────────────────────────── */
.text-sections { display: flex; flex-direction: column; gap: 16px; }
.text-block { display: flex; flex-direction: column; gap: 8px; }
.text-block--bordered { padding-top: 16px; border-top: 1px solid var(--ordo-border-color); }
.text-block-label { font-size: 12px; color: var(--ordo-text-secondary); font-weight: 500; }
.text-block-body { font-size: 14px; color: var(--ordo-text-primary); line-height: 1.75; }

/* ── approval ──────────────────────────────────────────────────────────── */
.approval-summary-card :deep(.t-card__body) { padding: 16px 20px; }
.approval-summary {
  display: flex;
  align-items: center;
  gap: 20px;
  flex-wrap: wrap;
}
.approval-progress-text {
  display: flex;
  align-items: baseline;
  gap: 4px;
  flex-shrink: 0;
}
.approval-count { font-size: 28px; font-weight: 700; color: var(--ordo-text-primary); font-variant-numeric: tabular-nums; }
.approval-sep { font-size: 20px; color: var(--ordo-text-tertiary); }
.approval-total { font-size: 20px; color: var(--ordo-text-secondary); }
.approval-label { font-size: 12px; color: var(--ordo-text-secondary); margin-left: 6px; }
.approval-meta-pills { display: flex; flex-wrap: wrap; gap: 8px; }
.meta-pill {
  font-size: 12px;
  color: var(--ordo-text-secondary);
  background: var(--ordo-hover-bg);
  padding: 4px 10px;
  border-radius: 999px;
}
.meta-pill strong { color: var(--ordo-text-primary); font-weight: 500; }

.my-action-card :deep(.t-card__body) { background: rgba(0, 105, 219, 0.04); padding: 14px 16px; }
.my-action {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  flex-wrap: wrap;
}
.my-action-title { font-size: 14px; font-weight: 600; color: var(--ordo-text-primary); }
.my-action-sub { font-size: 12px; color: var(--ordo-text-secondary); margin-top: 2px; }
.my-action-btns { display: flex; gap: 8px; flex-shrink: 0; }

.approval-chain { display: flex; flex-direction: column; gap: 0; }
.approval-row {
  display: flex;
  align-items: flex-start;
  gap: 14px;
  padding: 14px 0;
  border-bottom: 1px solid var(--ordo-border-color);
}
.approval-row:last-child { border-bottom: none; padding-bottom: 0; }
.approval-row--mine { background: rgba(0, 105, 219, 0.03); margin: 0 -16px; padding: 14px 16px; border-radius: 8px; }

.approval-stage-badge {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  background: var(--ordo-hover-bg);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: 600;
  color: var(--ordo-text-secondary);
  flex-shrink: 0;
  margin-top: 2px;
}

.approval-body { flex: 1; display: flex; flex-direction: column; gap: 6px; min-width: 0; }
.approval-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; flex-wrap: wrap; }
.approval-who { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.approval-who strong { font-size: 13px; color: var(--ordo-text-primary); }
.approval-who span { font-size: 12px; color: var(--ordo-text-secondary); }
.approval-right { display: flex; align-items: center; gap: 10px; flex-shrink: 0; }
.approval-time { font-size: 12px; color: var(--ordo-text-tertiary); white-space: nowrap; }
.approval-comment {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
  font-style: italic;
  padding: 8px 10px;
  background: var(--ordo-hover-bg);
  border-radius: 8px;
}

/* ── diff ──────────────────────────────────────────────────────────────── */
.diff-header { display: flex; flex-direction: column; gap: 12px; }
.diff-stat-row { display: flex; align-items: center; gap: 14px; flex-wrap: wrap; }
.diff-stat { display: flex; flex-direction: column; gap: 3px; }
.diff-stat span { font-size: 11px; color: var(--ordo-text-secondary); }
.diff-stat strong { font-size: 14px; color: var(--ordo-text-primary); font-weight: 500; font-family: monospace; }
.diff-stat-sep { width: 1px; height: 28px; background: var(--ordo-border-color); flex-shrink: 0; }
.diff-stat-arrow { color: var(--ordo-text-tertiary); font-weight: 600; font-size: 14px; }
.c-changed { color: #0069DB; }

.diff-flags { display: flex; gap: 6px; flex-wrap: wrap; }

.diff-empty-center { text-align: center; font-size: 13px; color: var(--ordo-text-tertiary); margin: 8px 0; }

.diff-columns { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; }

.diff-col-head {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 12px;
  font-size: 12px;
  font-weight: 600;
}
.diff-col-head.added   { color: #00A870; }
.diff-col-head.modified { color: #9E7400; }
.diff-col-head.removed { color: #DC503C; }
.diff-col-sign { font-size: 14px; font-weight: 700; }
.diff-col-count {
  margin-left: auto;
  font-size: 11px;
  padding: 1px 7px;
  border-radius: 999px;
  background: var(--ordo-hover-bg);
  color: var(--ordo-text-secondary);
  font-weight: 500;
}

.diff-item-list { display: flex; flex-direction: column; gap: 5px; }
.diff-item {
  display: flex; flex-direction: column; gap: 2px;
  padding: 8px 10px; border-radius: 8px;
  border-left: 3px solid transparent;
}
.diff-item--added   { background: rgba(0, 168, 112, 0.06); border-left-color: #00A870; }
.diff-item--modified { background: rgba(158, 116, 0, 0.06); border-left-color: #9E7400; }
.diff-item--removed { background: rgba(220, 80, 60, 0.06); border-left-color: #DC503C; }
.diff-item strong { font-size: 13px; color: var(--ordo-text-primary); font-weight: 500; }
.diff-item span { font-size: 11px; color: var(--ordo-text-secondary); font-family: monospace; }
.diff-empty { font-size: 12px; color: var(--ordo-text-tertiary); margin: 0; }

/* ── exec tab label ─────────────────────────────────────────────────────── */
.exec-tab-label { display: flex; align-items: center; gap: 6px; }
.exec-dot { width: 7px; height: 7px; border-radius: 50%; display: inline-block; }
.exec-dot--success { background: #00A870; }
.exec-dot--danger  { background: #DC503C; }
.exec-dot--live    { background: #EBA700; animation: blink 1.4s ease-in-out infinite; }
@keyframes blink { 0%, 100% { opacity: 1; } 50% { opacity: 0.25; } }

/* ── banner cards ───────────────────────────────────────────────────────── */
.banner-card :deep(.t-card__body) { padding: 12px 16px; font-size: 13px; font-weight: 500; }
.banner-card--success :deep(.t-card__body) { color: #00A870; background: rgba(0,168,112,0.08); }
.banner-card--danger  :deep(.t-card__body) { color: #DC503C; background: rgba(220,80,60,0.08); }
.banner-card--warning :deep(.t-card__body) { color: #9E7400; background: rgba(235,167,0,0.08); }

/* ── exec stats ─────────────────────────────────────────────────────────── */
.exec-stats { display: flex; gap: 28px; flex-wrap: wrap; }
.exec-stat { display: flex; flex-direction: column; gap: 4px; }
.exec-stat span { font-size: 12px; color: var(--ordo-text-secondary); }
.exec-stat strong { font-size: 20px; font-weight: 600; color: var(--ordo-text-primary); font-variant-numeric: tabular-nums; }
.c-success { color: #00A870; }
.c-danger  { color: #DC503C; }
.live-dot {
  display: inline-block; width: 6px; height: 6px; border-radius: 50%;
  background: currentColor; margin-left: 4px; vertical-align: middle;
  animation: blink 1.4s ease-in-out infinite;
}

/* ── instance table ─────────────────────────────────────────────────────── */
.instance-table { width: 100%; border-collapse: collapse; font-size: 13px; }
.instance-table th {
  text-align: left; font-size: 11px; font-weight: 600; text-transform: uppercase;
  letter-spacing: 0.04em; color: var(--ordo-text-secondary);
  padding: 0 12px 10px; border-bottom: 1px solid var(--ordo-border-color);
}
.instance-table td { padding: 10px 12px; border-bottom: 1px solid var(--ordo-border-color); vertical-align: middle; }
.instance-table tbody tr:last-child td { border-bottom: none; }
.td-name { font-weight: 500; color: var(--ordo-text-primary); }
.td-secondary { color: var(--ordo-text-secondary); }
.td-mono { font-family: monospace; font-size: 12px; color: var(--ordo-text-primary); }
.td-msg { max-width: 200px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

/* ── dialog ─────────────────────────────────────────────────────────────── */
.dialog-actions { display: flex; justify-content: flex-end; gap: 10px; padding-top: 12px; }
.review-preview {
  margin-top: 10px;
  padding: 10px 14px;
  border-radius: 10px;
  background: var(--ordo-hover-bg);
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.review-preview__title { font-size: 13px; font-weight: 600; color: var(--ordo-text-primary); }
.review-preview__meta { font-size: 12px; color: var(--ordo-text-secondary); }

@media (max-width: 980px) {
  .view-page { padding: 16px 20px 32px; }
  .kv-grid--3 { grid-template-columns: repeat(2, 1fr); }
  .diff-columns { grid-template-columns: 1fr; }
  .two-col { grid-template-columns: 1fr; }
  .strip { gap: 12px; }
  .approval-summary { flex-direction: column; align-items: flex-start; }
}
</style>
