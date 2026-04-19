<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { MessagePlugin } from 'tdesign-vue-next'
import { releaseApi } from '@/api/platform-client'
import type { ReleaseRequest } from '@/api/types'
import { StudioPageHeader } from '@/components/ui'
import ReleaseNav from '@/components/project/ReleaseNav.vue'
import { labelRolloutStrategy } from '@/constants/release-center'
import { useAuthStore } from '@/stores/auth'
import { useEnvironmentStore } from '@/stores/environment'
import { useProjectStore } from '@/stores/project'
import { useRbacStore } from '@/stores/rbac'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const auth = useAuthStore()
const envStore = useEnvironmentStore()
const projectStore = useProjectStore()
const rbacStore = useRbacStore()
const statusFilter = ref<'all' | 'pending_approval' | 'executing' | 'approved' | 'rejected'>('all')
const requests = ref<ReleaseRequest[]>([])
const loading = ref(false)
const policies = ref<any[]>([])
const creating = ref(false)
const showCreateDialog = ref(false)
const reviewLoading = ref(false)
const reviewDialog = ref<{ visible: boolean; mode: 'approve' | 'reject'; request: ReleaseRequest | null; comment: string }>({
  visible: false,
  mode: 'approve',
  request: null,
  comment: '',
})
const createForm = ref({
  ruleset_name: '',
  version: '',
  environment_id: '',
  policy_id: '',
  title: '',
  change_summary: '',
  release_note: '',
  rollback_version: '',
  affected_instance_count: 0,
})

const filteredRequests = computed(() =>
  statusFilter.value === 'all'
    ? requests.value
    : requests.value.filter((item) => item.status === statusFilter.value),
)

const canApprove = computed(() => rbacStore.can('release:request.approve'))
const canReject = computed(() => rbacStore.can('release:request.reject'))
const canManagePolicies = computed(() => rbacStore.can('release:policy.manage'))
const ruleOptions = computed(() => projectStore.rulesets)
const releaseRuleOptions = computed(() =>
  ruleOptions.value.map((rule) => ({
    label: rule.name,
    value: rule.name,
    version: rule.version,
  })),
)
const environmentOptions = computed(() =>
  envStore.environments.map((env) => ({
    label: env.name,
    value: env.id,
  })),
)
const policyOptions = computed(() =>
  policies.value.map((policy) => ({
    label: policy.name,
    value: policy.id,
    target_id: policy.target_id,
  })),
)
const selectedRule = computed(() =>
  ruleOptions.value.find((item) => item.name === createForm.value.ruleset_name) ?? null,
)
const selectedPolicy = computed(() =>
  policies.value.find((item) => item.id === createForm.value.policy_id)
  ?? policies.value.find((item) => item.target_id === createForm.value.environment_id)
  ?? null,
)
const selectedEnvironment = computed(() =>
  envStore.environments.find((item) => item.id === createForm.value.environment_id) ?? null,
)
const defaultEnvironmentId = computed(() =>
  envStore.environments.find((env) => env.is_default)?.id ?? envStore.environments[0]?.id ?? '',
)
const preferredPolicyId = computed(() =>
  selectedPolicy.value?.id
  ?? policies.value.find((item) => item.target_id === createForm.value.environment_id)?.id
  ?? policies.value[0]?.id
  ?? '',
)
const hasPolicies = computed(() => policies.value.length > 0)

function formatDate(iso: string) {
  return new Date(iso).toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' })
}

function syncCreateFormSelections() {
  const defaultEnvId = defaultEnvironmentId.value
  if (!createForm.value.environment_id || !environmentOptions.value.some((item) => item.value === createForm.value.environment_id)) {
    createForm.value.environment_id = defaultEnvId
  }

  if (createForm.value.environment_id) {
    const matchedPolicyId = policies.value.find((item) => item.target_id === createForm.value.environment_id)?.id ?? ''
    const fallbackPolicyId = matchedPolicyId || policies.value[0]?.id || ''
    if (!createForm.value.policy_id || !policyOptions.value.some((item) => item.value === createForm.value.policy_id)) {
      createForm.value.policy_id = fallbackPolicyId
    } else if (matchedPolicyId) {
      createForm.value.policy_id = matchedPolicyId
    }
  }
}

