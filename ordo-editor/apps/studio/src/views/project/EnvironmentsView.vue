<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { DialogPlugin, MessagePlugin } from 'tdesign-vue-next'
import { useEnvironmentStore } from '@/stores/environment'
import { useServerStore } from '@/stores/server'
import { StudioDialogActions, StudioPageHeader } from '@/components/ui'
import type { ProjectEnvironment } from '@/api/types'

const route = useRoute()
const { t } = useI18n()
const envStore = useEnvironmentStore()
const serverStore = useServerStore()

const orgId = route.params.orgId as string
const projectId = route.params.projectId as string

const showForm = ref(false)
const editingEnv = ref<ProjectEnvironment | null>(null)
const formName = ref('')
const formServerId = ref('')
const formNatsPrefix = ref('')
const saving = ref(false)

const showCanaryForm = ref(false)
const canaryEnv = ref<ProjectEnvironment | null>(null)
const canaryTargetId = ref('')
const canaryPct = ref(0)
const canarySaving = ref(false)

const availableCanaryTargets = computed(() =>
  envStore.environments.filter((env) => env.id !== canaryEnv.value?.id),
)

onMounted(async () => {
  await Promise.all([envStore.fetchEnvironments(orgId, projectId), serverStore.fetchServers()])
})

function startCreate() {
  editingEnv.value = null
  formName.value = ''
  formServerId.value = ''
  formNatsPrefix.value = ''
  showForm.value = true
}

function startEdit(env: ProjectEnvironment) {
  editingEnv.value = env
  formName.value = env.name
  formServerId.value = env.server_id ?? ''
  formNatsPrefix.value = env.nats_subject_prefix ?? ''
  showForm.value = true
}

async function saveForm() {
  const trimmedName = formName.value.trim()
  if (!trimmedName) {
    MessagePlugin.warning(t('environment.name'))
    return
  }

  saving.value = true
  try {
    if (editingEnv.value) {
      await envStore.updateEnvironment(orgId, projectId, editingEnv.value.id, {
        name: trimmedName,
        server_id: formServerId.value || null,
        nats_subject_prefix: formNatsPrefix.value.trim() || null,
      })
      MessagePlugin.success(t('environment.updated'))
    } else {
      await envStore.createEnvironment(orgId, projectId, {
        name: trimmedName,
        server_id: formServerId.value || null,
        nats_subject_prefix: formNatsPrefix.value.trim() || null,
      })
      MessagePlugin.success(t('environment.created'))
    }
    showForm.value = false
  } catch (e: any) {
    MessagePlugin.error(e.message)
  } finally {
    saving.value = false
  }
}

function deleteEnv(env: ProjectEnvironment) {
  const dialog = DialogPlugin.confirm({
    header: t('environment.delete'),
    body: t('environment.confirmDelete', { name: env.name }),
    confirmBtn: { content: t('common.delete'), theme: 'danger' },
    cancelBtn: t('common.cancel'),
    onConfirm: async () => {
      try {
        await envStore.deleteEnvironment(orgId, projectId, env.id)
        MessagePlugin.success(t('environment.deleted'))
        dialog.hide()
      } catch (e: any) {
        MessagePlugin.error(e.message)
      }
    },
  })
}

function startCanary(env: ProjectEnvironment) {
  canaryEnv.value = env
  canaryTargetId.value = env.canary_target_env_id ?? ''
  canaryPct.value = env.canary_percentage
  showCanaryForm.value = true
}

async function saveCanary() {
  if (!canaryEnv.value) return
  canarySaving.value = true
  try {
    await envStore.setCanary(orgId, projectId, canaryEnv.value.id, {
      canary_target_env_id: canaryTargetId.value || null,
      canary_percentage: canaryPct.value,
    })
    MessagePlugin.success(t('environment.canaryUpdated'))
    showCanaryForm.value = false
  } catch (e: any) {
    MessagePlugin.error(e.message)
  } finally {
    canarySaving.value = false
  }
}

function serverName(serverId: string | null) {
  if (!serverId) return t('environment.noServer')
  return serverStore.servers.find((server) => server.id === serverId)?.name ?? serverId
}

function canaryTargetName(envId: string | null) {
  if (!envId) return t('environment.noCanary')
  return envStore.environments.find((env) => env.id === envId)?.name ?? envId
}
</script>

