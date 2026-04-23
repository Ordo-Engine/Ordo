<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { MessagePlugin } from 'tdesign-vue-next'
import { releaseApi } from '@/api/platform-client'
import type { ReleaseRequest } from '@/api/types'
import { StudioPageHeader } from '@/components/ui'
import ReleaseNav from '@/components/project/ReleaseNav.vue'
import { useAuthStore } from '@/stores/auth'
import { useRbacStore } from '@/stores/rbac'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const auth = useAuthStore()
const rbacStore = useRbacStore()
const statusFilter = ref<'all' | 'pending_approval' | 'executing' | 'approved' | 'rejected'>('all')
const requests = ref<ReleaseRequest[]>([])
const loading = ref(false)
const reviewLoading = ref(false)
const reviewDialog = ref<{ visible: boolean; mode: 'approve' | 'reject'; request: ReleaseRequest | null; comment: string }>({
  visible: false,
  mode: 'approve',
  request: null,
  comment: '',
})

const filteredRequests = computed(() =>
  statusFilter.value === 'all'
    ? requests.value
    : requests.value.filter((item) => item.status === statusFilter.value),
)

const canApprove = computed(() => rbacStore.can('release:request.approve'))
const canReject = computed(() => rbacStore.can('release:request.reject'))

function formatDate(iso: string) {
  return new Date(iso).toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' })
}

onMounted(async () => {
  if (!auth.token) return
  loading.value = true
  try {
    await Promise.all([
      rbacStore.fetchRoles(route.params.orgId as string),
      rbacStore.fetchMyRoles(route.params.orgId as string),
      refreshRequests(),
    ])
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

function tagTheme(status: string) {
  if (status === 'approved' || status === 'completed') return 'success'
  if (status === 'pending_approval' || status === 'executing') return 'warning'
  if (status === 'rejected') return 'danger'
  return 'default'
}

function openCreatePage() {
  router.push({
    name: 'project-release-request-create',
    params: {
      orgId: route.params.orgId,
      projectId: route.params.projectId,
    },
  })
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

</script>

<template>
  <div class="view-page">
    <StudioPageHeader :title="t('releaseCenter.requestsTitle')" :subtitle="t('releaseCenter.requestsSubtitle')">
      <template #actions>
        <t-button theme="primary" @click="openCreatePage">
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

.review-context {
  margin-top: 8px;
  padding: 12px 14px;
  border-radius: 12px;
  background: var(--ordo-hover-bg);
  display: grid;
  gap: 8px;
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