watch(
  () => createForm.value.ruleset_name,
  (name) => {
    const match = ruleOptions.value.find((item) => item.name === name)
    if (!match) return
    if (!createForm.value.version) createForm.value.version = match.version || '1.0.0'
    if (!createForm.value.title) createForm.value.title = `${match.name} ${t('releaseCenter.requestTitleSuffix')}`
    if (!createForm.value.rollback_version) createForm.value.rollback_version = match.version || '1.0.0'
  },
)

watch(
  () => [showCreateDialog.value, defaultEnvironmentId.value, createForm.value.environment_id] as const,
  ([visible, nextDefault, currentEnvironmentId]) => {
    if (!visible) return
    if (currentEnvironmentId) {
      const exists = envStore.environments.some((env) => env.id === currentEnvironmentId)
      if (exists) return
    }
    if (nextDefault) createForm.value.environment_id = nextDefault
  },
  { immediate: true },
)

watch(
  () => [showCreateDialog.value, createForm.value.environment_id, preferredPolicyId.value] as const,
  ([visible, environmentId, nextPolicyId]) => {
    if (!visible || !environmentId) return
    const currentPolicyExists = policies.value.some((policy) => policy.id === createForm.value.policy_id)
    const currentPolicyMatchesEnvironment = policies.value.some(
      (policy) => policy.id === createForm.value.policy_id && policy.target_id === environmentId,
    )
    if (currentPolicyExists && currentPolicyMatchesEnvironment) return
    createForm.value.policy_id = nextPolicyId
  },
  { immediate: true },
)

onMounted(async () => {
  if (!auth.token) return
  loading.value = true
  try {
    await Promise.all([
      projectStore.fetchRulesets(),
      envStore.fetchEnvironments(route.params.orgId as string, route.params.projectId as string),
      rbacStore.fetchRoles(route.params.orgId as string),
      rbacStore.fetchMyRoles(route.params.orgId as string),
      refreshRequests(),
      refreshPolicies(),
    ])
    const requestedRuleset = typeof route.query.ruleset === 'string' ? route.query.ruleset : ''
    if (requestedRuleset) {
      await openCreateDialog(requestedRuleset)
    }
  } catch (e: any) {
    MessagePlugin.error(e.message || t('common.loadFailed'))
  } finally {
    loading.value = false
  }
})

async function refreshRequests() {
  if (!auth.token) return
  requests.value = await releaseApi.listRequests(
    auth.token,
    route.params.orgId as string,
    route.params.projectId as string,
  )
}

async function refreshPolicies() {
  if (!auth.token) return
  policies.value = await releaseApi.listPolicies(
    auth.token,
    route.params.orgId as string,
    route.params.projectId as string,
  )
}

function tagTheme(status: string) {
  if (status === 'approved' || status === 'completed') return 'success'
  if (status === 'pending_approval' || status === 'executing') return 'warning'
  if (status === 'rejected') return 'danger'
  return 'default'
}

async function openCreateDialog(presetRulesetName = '') {
  if (!auth.token) return
  loading.value = true
  try {
    await Promise.all([
      envStore.fetchEnvironments(route.params.orgId as string, route.params.projectId as string),
      refreshPolicies(),
    ])
  } catch (e: any) {
    MessagePlugin.error(e.message || t('common.loadFailed'))
  } finally {
    loading.value = false
  }
  createForm.value = {
    ruleset_name: presetRulesetName || (typeof route.query.ruleset === 'string' ? route.query.ruleset : ''),
    version: '',
    environment_id: '',
    policy_id: '',
    title: '',
    change_summary: '',
    release_note: '',
    rollback_version: '',
    affected_instance_count: 0,
  }
  showCreateDialog.value = true
  syncCreateFormSelections()
  await nextTick()
  syncCreateFormSelections()
}