<template>
  <div class="view-page">
    <StudioPageHeader :title="t('environment.title')">
      <template #actions>
        <t-button theme="primary" @click="startCreate">
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
            <t-tag v-if="env.is_default" theme="primary" variant="light">{{ t('environment.default') }}</t-tag>
          </div>
          <div class="env-actions">
            <t-button variant="text" size="small" @click="startCanary(env)">
              {{ t('environment.canary') }}
            </t-button>
            <t-button variant="text" size="small" @click="startEdit(env)">
              {{ t('environment.edit') }}
            </t-button>
            <t-button v-if="!env.is_default" variant="text" theme="danger" size="small" @click="deleteEnv(env)">
              {{ t('environment.delete') }}
            </t-button>
          </div>
        </div>

        <div class="env-meta">
          <span class="meta-label">{{ t('environment.server') }}:</span>
          <span>{{ serverName(env.server_id) }}</span>
        </div>
        <div v-if="env.nats_subject_prefix" class="env-meta">
          <span class="meta-label">{{ t('environment.natsPrefix') }}:</span>
          <code>{{ env.nats_subject_prefix }}</code>
        </div>
        <div v-if="env.canary_percentage > 0" class="env-meta">
          <span class="meta-label">{{ t('environment.canary') }}:</span>
          <span>{{ env.canary_percentage }}% → {{ canaryTargetName(env.canary_target_env_id) }}</span>
        </div>
      </t-card>
    </div>

    <t-dialog
      v-model:visible="showForm"
      :header="editingEnv ? t('environment.edit') : t('environment.add')"
      :footer="false"
      width="520px"
      destroy-on-close
    >
      <t-form label-align="top" :colon="false">
        <t-form-item :label="t('environment.name')" required>
          <t-input v-model="formName" />
        </t-form-item>
        <t-form-item :label="t('environment.server')">
          <t-select v-model="formServerId" clearable>
            <t-option
              v-for="server in serverStore.servers"
              :key="server.id"
              :label="server.name"
              :value="server.id"
            />
          </t-select>
        </t-form-item>
        <t-form-item :label="t('environment.natsPrefix')">
          <t-input v-model="formNatsPrefix" placeholder="ordo.rules" />
        </t-form-item>
      </t-form>
      <StudioDialogActions>
        <t-button variant="outline" @click="showForm = false">{{ t('environment.cancel') }}</t-button>
        <t-button theme="primary" :loading="saving" @click="saveForm">{{ t('environment.save') }}</t-button>
      </StudioDialogActions>
    </t-dialog>

    <t-dialog
      v-model:visible="showCanaryForm"
      :header="`${t('environment.canary')}: ${canaryEnv?.name ?? ''}`"
      :footer="false"
      width="520px"
      destroy-on-close
    >
      <t-form label-align="top" :colon="false">
        <t-form-item :label="t('environment.canaryTarget')">
          <t-select v-model="canaryTargetId" clearable>
            <t-option
              v-for="env in availableCanaryTargets"
              :key="env.id"
              :label="env.name"
              :value="env.id"
            />
          </t-select>
        </t-form-item>
        <t-form-item :label="`${t('environment.canaryPct')}: ${canaryPct}%`">
          <t-slider v-model="canaryPct" :min="0" :max="100" />
        </t-form-item>
      </t-form>
      <StudioDialogActions>
        <t-button variant="outline" @click="showCanaryForm = false">{{ t('environment.cancel') }}</t-button>
        <t-button theme="primary" :loading="canarySaving" @click="saveCanary">{{ t('environment.save') }}</t-button>
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

.env-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.env-card :deep(.t-card__body) {
  padding: 16px;
}

.env-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 10px;
}

.env-name-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.env-name {
  font-size: 15px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.env-actions {
  display: flex;
  align-items: center;
}

.env-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 8px;
  font-size: 13px;
  color: var(--ordo-text-secondary);
}

.meta-label {
  font-weight: 600;
}

code {
  display: inline-block;
  padding: 2px 8px;
  border-radius: var(--ordo-radius-sm);
  background: var(--ordo-bg-secondary);
  color: var(--ordo-text-primary);
  font-family: var(--ordo-font-mono);
  font-size: 12px;
}

@media (max-width: 900px) {
  .view-page {
    padding: 20px;
  }

  .env-header {
    flex-direction: column;
    align-items: flex-start;
  }
}
</style>
