<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useAuthStore } from '@/stores/auth'
import { useOrgStore } from '@/stores/org'
import { useProjectStore } from '@/stores/project'
import { MessagePlugin, DialogPlugin } from 'tdesign-vue-next'
import CreateFromTemplateDialog from '@/components/project/CreateFromTemplateDialog.vue'
import type { Project } from '@/api/types'

const router = useRouter()
const route = useRoute()
const auth = useAuthStore()
const orgStore = useOrgStore()
const projectStore = useProjectStore()
const { t, locale } = useI18n()

const orgId = computed(() => route.params.orgId as string)

const showCreate = ref(false)
const showTemplateDialog = ref(false)
const creating = ref(false)
const newName = ref('')
const newDesc = ref('')

async function handleTemplateCreated(p: Project) {
  projectStore.projects.unshift(p)
  MessagePlugin.success(t('template.createSuccess'))
  await openProject(p.id)
}

const isAdmin = computed(() => {
  if (!auth.user) return false
  return orgStore.canAdmin(auth.user.id)
})

onMounted(async () => {
  // On refresh, orgId comes from the URL — ensure the org is selected
  if (orgId.value && orgStore.currentOrg?.id !== orgId.value) {
    await orgStore.selectOrg(orgId.value)
  }
  await projectStore.fetchProjects(orgId.value)
})

watch(
  () => route.query.openTemplate,
  (value) => {
    if (value === '1' && isAdmin.value) {
      showTemplateDialog.value = true
    }
  },
  { immediate: true },
)

async function handleCreate() {
  if (!newName.value.trim()) {
    MessagePlugin.warning(t('project.nameRequired'))
    return
  }
  if (!orgStore.currentOrg) return
  creating.value = true
  try {
    const p = await projectStore.createProject(orgStore.currentOrg.id, newName.value.trim(), newDesc.value || undefined)
    showCreate.value = false
    newName.value = ''
    newDesc.value = ''
    MessagePlugin.success(t('project.createSuccess'))
    await openProject(p.id)
  } catch (e: any) {
    MessagePlugin.error(e.message)
  } finally {
    creating.value = false
  }
}

async function openProject(projectId: string) {
  const p = projectStore.projects.find((p) => p.id === projectId)
  if (!p) return
  await projectStore.selectProject(p)
  router.push(`/orgs/${orgId.value}/projects/${projectId}/editor`)
}

function goToMarketplace() {
  router.push('/marketplace')
}

function handleDelete(projectId: string, name: string) {
  if (!orgStore.currentOrg) return
  const orgId = orgStore.currentOrg.id
  const dlg = DialogPlugin.confirm({
    header: t('project.deleteDialog'),
    body: t('project.deleteConfirm', { name }),
    confirmBtn: { content: t('common.delete'), theme: 'danger' },
    cancelBtn: t('common.cancel'),
    onConfirm: async () => {
      try {
        await projectStore.deleteProject(orgId, projectId)
        dlg.hide()
        MessagePlugin.success(t('project.deleteSuccess'))
      } catch (e: any) {
        MessagePlugin.error(e.message)
      }
    },
  })
}

function formatDate(iso: string) {
  return new Date(iso).toLocaleDateString(
    locale.value === 'zh-TW' ? 'zh-TW' : locale.value === 'zh-CN' ? 'zh-CN' : 'en-US',
    { year: 'numeric', month: 'short', day: 'numeric' },
  )
}
</script>