async function submitCreateRequest() {
  if (!auth.token) return
  if (
    !createForm.value.ruleset_name ||
    !createForm.value.version ||
    !createForm.value.environment_id ||
    !createForm.value.title ||
    !createForm.value.change_summary
  ) {
    MessagePlugin.warning(t('releaseCenter.formRequired'))
    return
  }
  creating.value = true
  try {
    const created = await releaseApi.createRequest(
      auth.token,
      route.params.orgId as string,
      route.params.projectId as string,
      {
        ruleset_name: createForm.value.ruleset_name,
        version: createForm.value.version,
        environment_id: createForm.value.environment_id,
        policy_id: createForm.value.policy_id || undefined,
        title: createForm.value.title,
        change_summary: createForm.value.change_summary,
        release_note: createForm.value.release_note || undefined,
        rollback_version: createForm.value.rollback_version || undefined,
        affected_instance_count: createForm.value.affected_instance_count || 0,
      },
    )
    requests.value.unshift(created)
    showCreateDialog.value = false
    MessagePlugin.success(t('releaseCenter.requestCreated'))
  } catch (e: any) {
    MessagePlugin.error(e.message || t('releaseCenter.requestCreateFailed'))
  } finally {
    creating.value = false
  }
}

function openReview(mode: 'approve' | 'reject', request: ReleaseRequest) {
  reviewDialog.value = { visible: true, mode, request, comment: '' }
}

async function submitReview() {
  if (!auth.token || !reviewDialog.value.request) return
  reviewLoading.value = true
  try {
    const req = reviewDialog.value.mode === 'approve'
      ? await releaseApi.approveRequest(
          auth.token,
          route.params.orgId as string,
          route.params.projectId as string,
          reviewDialog.value.request.id,
          { comment: reviewDialog.value.comment || undefined },
        )
      : await releaseApi.rejectRequest(
          auth.token,
          route.params.orgId as string,
          route.params.projectId as string,
          reviewDialog.value.request.id,
          { comment: reviewDialog.value.comment || undefined },
        )
    const idx = requests.value.findIndex((item) => item.id === req.id)
    if (idx !== -1) requests.value[idx] = req
    reviewDialog.value.visible = false
    MessagePlugin.success(
      reviewDialog.value.mode === 'approve'
        ? t('releaseCenter.requestApproved')
        : t('releaseCenter.requestRejected'),
    )
  } catch (e: any) {
    MessagePlugin.error(e.message || t('common.saveFailed'))
  } finally {
    reviewLoading.value = false
  }
}

function canReviewRequest(request: ReleaseRequest) {
  return request.approvals.some(
    (approval) => approval.reviewer_id === auth.user?.id && approval.decision === 'pending',
  )
}

function openRequestDetail(request: ReleaseRequest) {
  router.push({
    name: 'project-release-request-detail',
    params: {
      orgId: route.params.orgId,
      projectId: route.params.projectId,
      releaseId: request.id,
    },
  })
}

function goToPolicyCreation() {
  router.push({
    name: 'project-release-policies',
    params: {
      orgId: route.params.orgId,
      projectId: route.params.projectId,
    },
    query: { create: '1' },
  })
}
</script>

