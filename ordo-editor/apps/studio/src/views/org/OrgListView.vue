<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useAuthStore } from '@/stores/auth'
import { useOrgStore } from '@/stores/org'
import { useSystemStore } from '@/stores/system'
import { useProjectStore } from '@/stores/project'
import { MessagePlugin } from 'tdesign-vue-next'

const router = useRouter()
const auth = useAuthStore()
const orgStore = useOrgStore()
const systemStore = useSystemStore()
const projectStore = useProjectStore()
const { t } = useI18n()

// Dialog state
const showCreate = ref(false)
const creating = ref(false)
const newOrgName = ref('')
const newOrgDesc = ref('')
// When set, dialog creates a sub-org under this parent id
const createParentId = ref<string | null>(null)

const rootOrgs = computed(() => orgStore.orgs.filter((o) => o.depth === 0))

function subOrgsOf(parentId: string) {
  return orgStore.orgs.filter((o) => o.parent_org_id === parentId)
}

function isAdmin(orgId: string): boolean {
  if (!auth.user) return false
  const org = orgStore.orgs.find((o) => o.id === orgId)
  if (!org) return false
  // Check via currentOrg members if this is the currently loaded org
  if (orgStore.currentOrg?.id === orgId) {
    return orgStore.canAdmin(auth.user.id)
  }
  return false
}

onMounted(async () => {
  await systemStore.fetchConfig()
  // For each root org the user has loaded, pre-fetch sub-orgs
  for (const org of rootOrgs.value) {
    if (org.child_count > 0) {
      await orgStore.fetchSubOrgs(org.id)
    }
  }
})

function openCreateRoot() {
  createParentId.value = null
  newOrgName.value = ''
  newOrgDesc.value = ''
  showCreate.value = true
}

function openCreateSubOrg(parentId: string) {
  createParentId.value = parentId
  newOrgName.value = ''
  newOrgDesc.value = ''
  showCreate.value = true
}

