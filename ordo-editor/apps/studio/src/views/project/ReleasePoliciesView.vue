<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { MessagePlugin } from 'tdesign-vue-next';
import { memberApi, releaseApi } from '@/api/platform-client';
import type { Member, ReleasePolicy } from '@/api/types';
import { StudioPageHeader } from '@/components/ui';
import ReleaseNav from '@/components/project/ReleaseNav.vue';
import { useRolloutStrategyLabel } from '@/constants/release-center';
import { useAuthStore } from '@/stores/auth';
import { useEnvironmentStore } from '@/stores/environment';

const route = useRoute();
const router = useRouter();
const { t } = useI18n();
const labelRolloutStrategy = useRolloutStrategyLabel();
const auth = useAuthStore();
const envStore = useEnvironmentStore();
const policies = ref<ReleasePolicy[]>([]);
const loading = ref(false);
const creating = ref(false);
const showCreateDialog = ref(false);
const members = ref<Member[]>([]);
const form = ref({
  name: '',
  scope: 'project' as 'org' | 'project',
  target_type: 'environment' as 'project' | 'environment',
  target_id: '',
  description: '',
  min_approvals: 1,
  allow_self_approval: true,
  approver_ids: [] as string[],
  batch_size: 1,
  batch_interval_seconds: 180,
  auto_rollback: true,
  max_failed_instances: 1,
  metric_guard: '',
});
const environmentOptions = computed(() =>
  envStore.environments.map((env) => ({
    label: env.name,
    value: env.id,
  }))
);
const memberOptions = computed(() =>
  members.value.map((member) => ({
    label: member.display_name || member.email,
    value: member.user_id,
  }))
);

function envName(id: string) {
  return envStore.environments.find((e) => e.id === id)?.name || id;
}

function approverNames(ids: string[]) {
  return ids
    .map((id) => {
      const m = members.value.find((m) => m.user_id === id);
      return m ? m.display_name || m.email : id;
    })
    .join(', ');
}

onMounted(async () => {
  if (!auth.token) return;
  loading.value = true;
  try {
    await Promise.all([
      refreshPolicies(),
      envStore.fetchEnvironments(route.params.orgId as string, route.params.projectId as string),
      loadMembers(),
    ]);
    if (route.query.create === '1') {
      openCreateDialog();
      const nextQuery = { ...route.query };
      delete nextQuery.create;
      router.replace({ query: nextQuery });
    }
  } catch (e: any) {
    MessagePlugin.error(e.message || t('common.loadFailed'));
  } finally {
    loading.value = false;
  }
});

async function refreshPolicies() {
  if (!auth.token) return;
  policies.value = await releaseApi.listPolicies(
    auth.token,
    route.params.orgId as string,
    route.params.projectId as string
  );
}

async function loadMembers() {
  if (!auth.token) return;
  members.value = await memberApi.list(auth.token, route.params.orgId as string);
}

function openCreateDialog() {
  form.value = {
    name: '',
    scope: 'project',
    target_type: 'environment',
    target_id: envStore.environments.find((env) => env.is_default)?.id ?? '',
    description: '',
    min_approvals: 1,
    allow_self_approval: true,
    approver_ids: [],
    batch_size: 1,
    batch_interval_seconds: 180,
    auto_rollback: true,
    max_failed_instances: 1,
    metric_guard: '',
  };
  showCreateDialog.value = true;
}

async function submitPolicy() {
  if (!auth.token) return;
  if (!form.value.name || !form.value.target_id || form.value.approver_ids.length === 0) {
    MessagePlugin.warning(t('releaseCenter.formRequired'));
    return;
  }
  creating.value = true;
  try {
    const created = await releaseApi.createPolicy(
      auth.token,
      route.params.orgId as string,
      route.params.projectId as string,
      {
        name: form.value.name,
        scope: form.value.scope,
        target_type: form.value.target_type,
        target_id: form.value.target_id,
        description: form.value.description || undefined,
        min_approvals: form.value.min_approvals,
        allow_self_approval: form.value.allow_self_approval,
        approver_ids: form.value.approver_ids,
        rollout_strategy: {
          kind: 'time_interval_batch',
          batch_size: form.value.batch_size,
          batch_interval_seconds: form.value.batch_interval_seconds,
          auto_rollback_on_failure: form.value.auto_rollback,
          pause_on_metric_breach: true,
        },
        rollback_policy: {
          auto_rollback: form.value.auto_rollback,
          max_failed_instances: form.value.max_failed_instances,
          metric_guard: form.value.metric_guard || undefined,
        },
      }
    );
    policies.value.unshift(created);
    showCreateDialog.value = false;
    MessagePlugin.success(t('releaseCenter.policyCreated'));
  } catch (e: any) {
    MessagePlugin.error(e.message || t('releaseCenter.policyCreateFailed'));
  } finally {
    creating.value = false;
  }
}
</script>