<template>
  <div class="view-page">
    <StudioPageHeader :title="t('releaseCenter.requestsTitle')" :subtitle="t('releaseCenter.requestsSubtitle')">
      <template #actions>
        <t-button theme="primary" @click="openCreateDialog">
          {{ t('releaseCenter.createRequest') }}
        </t-button>
      </template>
    </StudioPageHeader>
    <ReleaseNav />

    <div class="toolbar">
      <t-radio-group v-model="statusFilter" variant="default-filled">
        <t-radio-button value="all">{{ t('common.all') }}</t-radio-button>
        <t-radio-button value="pending_approval">{{ t('releaseCenter.statusPendingApproval') }}</t-radio-button>
        <t-radio-button value="executing">{{ t('releaseCenter.statusExecuting') }}</t-radio-button>
        <t-radio-button value="approved">{{ t('releaseCenter.statusApproved') }}</t-radio-button>
        <t-radio-button value="rejected">{{ t('releaseCenter.statusRejected') }}</t-radio-button>
      </t-radio-group>
    </div>

    <div v-if="loading" class="loading-state">
      <t-skeleton theme="paragraph" animation="gradient" :row-col="[{ width: '30%' }, { width: '96%' }, { width: '80%' }]" />
    </div>

    <div v-else-if="filteredRequests.length" class="request-list">
      <t-card
        v-for="request in filteredRequests"
        :key="request.id"
        :bordered="false"
        class="request-card"
        @click="openRequestDetail(request)"
      >
        <div class="req-row">
          <t-tag :theme="tagTheme(request.status)" variant="light" size="small" class="req-status">
            {{ t(`releaseCenter.statusMap.${request.status}`) }}
          </t-tag>

          <div class="req-body">
            <div class="req-title">{{ request.title }}</div>
            <div class="req-meta">
              <span>{{ request.ruleset_name }}</span>
              <span class="sep">·</span>
              <span>{{ request.environment_name }}</span>
              <span class="sep">·</span>
              <span>{{ request.version_diff?.from_version || 'Unreleased' }} → v{{ request.version }}</span>
              <span class="sep">·</span>
              <span>{{ request.created_by_name || request.created_by }}</span>
              <span class="sep">·</span>
              <span>{{ formatDate(request.created_at) }}</span>
            </div>
            <div v-if="request.change_summary" class="req-summary">{{ request.change_summary }}</div>
          </div>

          <div class="req-actions" @click.stop>
            <template v-if="request.status === 'pending_approval' && canReviewRequest(request)">
              <t-button v-if="canReject" variant="outline" theme="danger" size="small" @click="openReview('reject', request)">
                {{ t('releaseCenter.rejectAction') }}
              </t-button>
              <t-button v-if="canApprove" theme="primary" size="small" @click="openReview('approve', request)">
                {{ t('releaseCenter.approveAction') }}
              </t-button>
            </template>
          </div>
        </div>
      </t-card>
    </div>
    <div v-else class="state-center">
      <t-empty :title="t('releaseCenter.requestEmpty')" />
    </div>

    <!-- Create dialog -->
    <t-dialog v-model:visible="showCreateDialog" :header="t('releaseCenter.createRequest')" :footer="false" width="680px">
      <t-form label-align="top" :colon="false" class="dialog-form">
        <div class="dialog-grid">
          <t-form-item :label="t('releaseCenter.rulesetField')" required>
            <t-select v-model="createForm.ruleset_name" :options="releaseRuleOptions" />
          </t-form-item>
          <t-form-item :label="t('releaseCenter.versionField')" required>
            <t-input v-model="createForm.version" />
          </t-form-item>
        </div>

        <div class="dialog-grid">
          <t-form-item :label="t('releaseCenter.environmentField')" required>
            <t-select v-model="createForm.environment_id" :options="environmentOptions" />
          </t-form-item>
          <t-form-item :label="t('releaseCenter.policyField')">
            <t-select v-model="createForm.policy_id" clearable :options="policyOptions" />
          </t-form-item>
        </div>

        <div v-if="!hasPolicies" class="policy-quick-entry">
          <div>
            <div class="policy-quick-entry__title">{{ t('releaseCenter.noPolicyForRequestTitle') }}</div>
            <div class="policy-quick-entry__desc">{{ t('releaseCenter.noPolicyForRequestDesc') }}</div>
          </div>
          <t-button v-if="canManagePolicies" theme="primary" variant="outline" @click="goToPolicyCreation">
            {{ t('releaseCenter.createPolicy') }}
          </t-button>
        </div>

        <t-form-item :label="t('releaseCenter.titleField')" required>
          <t-input v-model="createForm.title" />
        </t-form-item>

        <t-form-item :label="t('releaseCenter.summaryField')" required>
          <t-textarea v-model="createForm.change_summary" :autosize="{ minRows: 3, maxRows: 5 }" />
        </t-form-item>

        <div class="dialog-grid">
          <t-form-item :label="t('releaseCenter.rollbackField')">
            <t-input v-model="createForm.rollback_version" :placeholder="selectedRule?.version ?? ''" />
          </t-form-item>
          <t-form-item :label="t('releaseCenter.affectedInstances')">
            <t-input-number v-model="createForm.affected_instance_count" theme="normal" :min="0" />
          </t-form-item>
        </div>

        <div class="dialog-grid">
          <t-form-item :label="t('publish.releaseNote')">
            <t-input v-model="createForm.release_note" />
          </t-form-item>
        </div>

        <div class="review-context review-context--preview">
          <div class="review-context__title">{{ createForm.title || t('releaseCenter.createRequest') }}</div>
          <div class="review-context__meta">
            <span>{{ createForm.ruleset_name || '—' }}</span>
            <span>·</span>
            <span>{{ selectedEnvironment?.name || '—' }}</span>
            <span>·</span>
            <span>{{ selectedRule?.version || 'Unreleased' }} → v{{ createForm.version || '—' }}</span>
          </div>
          <div class="preview-kv">
            <div class="kv-item">
              <span>{{ t('releaseCenter.rollbackBaseline') }}</span>
              <strong>{{ createForm.rollback_version || selectedRule?.version || '—' }}</strong>
            </div>
            <div class="kv-item">
              <span>{{ t('releaseCenter.affectedInstances') }}</span>
              <strong>{{ createForm.affected_instance_count }}</strong>
            </div>
            <div class="kv-item">
              <span>{{ t('releaseCenter.rolloutStrategy') }}</span>
              <strong>{{ selectedPolicy ? labelRolloutStrategy(selectedPolicy.rollout_strategy) : '—' }}</strong>
            </div>
            <div class="kv-item">
              <span>{{ t('releaseCenter.policyField') }}</span>
              <strong>{{ selectedPolicy?.name || '—' }}</strong>
            </div>
            <div class="kv-item">
              <span>{{ t('releaseCenter.requesterLabel') }}</span>
              <strong>{{ auth.user?.display_name || auth.user?.email || '—' }}</strong>
            </div>
            <div class="kv-item">
              <span>{{ t('releaseCenter.approverLabel') }}</span>
              <strong>{{ selectedPolicy?.approver_ids?.length ? selectedPolicy.approver_ids.join(', ') : '—' }}</strong>
            </div>
          </div>
        </div>
      </t-form>

      <div class="dialog-actions">
        <t-button variant="outline" @click="showCreateDialog = false">{{ t('common.cancel') }}</t-button>
        <t-button theme="primary" :loading="creating" @click="submitCreateRequest">
          {{ t('releaseCenter.createRequest') }}
        </t-button>
      </div>
    </t-dialog>

    <!-- Review dialog -->
    <t-dialog
      v-model:visible="reviewDialog.visible"
      :header="reviewDialog.mode === 'approve' ? t('releaseCenter.approveAction') : t('releaseCenter.rejectAction')"
      :footer="false"
      width="520px"
    >
      <t-form label-align="top" :colon="false" class="dialog-form">
        <t-form-item :label="t('releaseCenter.reviewComment')">
          <t-textarea v-model="reviewDialog.comment" :autosize="{ minRows: 3, maxRows: 5 }" />
        </t-form-item>

        <div v-if="reviewDialog.request" class="review-context">
          <div class="review-context__title">{{ reviewDialog.request.title }}</div>
          <div class="review-context__meta">
            <span>{{ reviewDialog.request.ruleset_name }}</span>
            <span>·</span>
            <span>{{ reviewDialog.request.environment_name }}</span>
            <span>·</span>
            <span>
              {{ reviewDialog.request.version_diff?.from_version || 'Unreleased' }} → v{{ reviewDialog.request.version_diff?.to_version || reviewDialog.request.version }}
            </span>
          </div>
          <div class="review-context__summary">{{ reviewDialog.request.change_summary }}</div>
          <div class="preview-kv preview-kv--3">
            <div class="kv-item">
              <span>{{ t('releaseCenter.requesterLabel') }}</span>
              <strong>
                {{
                  reviewDialog.request.created_by_name
                  || reviewDialog.request.request_snapshot?.requester_name
                  || reviewDialog.request.created_by
                }}
              </strong>
            </div>
            <div class="kv-item">
              <span>{{ t('releaseCenter.policyField') }}</span>
              <strong>{{ reviewDialog.request.request_snapshot?.policy_name || '—' }}</strong>
            </div>
            <div class="kv-item">
              <span>{{ t('releaseCenter.affectedInstances') }}</span>
              <strong>{{ reviewDialog.request.request_snapshot?.affected_instance_count ?? reviewDialog.request.affected_instance_count }}</strong>
            </div>
          </div>
          <div v-if="reviewDialog.request.release_note" class="review-context__summary review-context__note">
            {{ reviewDialog.request.release_note }}
          </div>
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

.toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
}

.request-list {
  display: grid;
  gap: 10px;
}

.loading-state {
  display: grid;
}

.request-card {
  cursor: pointer;
  transition: box-shadow 0.15s;
}

.request-card:hover {
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.08);
}

.req-row {
  display: flex;
  align-items: flex-start;
  gap: 14px;
}

.req-status {
  flex-shrink: 0;
  margin-top: 2px;
}

.req-body {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.req-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.req-meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.req-meta .sep {
  color: var(--ordo-text-tertiary);
}

.req-summary {
  font-size: 12px;
  color: var(--ordo-text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.req-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

/* dialog */
.dialog-form {
  padding-top: 4px;
}

.dialog-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}

.policy-quick-entry {
  margin-bottom: 12px;
  padding: 14px 16px;
  border: 1px solid var(--td-component-border);
  border-radius: 12px;
  background: var(--td-bg-color-container-hover);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.policy-quick-entry__title {
  font-size: 14px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.policy-quick-entry__desc {
  margin-top: 4px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.review-context {
  margin-top: 8px;
  padding: 12px 14px;
  border-radius: 12px;
  background: var(--ordo-hover-bg);
  display: grid;
  gap: 8px;
}

.review-context--preview {
  margin-top: 4px;
}

.review-context__title {
  font-size: 14px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.review-context__meta,
.review-context__summary {
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.review-context__meta {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.review-context__note {
  padding-top: 8px;
  border-top: 1px solid var(--ordo-border-color);
}

.preview-kv {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
  margin-top: 4px;
}

.preview-kv--3 {
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.kv-item {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.kv-item span {
  font-size: 11px;
  color: var(--ordo-text-secondary);
}

.kv-item strong {
  font-size: 13px;
  color: var(--ordo-text-primary);
  font-weight: 500;
}

.state-center {
  display: flex;
  justify-content: center;
  padding-top: 60px;
}

@media (max-width: 980px) {
  .view-page {
    padding: 20px;
  }

  .req-row {
    flex-direction: column;
    gap: 10px;
  }

  .req-actions {
    align-self: flex-end;
  }

  .preview-kv {
    grid-template-columns: repeat(2, 1fr);
  }

  .dialog-grid {
    grid-template-columns: 1fr;
  }
}
</style>
