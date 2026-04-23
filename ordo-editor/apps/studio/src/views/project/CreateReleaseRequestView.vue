<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { MessagePlugin } from 'tdesign-vue-next'
import type { ReleaseTargetPreview } from '@/api/types'
import { releaseApi } from '@/api/platform-client'
import { useAuthStore } from '@/stores/auth'
import { useEnvironmentStore } from '@/stores/environment'
import { useProjectStore } from '@/stores/project'
import { useRbacStore } from '@/stores/rbac'
import { useRolloutStrategyLabel } from '@/constants/release-center'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const labelRolloutStrategy = useRolloutStrategyLabel()
const auth = useAuthStore()
const envStore = useEnvironmentStore()
const projectStore = useProjectStore()
const rbacStore = useRbacStore()

const orgId = route.params.orgId as string
const projectId = route.params.projectId as string

const loading = ref(true)
const submitting = ref(false)
const policies = ref<any[]>([])
const previewLoading = ref(false)
const targetPreview = ref<ReleaseTargetPreview | null>(null)

const form = ref({
  ruleset_name: (route.query.ruleset as string) || '',
  version: '',
  environment_id: '',
  policy_id: '',
  title: '',
  change_summary: '',
  release_note: '',
  rollback_version: '',
  affected_instance_count: 0,
})

// ── Derived options ──────────────────────────────────────────────────────────

const ruleOptions = computed(() =>
  projectStore.rulesets.map((r) => ({ label: r.name, value: r.name, version: r.version })),
)
const environmentOptions = computed(() =>
  envStore.environments.map((e) => ({ label: e.name, value: e.id })),
)
const policyOptions = computed(() =>
  policies.value.map((p) => ({ label: p.name, value: p.id, target_id: p.target_id })),
)

const selectedRule = computed(() =>
  projectStore.rulesets.find((r) => r.name === form.value.ruleset_name) ?? null,
)
const selectedPublishedVersion = computed(() => selectedRule.value?.published_version ?? '')
const selectedEnvironment = computed(() =>
  envStore.environments.find((e) => e.id === form.value.environment_id) ?? null,
)
const selectedPolicy = computed(() =>
  policies.value.find((p) => p.id === form.value.policy_id) ?? null,
)
const canManagePolicies = computed(() => rbacStore.can('release:policy.manage'))
const hasPolicies = computed(() => policies.value.length > 0)

// ── Auto-fill logic ──────────────────────────────────────────────────────────

watch(
  () => form.value.ruleset_name,
  (name) => {
    const match = projectStore.rulesets.find((r) => r.name === name)
    if (!match) {
      form.value.version = ''
      form.value.rollback_version = ''
      return
    }
    form.value.version = match.version || '1.0.0'
    form.value.rollback_version = match.published_version || ''
    if (!form.value.title) form.value.title = `${match.name} ${t('releaseCenter.requestTitleSuffix')}`
  },
  { immediate: true },
)

watch(
  () => [form.value.environment_id, policies.value.length] as const,
  ([envId]) => {
    if (!envId) return
    const matched = policies.value.find((p) => p.target_id === envId)
    if (matched && !form.value.policy_id) form.value.policy_id = matched.id
  },
)

watch(
  () => form.value.environment_id,
  async (environmentId) => {
    targetPreview.value = null
    form.value.affected_instance_count = 0
    if (!environmentId || !auth.token) return
    previewLoading.value = true
    try {
      const preview = await releaseApi.previewTarget(auth.token, orgId, projectId, environmentId)
      targetPreview.value = preview
      form.value.affected_instance_count = preview.affected_instance_count
    } catch (e: any) {
      MessagePlugin.error(e.message || t('common.loadFailed'))
    } finally {
      previewLoading.value = false
    }
  },
  { immediate: true },
)

// ── Lifecycle ────────────────────────────────────────────────────────────────