async function handleCreate() {
  if (!newOrgName.value.trim()) {
    MessagePlugin.warning(t('org.nameRequired'))
    return
  }
  creating.value = true
  try {
    if (createParentId.value) {
      await orgStore.createSubOrg(createParentId.value, newOrgName.value.trim(), newOrgDesc.value || undefined)
      MessagePlugin.success(t('org.createSubOrgSuccess'))
    } else {
      await orgStore.createOrg(newOrgName.value.trim(), newOrgDesc.value || undefined)
      MessagePlugin.success(t('org.createSuccess'))
    }
    showCreate.value = false
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
      <t-button
        v-if="systemStore.allowOrgCreation"
        theme="primary"
        @click="openCreateRoot"
      >
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
    <div v-else-if="rootOrgs.length === 0" class="state-center">
      <t-empty :title="t('org.emptyTitle')" :description="t('org.emptyDesc')">
        <template v-if="systemStore.allowOrgCreation" #action>
          <t-button theme="primary" @click="openCreateRoot">{{ t('org.createBtn') }}</t-button>
        </template>
      </t-empty>
    </div>

    <!-- Hierarchy list -->
    <div v-else class="org-list">
      <div v-for="root in rootOrgs" :key="root.id" class="org-group">

        <!-- Root org card -->
        <div class="org-card org-card--root" @click="selectAndGo(root.id)">
          <div class="org-card__icon org-card__icon--root">
            {{ root.name[0]?.toUpperCase() }}
          </div>
          <div class="org-card__info">
            <div class="org-card__name">{{ root.name }}</div>
            <div class="org-card__meta">
              {{ t('org.memberCount', { count: root.member_count }) }}
              <span v-if="root.child_count > 0" class="meta-sep">·</span>
              <span v-if="root.child_count > 0">
                {{ t('org.subOrgCount', { count: root.child_count }) }}
              </span>
            </div>
          </div>
          <div class="org-card__actions" @click.stop>
            <t-button
              size="small"
              variant="outline"
              @click="openCreateSubOrg(root.id)"
            >
              <t-icon name="add" />
              {{ t('org.addSubOrg') }}
            </t-button>
          </div>
          <t-icon name="chevron-right" class="org-card__arrow" />
        </div>

        <!-- Sub-org cards (indented) -->
        <div v-if="subOrgsOf(root.id).length > 0" class="suborg-list">
          <div
            v-for="sub in subOrgsOf(root.id)"
            :key="sub.id"
            class="org-card org-card--sub"
            @click="selectAndGo(sub.id)"
          >
            <div class="suborg-indent">
              <div class="suborg-connector"></div>
            </div>
            <div class="org-card__icon org-card__icon--sub">
              {{ sub.name[0]?.toUpperCase() }}
            </div>
            <div class="org-card__info">
              <div class="org-card__name">{{ sub.name }}</div>
              <div class="org-card__meta">{{ t('org.memberCount', { count: sub.member_count }) }}</div>
            </div>
            <t-icon name="chevron-right" class="org-card__arrow" />
          </div>
        </div>

      </div>
    </div>

    <!-- Create dialog (root or sub-org) -->
    <t-dialog
      v-model:visible="showCreate"
      :header="createParentId ? t('org.createSubOrgDialog') : t('org.createDialog')"
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

/* ── Org list ──────────────────────────────────────────────────────────────── */
.org-list {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.org-group {
  display: flex;
  flex-direction: column;
  gap: 0;
}

.org-card {
  background: var(--ordo-bg-panel);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-lg);
  padding: 16px 20px;
  display: flex;
  align-items: center;
  gap: 14px;
  cursor: pointer;
  transition: border-color 0.15s, box-shadow 0.15s;
}

.org-card:hover {
  border-color: var(--ordo-accent);
  box-shadow: var(--ordo-shadow-sm);
}

.org-card--root {
  border-radius: var(--ordo-radius-lg);
}

/* When sub-orgs exist, square off root card bottom corners */
.org-group:has(.suborg-list) .org-card--root {
  border-bottom-left-radius: 0;
  border-bottom-right-radius: 0;
  border-bottom-color: transparent;
}

.org-card__icon {
  width: 40px;
  height: 40px;
  border-radius: var(--ordo-radius-lg);
  font-size: 16px;
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.org-card__icon--root {
  background: var(--ordo-accent-bg);
  color: var(--ordo-accent);
}

.org-card__icon--sub {
  background: #f0fdf4;
  color: #16a34a;
  width: 34px;
  height: 34px;
  font-size: 13px;
}

.org-card__info {
  flex: 1;
  min-width: 0;
}

.org-card__name {
  font-size: 14px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  margin-bottom: 2px;
}

.org-card__meta {
  font-size: 12px;
  color: var(--ordo-text-secondary);
  display: flex;
  align-items: center;
  gap: 4px;
}

.meta-sep {
  color: var(--ordo-text-tertiary, #9ca3af);
}

.org-card__actions {
  flex-shrink: 0;
}

.org-card__arrow {
  color: var(--ordo-text-tertiary);
  flex-shrink: 0;
}

/* ── Sub-org list ──────────────────────────────────────────────────────────── */
.suborg-list {
  border: 1px solid var(--ordo-border-color);
  border-top: none;
  border-radius: 0 0 var(--ordo-radius-lg) var(--ordo-radius-lg);
  overflow: hidden;
  background: var(--ordo-bg-panel);
}

.org-card--sub {
  border: none;
  border-top: 1px solid var(--ordo-border-color);
  border-radius: 0;
  padding: 12px 20px 12px 16px;
  background: #fafaf8;
}

.org-card--sub:first-child {
  border-top: none;
}

.org-card--sub:hover {
  background: var(--ordo-bg-panel);
  border-color: transparent;
  box-shadow: none;
}

.suborg-indent {
  display: flex;
  align-items: center;
  width: 20px;
  flex-shrink: 0;
}

.suborg-connector {
  width: 12px;
  height: 12px;
  border-left: 2px solid var(--ordo-border-color);
  border-bottom: 2px solid var(--ordo-border-color);
  border-bottom-left-radius: 4px;
  margin-left: 4px;
}

/* ── Skeleton ──────────────────────────────────────────────────────────────── */
.org-skeleton-list {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
</style>
