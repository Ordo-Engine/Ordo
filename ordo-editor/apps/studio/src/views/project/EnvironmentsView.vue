<script setup lang="ts">
import { onMounted } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { DialogPlugin, MessagePlugin } from 'tdesign-vue-next';
import type { ProjectEnvironment } from '@/api/types';
import { StudioPageHeader } from '@/components/ui';
import { useEnvironmentStore } from '@/stores/environment';
import { useServerStore } from '@/stores/server';

const route = useRoute();
const router = useRouter();
const { t } = useI18n();
const envStore = useEnvironmentStore();
const serverStore = useServerStore();

const orgId = route.params.orgId as string;
const projectId = route.params.projectId as string;

onMounted(async () => {
  await Promise.all([envStore.fetchEnvironments(orgId, projectId), serverStore.fetchServers()]);
});

function createEnv() {
  router.push({ name: 'project-environment-create', params: { orgId, projectId } });
}

function editEnv(envId: string) {
  router.push({ name: 'project-environment-edit', params: { orgId, projectId, envId } });
}

function deleteEnv(env: ProjectEnvironment) {
  const dialog = DialogPlugin.confirm({
    header: t('environment.delete'),
    body: t('environment.confirmDelete', { name: env.name }),
    confirmBtn: { content: t('common.delete'), theme: 'danger' },
    cancelBtn: t('common.cancel'),
    onConfirm: async () => {
      try {
        await envStore.deleteEnvironment(orgId, projectId, env.id);
        MessagePlugin.success(t('environment.deleted'));
        dialog.hide();
      } catch (e: any) {
        MessagePlugin.error(e.message);
      }
    },
  });
}

function serverNames(serverIds: string[]) {
  if (serverIds.length === 0) return t('environment.noServer');
  return serverIds.map((serverId) => serverStore.getById(serverId)?.name ?? serverId).join(', ');
}

function canaryTargetName(envId: string | null) {
  if (!envId) return t('environment.noCanary');
  return envStore.environments.find((env) => env.id === envId)?.name ?? envId;
}
</script>

<template>
  <div class="view-page">
    <StudioPageHeader :title="t('environment.title')">
      <template #actions>
        <t-button theme="primary" @click="createEnv">
          <template #icon>
            <t-icon name="add" />
          </template>
          {{ t('environment.add') }}
        </t-button>
      </template>
    </StudioPageHeader>

    <div v-if="envStore.loading" class="list-skeleton">
      <t-skeleton
        v-for="i in 3"
        :key="i"
        theme="paragraph"
        animation="gradient"
        :row-col="[{ width: '35%' }, { width: '60%' }, { width: '45%' }]"
      />
    </div>

    <div v-else-if="envStore.environments.length === 0" class="state-center">
      <t-empty :title="t('environment.noEnvs')" />
    </div>

    <div v-else class="env-list">
      <t-card v-for="env in envStore.environments" :key="env.id" :bordered="false" class="env-card">
        <div class="env-header">
          <div class="env-name-row">
            <span class="env-name">{{ env.name }}</span>
            <t-tag v-if="env.is_default" theme="primary" variant="light">{{
              t('environment.default')
            }}</t-tag>
          </div>
          <div class="env-actions">
            <t-button variant="text" size="small" @click="editEnv(env.id)">
              {{ t('environment.edit') }}
            </t-button>
            <t-button
              v-if="!env.is_default"
              variant="text"
              theme="danger"
              size="small"
              @click="deleteEnv(env)"
            >
              {{ t('environment.delete') }}
            </t-button>
          </div>
        </div>

        <div class="env-meta">
          <span class="meta-label">{{ t('environment.serverNodes') }}:</span>
          <span>{{ serverNames(env.server_ids) }}</span>
        </div>
        <div v-if="env.nats_subject_prefix" class="env-meta">
          <span class="meta-label">{{ t('environment.natsPrefix') }}:</span>
          <code>{{ env.nats_subject_prefix }}</code>
        </div>
        <div v-if="env.canary_percentage > 0" class="env-meta">
          <span class="meta-label">{{ t('environment.canary') }}:</span>
          <span
            >{{ env.canary_percentage }}% → {{ canaryTargetName(env.canary_target_env_id) }}</span
          >
        </div>
      </t-card>
    </div>
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

.env-list {
  display: grid;
  gap: 16px;
}

.env-card {
  border-radius: 18px;
  border: 1px solid var(--ordo-border-color);
}

.env-header {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  align-items: flex-start;
}

.env-name-row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 10px;
}

.env-name {
  font-size: 18px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.env-actions {
  display: flex;
  align-items: center;
  gap: 4px;
}

.env-meta {
  display: flex;
  gap: 8px;
  color: var(--ordo-text-secondary);
  font-size: 13px;
  margin-top: 8px;
  line-height: 1.5;
}

.meta-label {
  color: var(--ordo-text-tertiary);
  min-width: 72px;
}
</style>