<template>
  <div class="view-page">
    <StudioPageHeader
      :title="$t('releaseCenter.policiesTitle')"
      :subtitle="$t('releaseCenter.policiesSubtitle')"
    >
      <template #actions>
        <t-button theme="primary" @click="openCreateDialog">{{
          $t('releaseCenter.createPolicy')
        }}</t-button>
      </template>
    </StudioPageHeader>
    <ReleaseNav />

    <div v-if="loading" class="loading-state">
      <t-skeleton
        theme="paragraph"
        animation="gradient"
        :row-col="[{ width: '36%' }, { width: '92%' }, { width: '70%' }]"
      />
    </div>

    <div v-else-if="policies.length" class="policy-list">
      <t-card v-for="policy in policies" :key="policy.id" :bordered="false" class="policy-card">
        <div class="policy-head">
          <div>
            <div class="policy-title">{{ policy.name }}</div>
            <div v-if="policy.description" class="policy-desc">{{ policy.description }}</div>
          </div>
          <t-tag variant="light" theme="primary">{{ envName(policy.target_id) }}</t-tag>
        </div>

        <div class="policy-kv">
          <div class="kv">
            <span>{{ $t('releaseCenter.approvers') }}</span>
            <strong>{{ approverNames(policy.approver_ids) }}</strong>
          </div>
          <div class="kv">
            <span>{{ $t('releaseCenter.minApprovals') }}</span>
            <strong>{{ policy.min_approvals }}</strong>
          </div>
          <div class="kv">
            <span>{{ $t('releaseCenter.rolloutStrategy') }}</span>
            <strong>{{ labelRolloutStrategy(policy.rollout_strategy) }}</strong>
          </div>
          <div class="kv">
            <span>{{ $t('releaseCenter.rollbackPolicy') }}</span>
            <strong>{{
              policy.rollback_policy.auto_rollback ? $t('common.enabled') : $t('common.disabled')
            }}</strong>
          </div>
        </div>

        <div class="policy-foot">
          <span v-if="policy.rollback_policy.metric_guard"
            >{{ $t('releaseCenter.metricGuard') }}: {{ policy.rollback_policy.metric_guard }}</span
          >
          <span
            >{{ $t('releaseCenter.updatedAt') }}:
            {{ new Date(policy.updated_at).toLocaleString() }}</span
          >
        </div>
      </t-card>
    </div>
    <div v-else class="state-center">
      <t-empty :title="$t('releaseCenter.policyEmpty')" />
    </div>

    <t-dialog
      v-model:visible="showCreateDialog"
      :header="$t('releaseCenter.createPolicy')"
      :footer="false"
      width="720px"
    >
      <t-form label-align="top" :colon="false" class="dialog-form">
        <div class="dialog-grid">
          <t-form-item required>
            <template #label>
              <div class="field-label">
                <span>{{ $t('releaseCenter.policyNameField') }}</span>
                <t-popup :content="$t('releaseCenter.policyNameHelp')" placement="top">
                  <button type="button" class="field-help" aria-label="Policy name help">?</button>
                </t-popup>
              </div>
            </template>
            <t-input v-model="form.name" />
          </t-form-item>
          <t-form-item required>
            <template #label>
              <div class="field-label">
                <span>{{ $t('releaseCenter.targetField') }}</span>
                <t-popup :content="$t('releaseCenter.targetFieldHelp')" placement="top">
                  <button type="button" class="field-help" aria-label="Target help">?</button>
                </t-popup>
              </div>
            </template>
            <t-select v-model="form.target_id" :options="environmentOptions" />
          </t-form-item>
        </div>

        <t-form-item :label="$t('rbac.roleDesc')">
          <t-input v-model="form.description" />
        </t-form-item>

        <div class="dialog-grid">
          <t-form-item required>
            <template #label>
              <div class="field-label">
                <span>{{ $t('releaseCenter.minApprovals') }}</span>
                <t-popup :content="$t('releaseCenter.minApprovalsHelp')" placement="top">
                  <button type="button" class="field-help" aria-label="Minimum approvals help">
                    ?
                  </button>
                </t-popup>
              </div>
            </template>
            <t-input-number v-model="form.min_approvals" :min="1" />
          </t-form-item>
          <t-form-item required>
            <template #label>
              <div class="field-label">
                <span>{{ $t('releaseCenter.approvers') }}</span>
                <t-popup :content="$t('releaseCenter.approversHelp')" placement="top">
                  <button type="button" class="field-help" aria-label="Approvers help">?</button>
                </t-popup>
              </div>
            </template>
            <t-select v-model="form.approver_ids" multiple :options="memberOptions" />
          </t-form-item>
        </div>

        <div class="dialog-grid">
          <t-form-item>
            <template #label>
              <div class="field-label">
                <span>{{ $t('releaseCenter.batchSizeField') }}</span>
                <t-popup :content="$t('releaseCenter.batchSizeHelp')" placement="top">
                  <button type="button" class="field-help" aria-label="Batch size help">?</button>
                </t-popup>
              </div>
            </template>
            <t-input-number v-model="form.batch_size" :min="1" />
          </t-form-item>
          <t-form-item>
            <template #label>
              <div class="field-label">
                <span>{{ $t('releaseCenter.batchIntervalField') }}</span>
                <t-popup :content="$t('releaseCenter.batchIntervalHelp')" placement="top">
                  <button type="button" class="field-help" aria-label="Batch interval help">
                    ?
                  </button>
                </t-popup>
              </div>
            </template>
            <t-input-number v-model="form.batch_interval_seconds" :min="30" />
          </t-form-item>
        </div>

        <div class="dialog-grid">
          <t-form-item>
            <template #label>
              <div class="field-label">
                <span>{{ $t('releaseCenter.maxFailedInstancesField') }}</span>
                <t-popup :content="$t('releaseCenter.maxFailedInstancesHelp')" placement="top">
                  <button type="button" class="field-help" aria-label="Max failed instances help">
                    ?
                  </button>
                </t-popup>
              </div>
            </template>
            <t-input-number v-model="form.max_failed_instances" :min="1" />
          </t-form-item>
          <t-form-item>
            <template #label>
              <div class="field-label">
                <span>{{ $t('releaseCenter.metricGuard') }}</span>
                <t-popup :content="$t('releaseCenter.metricGuardHelp')" placement="top">
                  <button type="button" class="field-help" aria-label="Metric guard help">?</button>
                </t-popup>
              </div>
            </template>
            <t-input v-model="form.metric_guard" />
          </t-form-item>
        </div>
      </t-form>

      <div class="dialog-actions">
        <t-button variant="outline" @click="showCreateDialog = false">{{
          $t('common.cancel')
        }}</t-button>
        <t-button theme="primary" :loading="creating" @click="submitPolicy">{{
          $t('releaseCenter.createPolicy')
        }}</t-button>
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

.policy-list {
  display: grid;
  gap: 14px;
}

.loading-state {
  display: grid;
}

.dialog-form {
  padding-top: 4px;
}

.dialog-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}

.field-label {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.field-help {
  width: 18px;
  height: 18px;
  border: 1px solid var(--td-component-border);
  border-radius: 999px;
  background: var(--td-bg-color-container);
  color: var(--ordo-text-secondary);
  font-size: 12px;
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

.policy-card :deep(.t-card__body) {
  display: grid;
  gap: 14px;
}

.policy-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.policy-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.policy-desc,
.policy-foot {
  font-size: 13px;
  color: var(--ordo-text-secondary);
}

.policy-kv {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 16px;
}

.kv {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.kv span {
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.kv strong {
  font-size: 13px;
  color: var(--ordo-text-primary);
  font-weight: 500;
}

.policy-foot {
  display: flex;
  justify-content: space-between;
  gap: 12px;
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}

@media (max-width: 980px) {
  .view-page {
    padding: 20px;
  }

  .policy-head,
  .policy-foot {
    flex-direction: column;
  }

  .policy-kv {
    grid-template-columns: repeat(2, 1fr);
  }

  .dialog-grid {
    grid-template-columns: 1fr;
  }
}
</style>