onMounted(async () => {
  try {
    await Promise.all([
      projectStore.fetchRulesets(),
      envStore.fetchEnvironments(orgId, projectId),
      rbacStore.fetchRoles(orgId),
      rbacStore.fetchMyRoles(orgId),
      refreshPolicies(),
    ])
    await nextTick()
    if (!form.value.ruleset_name && projectStore.rulesets.length > 0) {
      form.value.ruleset_name = projectStore.rulesets[0].name
    }
    // Set default environment
    if (!form.value.environment_id) {
      const def = envStore.environments.find((e) => e.is_default) ?? envStore.environments[0]
      if (def) form.value.environment_id = def.id
    }
    // Set default policy for environment
    if (form.value.environment_id && !form.value.policy_id) {
      const matched = policies.value.find((p) => p.target_id === form.value.environment_id)
      if (matched) form.value.policy_id = matched.id
    }
  } catch (e: any) {
    MessagePlugin.error(e.message || t('common.loadFailed'))
  } finally {
    loading.value = false
  }
})

async function refreshPolicies() {
  if (!auth.token) return
  policies.value = await releaseApi.listPolicies(auth.token, orgId, projectId)
}

// ── Submit ───────────────────────────────────────────────────────────────────

async function handleSubmit() {
  if (!auth.token) {
    MessagePlugin.error(t('auth.loginFailed'))
    return
  }
  if (
    !form.value.ruleset_name ||
    !form.value.version ||
    !form.value.environment_id ||
    !form.value.title ||
    !form.value.change_summary
  ) {
    MessagePlugin.warning(t('releaseCenter.formRequired'))
    return
  }
  submitting.value = true
  try {
    const created = await releaseApi.createRequest(auth.token, orgId, projectId, {
      ruleset_name: form.value.ruleset_name,
      version: form.value.version,
      environment_id: form.value.environment_id,
      policy_id: form.value.policy_id || undefined,
      title: form.value.title,
      change_summary: form.value.change_summary,
      release_note: form.value.release_note || undefined,
      rollback_version: form.value.rollback_version || undefined,
      affected_instance_count: targetPreview.value?.affected_instance_count ?? 0,
    })
    MessagePlugin.success(t('releaseCenter.requestCreated'))
    router.replace({
      name: 'project-release-request-detail',
      params: { orgId, projectId, releaseId: created.id },
    })
  } catch (e: any) {
    MessagePlugin.error(e.message || t('releaseCenter.requestCreateFailed'))
  } finally {
    submitting.value = false
  }
}

function handleCancel() {
  router.push({ name: 'project-release-requests', params: { orgId, projectId } })
}

function goToPolicies() {
  router.push({
    name: 'project-release-policies',
    params: { orgId, projectId },
    query: { create: '1' },
  })
}
</script>

