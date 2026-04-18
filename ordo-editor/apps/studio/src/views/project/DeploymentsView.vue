<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useAuthStore } from '@/stores/auth'
import { useEnvironmentStore } from '@/stores/environment'
import { rulesetDraftApi } from '@/api/platform-client'
import { MessagePlugin } from 'tdesign-vue-next'
import type { RulesetDeployment } from '@/api/types'

const route = useRoute()
const { t } = useI18n()
const auth = useAuthStore()
const envStore = useEnvironmentStore()

const orgId = route.params.orgId as string
const projectId = route.params.projectId as string

const deployments = ref<RulesetDeployment[]>([])
const loading = ref(false)
const redeployingId = ref<string | null>(null)
const showRedeployDialog = ref(false)
const selectedDeployment = ref<RulesetDeployment | null>(null)
const redeployEnvId = ref('')

onMounted(async () => {
  loading.value = true
  try {
    await envStore.fetchEnvironments(orgId, projectId)
    redeployEnvId.value = envStore.environments.find((e) => e.is_default)?.id ?? ''
    deployments.value = await rulesetDraftApi.listProjectDeployments(auth.token!, orgId, projectId, 100)
  } finally {
    loading.value = false
  }
})

function statusClass(status: string) {
  return {
    'status-queued': status === 'queued',
    'status-success': status === 'success',
    'status-failed': status === 'failed',
  }
}

function statusLabel(status: string) {
  if (status === 'queued') return t('deployments.statusQueued')
  if (status === 'success') return t('deployments.statusSuccess')
  return t('deployments.statusFailed')
}

function openRedeploy(dep: RulesetDeployment) {
  selectedDeployment.value = dep
  redeployEnvId.value = envStore.environments.find((e) => e.is_default)?.id ?? dep.environment_id
  showRedeployDialog.value = true
}

async function confirmRedeploy() {
  if (!selectedDeployment.value || !redeployEnvId.value) return
  const dep = selectedDeployment.value
  redeployingId.value = dep.id
  showRedeployDialog.value = false
  try {
    const newDep = await rulesetDraftApi.redeploy(
      auth.token!,
      orgId,
      projectId,
      dep.ruleset_name,
      dep.id,
      { environment_id: redeployEnvId.value },
    )
    deployments.value.unshift(newDep)
    MessagePlugin.success(t('deployments.redeploySuccess'))
  } catch (e: any) {
    MessagePlugin.error(e.message)
  } finally {
    redeployingId.value = null
    selectedDeployment.value = null
  }
}

function formatDate(dt: string) {
  return new Date(dt).toLocaleString()
}
</script>

