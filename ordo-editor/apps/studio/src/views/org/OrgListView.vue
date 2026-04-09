<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useOrgStore } from '@/stores/org'
import { useProjectStore } from '@/stores/project'
import { MessagePlugin } from 'tdesign-vue-next'

const router = useRouter()
const orgStore = useOrgStore()
const projectStore = useProjectStore()
const { t } = useI18n()

const showCreate = ref(false)
const creating = ref(false)
const newOrgName = ref('')
const newOrgDesc = ref('')

async function handleCreate() {
  if (!newOrgName.value.trim()) {
    MessagePlugin.warning(t('org.nameRequired'))
    return
  }
  creating.value = true
  try {
    await orgStore.createOrg(newOrgName.value.trim(), newOrgDesc.value || undefined)
    showCreate.value = false
    newOrgName.value = ''
    newOrgDesc.value = ''
    MessagePlugin.success(t('org.createSuccess'))
  } catch (e: any) {
    MessagePlugin.error(e.message)
  } finally {
    creating.value = false
  }
}

async function selectAndGo(orgId: string) {
  await orgStore.selectOrg(orgId)
  await projectStore.fetchProjects(orgId)
  router.push(`/orgs/${orgId}/projects`)
}
</script>

<template>
  <div class="view-page">
    <div class="page-header">
      <div>
        <h2 class="page-title">{{ t('org.title') }}</h2>
        <p class="page-subtitle">{{ t('org.subtitle') }}</p>
      </div>
      <t-button theme="primary" @click="showCreate = true">
        <t-icon name="add" />
        {{ t('org.new') }}
      </t-button>
    </div>

    <!-- Loading -->
    <div v-if="orgStore.loading" class="org-skeleton-list">
      <t-skeleton v-for="i in 3" :key="i" theme="paragraph" animation="gradient"
        :row-col="[{ width: '40%' }, { width: '60%' }, { width: '30%' }]" />
    </div>

    <!-- Empty -->
    <div v-else-if="orgStore.orgs.length === 0" class="state-center">
      <t-empty :title="t('org.emptyTitle')" :description="t('org.emptyDesc')">
        <template #action>
          <t-button theme="primary" @click="showCreate = true">{{ t('org.createBtn') }}</t-button>
        </template>
      </t-empty>
    </div>

    <!-- Grid -->
    <div v-else class="org-grid">
      <div
        v-for="org in orgStore.orgs"
        :key="org.id"
        class="org-card"
        @click="selectAndGo(org.id)"
      >
        <div class="org-card__icon">
          {{ org.name[0]?.toUpperCase() }}
        </div>
        <div class="org-card__info">
          <div class="org-card__name">{{ org.name }}</div>
          <div class="org-card__meta">{{ t('org.memberCount', { count: org.member_count }) }}</div>
        </div>
        <t-icon name="chevron-right" class="org-card__arrow" />
      </div>
    </div>

    <!-- Create dialog -->
    <t-dialog
      v-model:visible="showCreate"
      :header="t('org.createDialog')"
      :confirm-btn="{ content: t('common.create'), loading: creating }"
      @confirm="handleCreate"
      @close="showCreate = false"
      width="440px"
    >
      <t-form label-align="top">
        <t-form-item :label="t('org.nameLabel')" required>
          <t-input
            v-model="newOrgName"
            :placeholder="t('org.namePlaceholder')"
            autofocus
            @keyup.enter="handleCreate"
          />
        </t-form-item>
        <t-form-item :label="t('org.descLabel')">
          <t-input v-model="newOrgDesc" :placeholder="t('org.descPlaceholder')" />
        </t-form-item>
      </t-form>
    </t-dialog>
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

.org-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: 16px;
}

.org-card {
  background: var(--ordo-bg-panel);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-lg);
  padding: 20px;
  display: flex;
  align-items: center;
  gap: 16px;
  cursor: pointer;
  transition: border-color 0.15s, box-shadow 0.15s;
}

.org-card:hover {
  border-color: var(--ordo-accent);
  box-shadow: var(--ordo-shadow-sm);
}

.org-card__icon {
  width: 44px;
  height: 44px;
  border-radius: var(--ordo-radius-lg);
  background: var(--ordo-accent-bg);
  color: var(--ordo-accent);
  font-size: 18px;
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.org-card__info {
  flex: 1;
  min-width: 0;
}

.org-card__name {
  font-size: 15px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  margin-bottom: 2px;
}

.org-card__meta {
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.org-card__arrow {
  color: var(--ordo-text-tertiary);
  flex-shrink: 0;
}
</style>