<template>
  <div class="view-page">
    <!-- Header -->
    <div class="page-header">
      <t-button variant="text" @click="handleCancel">
        <t-icon name="chevron-left" />
        {{ t('releaseCenter.requestsTitle') }}
      </t-button>
      <h2 class="page-title">{{ t('releaseCenter.createRequest') }}</h2>
    </div>

    <div v-if="loading" class="loading-state">
      <t-skeleton theme="paragraph" animation="gradient" :row-col="[{}, {}, {}, {}]" />
    </div>

    <div v-else class="page-body">
      <!-- Left: form -->
      <div class="form-col">
        <t-form label-align="top" :colon="false">

          <div class="section-label">{{ t('releaseCenter.sectionTarget') }}</div>
          <div class="field-row">
            <t-form-item :label="t('releaseCenter.rulesetField')" required>
              <t-select
                v-model="form.ruleset_name"
                :options="ruleOptions"
                :placeholder="t('common.select')"
              />
            </t-form-item>
            <t-form-item :label="t('releaseCenter.versionField')" required>
              <t-input
                :model-value="form.version"
                :placeholder="selectedRule?.version ?? '1.0.0'"
                readonly
              />
            </t-form-item>
          </div>

          <div class="field-row">
            <t-form-item :label="t('releaseCenter.environmentField')" required>
              <t-select
                v-model="form.environment_id"
                :options="environmentOptions"
                :placeholder="t('common.select')"
              />
            </t-form-item>
            <t-form-item :label="t('releaseCenter.policyField')">
              <t-select
                v-model="form.policy_id"
                clearable
                :options="policyOptions"
                :placeholder="t('common.optional')"
              />
            </t-form-item>
          </div>

          <!-- No policy hint -->
          <div v-if="!hasPolicies" class="notice-row">
            <t-icon name="info-circle" class="notice-icon" />
            <span>{{ t('releaseCenter.noPolicyForRequestDesc') }}</span>
            <t-button
              v-if="canManagePolicies"
              size="small"
              variant="text"
              theme="primary"
              @click="goToPolicies"
            >
              {{ t('releaseCenter.createPolicy') }}
            </t-button>
          </div>

          <div class="section-label">{{ t('releaseCenter.sectionContent') }}</div>

          <t-form-item :label="t('releaseCenter.titleField')" required>
            <t-input v-model="form.title" />
          </t-form-item>

          <t-form-item :label="t('releaseCenter.summaryField')" required>
            <t-textarea
              v-model="form.change_summary"
              :autosize="{ minRows: 4, maxRows: 8 }"
              :placeholder="t('releaseCenter.summaryPlaceholder')"
            />
          </t-form-item>

          <t-form-item :label="t('publish.releaseNote')">
            <t-textarea
              v-model="form.release_note"
              :autosize="{ minRows: 2, maxRows: 4 }"
              :placeholder="t('releaseCenter.releaseNotePlaceholder')"
            />
          </t-form-item>

          <div class="section-label">{{ t('releaseCenter.sectionRollout') }}</div>
          <div class="field-row">
            <t-form-item :label="t('releaseCenter.rollbackField')">
              <t-input
                :model-value="form.rollback_version || '—'"
                :placeholder="t('releaseCenter.rollbackAuto')"
                readonly
              />
            </t-form-item>
            <t-form-item :label="t('releaseCenter.affectedInstances')">
              <t-input
                :model-value="String(targetPreview?.affected_instance_count ?? 0)"
                readonly
                :status="previewLoading ? 'warning' : 'default'"
              />
            </t-form-item>
          </div>

          <div class="notice-row notice-row--stacked">
            <div class="notice-main">
              <t-icon name="server" class="notice-icon" />
              <span>
                {{
                  targetPreview?.bound_servers?.length
                    ? `${targetPreview.bound_servers.length} ${t('releaseCenter.affectedInstances')} · ${targetPreview.bound_servers.map((server) => server.name).join(', ')}`
                    : (targetPreview?.message || t('releaseCenter.noBoundServer'))
                }}
              </span>
            </div>
            <div v-if="targetPreview?.bound_servers?.length" class="notice-sub">
              {{
                targetPreview.bound_servers
                  .map((server) => `${server.name} · ${t(`releaseCenter.serverStatusMap.${server.status}`)}`)
                  .join(' / ')
              }}
            </div>
          </div>

        </t-form>

        <!-- Actions -->
        <div class="form-actions">
          <t-button variant="outline" @click="handleCancel">{{ t('common.cancel') }}</t-button>
          <t-button theme="primary" :loading="submitting" @click="handleSubmit">
            {{ t('releaseCenter.submitRequest') }}
          </t-button>
        </div>
      </div>

      <!-- Right: preview -->
      <div class="preview-col">
        <div class="preview-card">
          <div class="preview-header">{{ t('releaseCenter.previewTitle') }}</div>

          <div class="preview-title">
            {{ form.title || t('releaseCenter.createRequest') }}
          </div>

          <div class="preview-meta">
            <span>{{ form.ruleset_name || '—' }}</span>
            <span class="sep">·</span>
            <span>{{ selectedEnvironment?.name || '—' }}</span>
            <span class="sep">·</span>
            <span>{{ selectedPublishedVersion || 'Unreleased' }} → v{{ form.version || '—' }}</span>
          </div>

          <p v-if="form.change_summary" class="preview-summary">{{ form.change_summary }}</p>

          <div class="preview-kv">
            <div class="kv-item">
              <span>{{ t('releaseCenter.rolloutStrategy') }}</span>
              <strong>{{ selectedPolicy ? labelRolloutStrategy(selectedPolicy.rollout_strategy) : '—' }}</strong>
            </div>
            <div class="kv-item">
              <span>{{ t('releaseCenter.policyField') }}</span>
              <strong>{{ selectedPolicy?.name || '—' }}</strong>
            </div>
            <div class="kv-item">
              <span>{{ t('releaseCenter.rollbackBaseline') }}</span>
              <strong>{{ form.rollback_version || selectedPublishedVersion || '—' }}</strong>
            </div>
            <div class="kv-item">
              <span>{{ t('releaseCenter.affectedInstances') }}</span>
              <strong>{{ targetPreview?.affected_instance_count ?? 0 }}</strong>
            </div>
            <div class="kv-item">
              <span>{{ t('releaseCenter.executionNode') }}</span>
              <strong>{{ targetPreview?.bound_servers?.map((server) => server.name).join(', ') || '—' }}</strong>
            </div>
            <div class="kv-item">
              <span>{{ t('releaseCenter.requesterLabel') }}</span>
              <strong>{{ auth.user?.display_name || auth.user?.email || '—' }}</strong>
            </div>
            <div class="kv-item">
              <span>{{ t('releaseCenter.approverLabel') }}</span>
              <strong>
                {{
                  selectedPolicy?.approver_ids?.length
                    ? `${selectedPolicy.approver_ids.length} ${t('releaseCenter.approvers')}`
                    : '—'
                }}
              </strong>
            </div>
          </div>

          <div v-if="form.release_note" class="preview-note">
            <div class="preview-note__label">{{ t('publish.releaseNote') }}</div>
            <div class="preview-note__text">{{ form.release_note }}</div>
          </div>
        </div>
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