<template>
  <div class="view-page">
    <div class="page-header">
      <div>
        <h2 class="page-title">{{ t('project.title') }}</h2>
        <p class="page-subtitle">
          {{ t('project.subtitle', { org: orgStore.currentOrg?.name ?? '' }) }}
        </p>
      </div>
      <div class="page-header__actions">
        <t-button theme="default" variant="outline" @click="goToMarketplace">
          <t-icon name="shop" />
          {{ t('marketplace.title') }}
        </t-button>
        <t-button v-if="isAdmin" theme="primary" @click="showTemplateDialog = true">
          <t-icon name="gift" />
          {{ t('template.fromTemplate') }}
        </t-button>
        <t-button v-if="isAdmin" theme="default" variant="outline" @click="showCreate = true">
          <t-icon name="add" />
          {{ t('project.new') }}
        </t-button>
      </div>
    </div>

    <!-- Loading -->
    <div v-if="projectStore.loading" class="project-skeleton-grid">
      <t-skeleton v-for="i in 4" :key="i" theme="paragraph" animation="gradient"
        :row-col="[{ width: '50%' }, { width: '70%' }, { width: '40%' }]" />
    </div>

    <!-- Empty -->
    <div v-else-if="projectStore.projects.length === 0" class="state-center">
      <t-empty
        :title="t('project.emptyTitle')"
        :description="t('project.emptyDesc')"
      >
        <template #action>
          <t-button v-if="isAdmin" theme="primary" @click="showCreate = true">{{ t('project.createBtn') }}</t-button>
        </template>
      </t-empty>
    </div>

    <!-- Grid -->
    <div v-else class="project-grid">
      <div
        v-for="project in projectStore.projects"
        :key="project.id"
        class="project-card"
        :class="{ 'is-active': project.id === projectStore.currentProjectId }"
        @click="openProject(project.id)"
      >
        <div class="project-card__header">
          <div class="project-card__icon">
            <t-icon name="layers" size="20px" />
          </div>
          <div v-if="project.id === projectStore.currentProjectId" class="project-card__badge">
            {{ t('project.current') }}
          </div>
        </div>
        <div class="project-card__name">{{ project.name }}</div>
        <div class="project-card__desc">{{ project.description || t('project.noDesc') }}</div>
        <div class="project-card__footer">
          <span class="project-card__date">{{ formatDate(project.created_at) }}</span>
          <t-button
            v-if="isAdmin"
            variant="text"
            theme="danger"
            size="small"
            @click.stop="handleDelete(project.id, project.name)"
          >
            {{ t('project.delete') }}
          </t-button>
        </div>
      </div>
    </div>

    <!-- Create dialog -->
    <t-dialog
      v-model:visible="showCreate"
      :header="t('project.createDialog')"
      :confirm-btn="{ content: t('common.create'), loading: creating }"
      @confirm="handleCreate"
      @close="showCreate = false"
      width="440px"
    >
      <t-form label-align="top">
        <t-form-item :label="t('project.nameLabel')" required>
          <t-input
            v-model="newName"
            :placeholder="t('project.namePlaceholder')"
            autofocus
            @keyup.enter="handleCreate"
          />
        </t-form-item>
        <t-form-item :label="t('project.descLabel')">
          <t-input v-model="newDesc" :placeholder="t('project.descPlaceholder')" />
        </t-form-item>
      </t-form>
    </t-dialog>

    <CreateFromTemplateDialog
      v-if="orgId"
      v-model:visible="showTemplateDialog"
      :org-id="orgId"
      @created="handleTemplateCreated"
    />
  </div>
</template>

<style scoped>
.view-page {
  padding: 32px;
  height: 100%;
  overflow-y: auto;
}

.page-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  margin-bottom: 32px;
}

.page-header__actions {
  display: flex;
  gap: 10px;
}

.page-title {
  font-size: 20px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  margin: 0 0 4px;
}

.page-subtitle {
  font-size: 13px;
  color: var(--ordo-text-secondary);
  margin: 0;
}

.state-center {
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 240px;
}

.project-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 16px;
}

.project-card {
  background: var(--ordo-bg-panel);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-lg);
  padding: 20px;
  cursor: pointer;
  transition: border-color 0.15s, box-shadow 0.15s;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.project-card:hover {
  border-color: var(--ordo-accent);
  box-shadow: var(--ordo-shadow-sm);
}

.project-card.is-active {
  border-color: var(--ordo-accent);
  background: var(--ordo-accent-bg);
}

.project-card__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 4px;
}

.project-card__icon {
  width: 36px;
  height: 36px;
  border-radius: var(--ordo-radius-md);
  background: var(--ordo-accent-bg);
  color: var(--ordo-accent);
  display: flex;
  align-items: center;
  justify-content: center;
}

.project-card.is-active .project-card__icon {
  background: var(--ordo-accent);
  color: #fff;
}

.project-card__badge {
  font-size: 11px;
  font-weight: 600;
  color: var(--ordo-accent);
  background: var(--ordo-accent-bg);
  padding: 2px 8px;
  border-radius: 99px;
}

.project-card__name {
  font-size: 15px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.project-card__desc {
  font-size: 12px;
  color: var(--ordo-text-secondary);
  line-height: 1.5;
  flex: 1;
}

.project-card__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 4px;
}

.project-card__date {
  font-size: 11px;
  color: var(--ordo-text-tertiary);
}
</style>
