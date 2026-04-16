<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { MessagePlugin } from 'tdesign-vue-next'
import { useAuthStore } from '@/stores/auth'
import { useOrgStore } from '@/stores/org'
import { useProjectStore } from '@/stores/project'
import { projectApi } from '@/api/platform-client'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const auth = useAuthStore()
const orgStore = useOrgStore()
const projectStore = useProjectStore()

const orgId = computed(() => route.params.orgId as string)
const projectId = computed(() => route.params.projectId as string)
const project = computed(() => projectStore.currentProject)

// General form
const nameValue = ref('')
const descValue = ref('')
const saving = ref(false)

onMounted(() => {
  if (project.value) {
    nameValue.value = project.value.name
    descValue.value = project.value.description ?? ''
  }
})

const canEdit = computed(() => {
  if (!auth.user) return false
  return orgStore.canEdit(auth.user.id)
})

const canAdmin = computed(() => {
  if (!auth.user) return false
  return orgStore.canAdmin(auth.user.id)
})

async function saveGeneral() {
  if (!nameValue.value.trim()) {
    MessagePlugin.warning(t('projectSettings.nameRequired'))
    return
  }
  if (!orgStore.currentOrg) return
  saving.value = true
  try {
    await projectApi.update(auth.token!, orgStore.currentOrg.id, projectId.value, {
      name: nameValue.value.trim(),
      description: descValue.value.trim() || undefined,
    })
    // Refresh project list
    await projectStore.fetchProjects(orgStore.currentOrg.id)
    MessagePlugin.success(t('projectSettings.saveSuccess'))
  } catch {
    MessagePlugin.error(t('projectSettings.saveFailed'))
  } finally {
    saving.value = false
  }
}

// Delete
const showDeleteDialog = ref(false)
const deleting = ref(false)

async function confirmDelete() {
  if (!orgStore.currentOrg) return
  deleting.value = true
  try {
    await projectStore.deleteProject(orgStore.currentOrg.id, projectId.value)
    MessagePlugin.success(t('projectSettings.deleteSuccess'))
    router.push(`/orgs/${orgId.value}/projects`)
  } catch {
    MessagePlugin.error(t('common.saveFailed'))
  } finally {
    deleting.value = false
    showDeleteDialog.value = false
  }
}
</script>

<template>
  <div class="project-settings-page">
    <!-- Breadcrumb -->
    <t-breadcrumb class="breadcrumb">
      <t-breadcrumb-item @click="router.push('/dashboard')">{{ t('breadcrumb.home') }}</t-breadcrumb-item>
      <t-breadcrumb-item @click="router.push(`/orgs/${orgId}/projects`)">{{ t('breadcrumb.projects') }}</t-breadcrumb-item>
      <t-breadcrumb-item>{{ project?.name }}</t-breadcrumb-item>
      <t-breadcrumb-item>{{ t('breadcrumb.projectSettings') }}</t-breadcrumb-item>
    </t-breadcrumb>

    <h1 class="page-title">{{ t('projectSettings.title') }}</h1>

    <div class="settings-layout">
      <!-- General -->
      <t-card :bordered="false" class="settings-card">
        <h2 class="card-title">{{ t('projectSettings.general') }}</h2>
        <t-form label-align="top" :colon="false" class="settings-form">
          <t-form-item :label="t('projectSettings.nameLabel')">
            <t-input
              v-model="nameValue"
              :disabled="!canEdit"
              :placeholder="t('project.namePlaceholder')"
            />
          </t-form-item>
          <t-form-item :label="t('projectSettings.descLabel')">
            <t-textarea
              v-model="descValue"
              :disabled="!canEdit"
              :placeholder="t('projectSettings.descPlaceholder')"
              :autosize="{ minRows: 2, maxRows: 4 }"
            />
          </t-form-item>
          <t-form-item v-if="canEdit">
            <t-button theme="primary" :loading="saving" @click="saveGeneral">
              {{ t('projectSettings.saveBtn') }}
            </t-button>
          </t-form-item>
        </t-form>
      </t-card>

      <!-- Member Access -->
      <t-card :bordered="false" class="settings-card">
        <h2 class="card-title">{{ t('projectSettings.members') }}</h2>
        <p class="card-desc">{{ t('projectSettings.membersDesc') }}</p>
        <t-button
          variant="outline"
          size="small"
          @click="router.push(`/orgs/${orgStore.currentOrg?.id}/members`)"
        >
          <t-icon name="user" />
          {{ t('projectSettings.viewMembers') }}
        </t-button>
      </t-card>

      <!-- Engine Status -->
      <t-card :bordered="false" class="settings-card">
        <h2 class="card-title">{{ t('projectSettings.engineStatus') }}</h2>
        <div class="engine-status">
          <t-tag theme="success" variant="light">
            <t-icon name="check-circle" />
            {{ t('projectSettings.engineConnected') }}
          </t-tag>
          <span class="engine-id">ID: {{ projectId }}</span>
        </div>
      </t-card>

      <!-- Danger Zone -->
      <t-card v-if="canAdmin" :bordered="false" class="settings-card danger-card">
        <h2 class="card-title danger-title">{{ t('projectSettings.danger') }}</h2>
        <div class="danger-row">
          <div>
            <div class="danger-label">{{ t('projectSettings.deleteLabel') }}</div>
            <div class="danger-desc">{{ t('projectSettings.deleteDesc') }}</div>
          </div>
          <t-button theme="danger" variant="outline" @click="showDeleteDialog = true">
            {{ t('projectSettings.deleteBtn') }}
          </t-button>
        </div>
      </t-card>
    </div>

    <!-- Delete dialog -->
    <t-dialog
      v-model:visible="showDeleteDialog"
      :header="t('projectSettings.deleteDialog')"
      :confirm-btn="{ content: t('projectSettings.deleteBtn'), theme: 'danger', loading: deleting }"
      :cancel-btn="t('common.cancel')"
      @confirm="confirmDelete"
      @cancel="showDeleteDialog = false"
    >
      <p>{{ t('projectSettings.deleteConfirm', { name: project?.name }) }}</p>
    </t-dialog>
  </div>
</template>

<style scoped>
.project-settings-page {
  padding: 32px;
  overflow-y: auto;
  height: 100%;
}

.breadcrumb {
  margin-bottom: 16px;
}

.page-title {
  margin: 0 0 24px;
  font-size: 20px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.settings-layout {
  display: flex;
  flex-direction: column;
  gap: 20px;
  max-width: 600px;
}

.settings-card {
  background: var(--ordo-bg-panel);
  border-radius: 8px;
}

.card-title {
  margin: 0 0 16px;
  font-size: 14px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.card-desc {
  margin: 0 0 14px;
  font-size: 13px;
  color: var(--ordo-text-secondary);
}

.settings-form {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.engine-status {
  display: flex;
  align-items: center;
  gap: 12px;
}

.engine-id {
  font-size: 12px;
  color: var(--ordo-text-tertiary);
  font-family: 'JetBrains Mono', monospace;
}

.danger-card {
  border: 1px solid rgba(245, 63, 63, 0.3) !important;
}

.danger-title {
  color: #f53f3f;
}

.danger-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
}

.danger-label {
  font-size: 13px;
  font-weight: 500;
  color: var(--ordo-text-primary);
  margin-bottom: 4px;
}

.danger-desc {
  font-size: 12px;
  color: var(--ordo-text-secondary);
}
</style>