<template>
  <div class="deployments-view">
    <h2 class="page-title">{{ $t('deployments.title') }}</h2>

    <div v-if="loading" class="loading">{{ $t('common.loading') }}</div>

    <div v-else-if="deployments.length === 0" class="empty">
      {{ $t('deployments.noDeployments') }}
    </div>

    <div v-else class="deployment-list">
      <div v-for="dep in deployments" :key="dep.id" class="deployment-card">
        <div class="card-header">
          <span class="ruleset-name">{{ dep.ruleset_name }}</span>
          <span class="version">v{{ dep.version }}</span>
          <span class="status-badge" :class="statusClass(dep.status)">
            {{ statusLabel(dep.status) }}
          </span>
        </div>
        <div class="card-meta">
          <span class="env-name">{{ dep.environment_name ?? dep.environment_id }}</span>
          <span class="sep">·</span>
          <span class="deployed-by">{{ dep.deployed_by ?? '—' }}</span>
          <span class="sep">·</span>
          <span class="deployed-at">{{ formatDate(dep.deployed_at) }}</span>
        </div>
        <div v-if="dep.release_note" class="release-note">{{ dep.release_note }}</div>
        <div class="card-actions">
          <button
            class="btn-redeploy"
            :disabled="redeployingId === dep.id"
            @click="openRedeploy(dep)"
          >
            {{ redeployingId === dep.id ? $t('deployments.redeploying') : $t('deployments.redeploy') }}
          </button>
        </div>
      </div>
    </div>

    <!-- Redeploy dialog -->
    <div v-if="showRedeployDialog" class="dialog-overlay" @click.self="showRedeployDialog = false">
      <div class="dialog">
        <h3>{{ $t('deployments.redeploy') }}</h3>
        <div class="field">
          <label>{{ $t('deployments.environment') }}</label>
          <select v-model="redeployEnvId" class="input">
            <option v-for="env in envStore.environments" :key="env.id" :value="env.id">
              {{ env.name }}{{ env.is_default ? ' ★' : '' }}
            </option>
          </select>
        </div>
        <div class="dialog-actions">
          <button class="btn-secondary" @click="showRedeployDialog = false">{{ $t('common.cancel') }}</button>
          <button class="btn-primary" @click="confirmRedeploy">{{ $t('deployments.redeploy') }}</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.deployments-view {
  padding: 24px;
  max-width: 900px;
}
.page-title {
  margin: 0 0 20px;
  font-size: 20px;
  font-weight: 600;
}
.loading,
.empty {
  color: var(--text-secondary, #a6adc8);
  font-size: 14px;
}
.deployment-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.deployment-card {
  background: var(--surface-color, #1e1e2e);
  border: 1px solid var(--border-color, #313244);
  border-radius: 6px;
  padding: 14px 16px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.card-header {
  display: flex;
  align-items: center;
  gap: 10px;
}
.ruleset-name {
  font-weight: 600;
  font-size: 14px;
}
.version {
  font-size: 12px;
  color: var(--text-secondary, #a6adc8);
}
.status-badge {
  font-size: 11px;
  font-weight: 600;
  padding: 2px 8px;
  border-radius: 10px;
}
.status-queued { background: #585b7066; color: #a6adc8; }
.status-success { background: #a6e3a133; color: #a6e3a1; }
.status-failed { background: #f38ba833; color: #f38ba8; }
.card-meta {
  font-size: 12px;
  color: var(--text-secondary, #a6adc8);
  display: flex;
  align-items: center;
  gap: 6px;
}
.sep { opacity: 0.4; }
.release-note {
  font-size: 12px;
  color: var(--text-secondary, #cdd6f4);
  font-style: italic;
}
.card-actions {
  display: flex;
  justify-content: flex-end;
}
.btn-redeploy {
  font-size: 12px;
  padding: 4px 12px;
  border-radius: 4px;
  border: 1px solid var(--border-color, #45475a);
  background: transparent;
  color: inherit;
  cursor: pointer;
}
.btn-redeploy:disabled { opacity: 0.5; cursor: not-allowed; }
.dialog-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0,0,0,0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}
.dialog {
  background: var(--surface-color, #1e1e2e);
  border: 1px solid var(--border-color, #313244);
  border-radius: 8px;
  padding: 24px;
  width: 380px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.field { display: flex; flex-direction: column; gap: 6px; }
.field label { font-size: 12px; color: var(--text-secondary, #a6adc8); }
.input {
  background: var(--input-bg, #313244);
  border: 1px solid var(--border-color, #45475a);
  border-radius: 4px;
  padding: 8px 10px;
  color: inherit;
  font-size: 13px;
}
.dialog-actions { display: flex; justify-content: flex-end; gap: 8px; }
.btn-primary {
  padding: 8px 16px; border-radius: 4px; border: none; cursor: pointer;
  background: var(--accent-color, #cba6f7); color: #1e1e2e; font-weight: 600; font-size: 13px;
}
.btn-secondary {
  padding: 8px 16px; border-radius: 4px;
  border: 1px solid var(--border-color, #45475a);
  cursor: pointer; background: transparent; color: inherit; font-size: 13px;
}
</style>
