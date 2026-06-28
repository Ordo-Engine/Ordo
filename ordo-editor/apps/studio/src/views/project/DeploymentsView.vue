<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { useRoute } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { MessagePlugin } from 'tdesign-vue-next';
import { useAuthStore } from '@/stores/auth';
import { useEnvironmentStore } from '@/stores/environment';
import { rulesetDraftApi } from '@/api/platform-client';
import ReleaseNav from '@/components/project/ReleaseNav.vue';
import { StudioDialogActions, StudioPageHeader } from '@/components/ui';
import type { RulesetDeployment } from '@/api/types';

const route = useRoute();
const { t } = useI18n();
const auth = useAuthStore();
const envStore = useEnvironmentStore();

const orgId = route.params.orgId as string;
const projectId = route.params.projectId as string;

const deployments = ref<RulesetDeployment[]>([]);
const loading = ref(false);
const redeployingId = ref<string | null>(null);
const showRedeployDialog = ref(false);
const selectedDeployment = ref<RulesetDeployment | null>(null);
const redeployEnvId = ref('');

onMounted(async () => {
  loading.value = true;
  try {
    await envStore.fetchEnvironments(orgId, projectId);
    redeployEnvId.value = envStore.environments.find((env) => env.is_default)?.id ?? '';
    deployments.value = await rulesetDraftApi.listProjectDeployments(
      auth.token!,
      orgId,
      projectId,
      100
    );
  } finally {
    loading.value = false;
  }
});

function statusTheme(status: string) {
  if (status === 'success') return 'success';
  if (status === 'failed') return 'danger';
  return 'warning';
}

function statusLabel(status: string) {
  if (status === 'queued') return t('deployments.statusQueued');
  if (status === 'success') return t('deployments.statusSuccess');
  return t('deployments.statusFailed');
}

function openRedeploy(dep: RulesetDeployment) {
  selectedDeployment.value = dep;
  redeployEnvId.value =
    envStore.environments.find((env) => env.is_default)?.id ?? dep.environment_id;
  showRedeployDialog.value = true;
}

async function confirmRedeploy() {
  if (!selectedDeployment.value || !redeployEnvId.value) return;
  const dep = selectedDeployment.value;
  redeployingId.value = dep.id;
  showRedeployDialog.value = false;
  try {
    const newDep = await rulesetDraftApi.redeploy(
      auth.token!,
      orgId,
      projectId,
      dep.ruleset_name,
      dep.id,
      { environment_id: redeployEnvId.value }
    );
    deployments.value.unshift(newDep);
    MessagePlugin.success(t('deployments.redeploySuccess'));
  } catch (e: any) {
    MessagePlugin.error(e.message);
  } finally {
    redeployingId.value = null;
    selectedDeployment.value = null;
  }
}

function formatDate(dt: string) {
  return new Date(dt).toLocaleString();
}
</script>

<template>
  <div class="view-page">
    <StudioPageHeader
      :title="t('releaseCenter.historyTitle')"
      :subtitle="t('releaseCenter.historySubtitle')"
    />
    <ReleaseNav />

    <div v-if="loading" class="list-skeleton">
      <t-skeleton
        v-for="i in 3"
        :key="i"
        theme="paragraph"
        animation="gradient"
        :row-col="[{ width: '38%' }, { width: '58%' }, { width: '30%' }]"
      />
    </div>

    <div v-else-if="deployments.length === 0" class="state-center">
      <t-empty :title="t('deployments.noDeployments')" />
    </div>

    <div v-else class="deployment-list">
      <t-card v-for="dep in deployments" :key="dep.id" :bordered="false" class="deployment-card">
        <div class="card-header">
          <span class="ruleset-name">{{ dep.ruleset_name }}</span>
          <span class="version">v{{ dep.version }}</span>
          <t-tag :theme="statusTheme(dep.status)" variant="light">
            {{ statusLabel(dep.status) }}
          </t-tag>
        </div>

        <div class="card-meta">
          <span>{{ dep.environment_name ?? dep.environment_id }}</span>
          <span class="sep">·</span>
          <span>{{ dep.deployed_by ?? '—' }}</span>
          <span class="sep">·</span>
          <span>{{ formatDate(dep.deployed_at) }}</span>
        </div>

        <div v-if="dep.release_note" class="release-note">{{ dep.release_note }}</div>

        <div class="card-actions">
          <t-button
            variant="outline"
            size="small"
            :loading="redeployingId === dep.id"
            @click="openRedeploy(dep)"
          >
            {{
              redeployingId === dep.id ? t('deployments.redeploying') : t('deployments.redeploy')
            }}
          </t-button>
        </div>
      </t-card>
    </div>

    <t-dialog
      v-model:visible="showRedeployDialog"
      :header="t('deployments.redeploy')"
      :footer="false"
      width="460px"
      destroy-on-close
    >
      <t-form label-align="top" :colon="false">
        <t-form-item :label="t('deployments.environment')" required>
          <t-select v-model="redeployEnvId">
            <t-option
              v-for="env in envStore.environments"
              :key="env.id"
              :value="env.id"
              :label="`${env.name}${env.is_default ? ' ★' : ''}`"
            />
          </t-select>
        </t-form-item>
      </t-form>
      <StudioDialogActions>
        <t-button variant="outline" @click="showRedeployDialog = false">{{
          t('common.cancel')
        }}</t-button>
        <t-button theme="primary" @click="confirmRedeploy">{{
          t('deployments.redeploy')
        }}</t-button>
      </StudioDialogActions>
    </t-dialog>
  </div>
</template>

<style scoped>
.view-page {
  padding: 24px 32px 32px;
  height: 100%;
  overflow-y: auto;
}

.list-skeleton {
  display: grid;
  gap: 12px;
}

.state-center {
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 240px;
}

.deployment-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.deployment-card :deep(.t-card__body) {
  padding: 16px;
}

.card-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 8px;
}

.ruleset-name {
  font-size: 15px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.version {
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.card-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.sep {
  opacity: 0.5;
}

.release-note {
  margin-top: 8px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
  font-style: italic;
}

.card-actions {
  display: flex;
  justify-content: flex-end;
  margin-top: 12px;
}

@media (max-width: 900px) {
  .view-page {
    padding: 20px;
  }

  .card-meta {
    flex-wrap: wrap;
  }
}
</style>