/* Form column */
.form-col {
  display: flex;
  flex-direction: column;
  gap: 0;
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

.field-row {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
}

.notice-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  border-radius: 8px;
  background: var(--td-warning-color-light, #fff3cd);
  font-size: 13px;
  color: var(--ordo-text-secondary);
  margin-bottom: 12px;
}

.notice-row--stacked {
  align-items: flex-start;
  flex-direction: column;
  gap: 6px;
}

.notice-main {
  display: flex;
  align-items: center;
  gap: 8px;
}

.notice-sub {
  padding-left: 24px;
  font-size: 12px;
  color: var(--ordo-text-tertiary);
}

.notice-icon {
  color: var(--td-warning-color, #ed7b2f);
  flex-shrink: 0;
}

.form-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 28px;
  padding-top: 20px;
  border-top: 1px solid var(--ordo-border-color);
}

/* Preview column */
.preview-col {
  position: sticky;
  top: 0;
}

.preview-card {
  background: var(--ordo-bg-panel);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-lg);
  padding: 16px 18px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.preview-header {
  font-size: 11px;
  font-weight: 600;
  color: var(--ordo-text-tertiary, #9ca3af);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.preview-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  line-height: 1.4;
}

.preview-meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.sep {
  color: var(--ordo-text-tertiary);
}

.preview-summary {
  font-size: 12px;
  color: var(--ordo-text-secondary);
  margin: 0;
  padding-top: 8px;
  border-top: 1px solid var(--ordo-border-color);
  white-space: pre-wrap;
  word-break: break-word;
}

.preview-kv {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
  padding-top: 8px;
  border-top: 1px solid var(--ordo-border-color);
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

.preview-note {
  padding-top: 8px;
  border-top: 1px solid var(--ordo-border-color);
}

.preview-note__label {
  font-size: 11px;
  color: var(--ordo-text-secondary);
  margin-bottom: 4px;
}

.preview-note__text {
  font-size: 12px;
  color: var(--ordo-text-primary);
  white-space: pre-wrap;
  word-break: break-word;
}

@media (max-width: 960px) {
  .page-body {
    grid-template-columns: 1fr;
  }

  .preview-col {
    position: static;
    order: -1;
  }

  .field-row {
    grid-template-columns: 1fr;
  }
}
</style>
