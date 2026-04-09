<script setup lang="ts">
import { onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useAuthStore } from '@/stores/auth'
import { useOrgStore } from '@/stores/org'
import { useProjectStore } from '@/stores/project'
import { useDashboardStore } from '@/stores/dashboard'

const { t } = useI18n()
const router = useRouter()
const auth = useAuthStore()
const orgStore = useOrgStore()
const projectStore = useProjectStore()
const dash = useDashboardStore()

onMounted(() => dash.fetchDashboardData())

function openProject(projectId: string) {
  const orgId = orgStore.currentOrg?.id
  if (orgId) router.push(`/orgs/${orgId}/projects/${projectId}/editor`)
}

function goToProjects() {
  const orgId = orgStore.currentOrg?.id
  if (orgId) router.push(`/orgs/${orgId}/projects`)
}

async function createProject() {
  const orgId = orgStore.currentOrg?.id
  if (orgId) router.push(`/orgs/${orgId}/projects`)
}
</script>

<template>
  <div class="dashboard">
    <!-- Header -->
    <div class="dashboard-header">
      <div class="dashboard-header__title">
        <h1>{{ t('dashboard.welcome', { name: auth.user?.display_name ?? '' }) }}</h1>
      </div>
      <t-button theme="primary" @click="createProject">
        <t-icon name="add" />
        {{ t('dashboard.newProject') }}
      </t-button>
    </div>

    <!-- Stats row -->
    <div class="stats-row">
      <t-card class="stat-card" :bordered="false">
        <template v-if="dash.loading">
          <t-skeleton theme="paragraph" animation="gradient" :row-col="[{ width: '60%' }, { width: '40%' }]" />
        </template>
        <template v-else>
          <t-statistic :title="t('dashboard.statOrgs')" :value="dash.totalOrgs">
            <template #prefix><t-icon name="home" style="color: var(--ordo-accent)" /></template>
          </t-statistic>
        </template>
      </t-card>

      <t-card class="stat-card" :bordered="false">
        <template v-if="dash.loading">
          <t-skeleton theme="paragraph" animation="gradient" :row-col="[{ width: '60%' }, { width: '40%' }]" />
        </template>
        <template v-else>
          <t-statistic :title="t('dashboard.statProjects')" :value="dash.totalProjects">
            <template #prefix><t-icon name="layers" style="color: var(--ordo-accent)" /></template>
          </t-statistic>
        </template>
      </t-card>

      <t-card class="stat-card" :bordered="false">
        <template v-if="dash.loading">
          <t-skeleton theme="paragraph" animation="gradient" :row-col="[{ width: '60%' }, { width: '40%' }]" />
        </template>
        <template v-else>
          <t-statistic :title="t('dashboard.statRulesets')" :value="dash.totalRulesets">
            <template #prefix><t-icon name="file-code" style="color: var(--ordo-accent)" /></template>
          </t-statistic>
        </template>
      </t-card>
    </div>

    <!-- Recent projects -->
    <div class="section">
      <div class="section-header">
        <h2 class="section-title">{{ t('dashboard.recentProjects') }}</h2>
        <t-button variant="text" size="small" @click="goToProjects">
          {{ t('dashboard.viewAll') }} →
        </t-button>
      </div>

      <template v-if="dash.loading">
        <div class="project-grid">
          <t-card v-for="i in 4" :key="i" class="project-card" :bordered="false">
            <t-skeleton theme="paragraph" animation="gradient" :row-col="[{ width: '70%' }, { width: '50%' }, { width: '30%' }]" />
          </t-card>
        </div>
      </template>

      <template v-else-if="dash.recentProjects.length === 0">
        <t-card :bordered="false" class="empty-card">
          <t-empty :description="t('dashboard.noRecentProjectsDesc')">
            <template #image><t-icon name="layers" size="48px" style="color: var(--ordo-text-tertiary)" /></template>
          </t-empty>
        </t-card>
      </template>

      <template v-else>
        <div class="project-grid">
          <t-card
            v-for="project in dash.recentProjects"
            :key="project.id"
            class="project-card card-hover"
            :bordered="false"
            @click="openProject(project.id)"
          >
            <div class="project-card__icon">
              <t-icon name="layers" size="20px" />
            </div>
            <div class="project-card__body">
              <div class="project-card__name">{{ project.name }}</div>
              <div class="project-card__desc">{{ project.description || t('project.noDesc') }}</div>
              <div class="project-card__org">
                <t-tag size="small" theme="default" variant="light">{{ orgStore.currentOrg?.name }}</t-tag>
              </div>
            </div>
          </t-card>
        </div>
      </template>
    </div>

    <!-- Activity -->
    <div class="section">
      <div class="section-header">
        <h2 class="section-title">{{ t('dashboard.activityTitle') }}</h2>
      </div>
      <t-card :bordered="false" class="activity-card">
        <t-empty :description="t('dashboard.activityPlaceholder')">
          <template #image><t-icon name="history" size="48px" style="color: var(--ordo-text-tertiary)" /></template>
        </t-empty>
      </t-card>
    </div>
  </div>
</template>

<style scoped>
.dashboard {
  padding: 32px;
  overflow-y: auto;
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 28px;
}

.dashboard-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.dashboard-header__title h1 {
  margin: 0;
  font-size: 22px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.stats-row {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 16px;
}

.stat-card {
  background: var(--ordo-bg-panel);
  border-radius: 8px;
  padding: 4px 0;
}

.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 14px;
}

.section-title {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.project-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 14px;
}

.project-card {
  background: var(--ordo-bg-panel);
  border-radius: 8px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.project-card__icon {
  width: 36px;
  height: 36px;
  border-radius: 8px;
  background: rgba(var(--ordo-accent-rgb, 99, 102, 241), 0.12);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--ordo-accent);
  flex-shrink: 0;
}

.project-card__body {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.project-card__name {
  font-size: 13px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.project-card__desc {
  font-size: 12px;
  color: var(--ordo-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.project-card__org {
  margin-top: 4px;
}

.empty-card,
.activity-card {
  background: var(--ordo-bg-panel);
  border-radius: 8px;
  padding: 20px 0;
}
</style>
