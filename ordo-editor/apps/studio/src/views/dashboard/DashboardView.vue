<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { useAuthStore } from '@/stores/auth';
import { useDashboardStore } from '@/stores/dashboard';
import { useOrgStore } from '@/stores/org';
import { useProjectStore } from '@/stores/project';
import { useTemplateStore } from '@/stores/template';
import { serverApi } from '@/api/platform-client';
import type { Project, ServerInfo } from '@/api/types';
import CreateFromTemplateDialog from '@/components/project/CreateFromTemplateDialog.vue';

const router = useRouter();
const { t, locale } = useI18n();
const auth = useAuthStore();
const dash = useDashboardStore();
const orgStore = useOrgStore();
const projectStore = useProjectStore();
const templateStore = useTemplateStore();

const showTemplateDialog = ref(false);
const serverLoading = ref(false);
const servers = ref<ServerInfo[]>([]);

const currentOrgId = computed(() => orgStore.currentOrg?.id ?? '');
const recentProjects = computed(() => dash.recentProjects);
const onlineServers = computed(
  () => servers.value.filter((server) => server.status === 'online').length
);
const offlineServers = computed(
  () => servers.value.filter((server) => server.status !== 'online').length
);

const workspaceStats = computed(() => [
  {
    label: t('dashboard.statProjects'),
    value: dash.totalProjects,
    hint: t('dashboard.projectHint'),
  },
  {
    label: t('dashboard.statRulesets'),
    value: dash.totalRulesets,
    hint: t('dashboard.rulesetHint'),
  },
  {
    label: t('dashboard.statOrgs'),
    value: dash.totalOrgs,
    hint: t('dashboard.orgHint'),
  },
]);

const isAdmin = computed(() => {
  if (!auth.user) return false;
  return orgStore.canAdmin(auth.user.id);
});

onMounted(async () => {
  await dash.fetchDashboardData();
  await Promise.all([loadTemplates(), loadServers()]);
});

async function loadTemplates() {
  await templateStore.fetchTemplates();
}

async function loadServers() {
  if (!auth.token) return;
  serverLoading.value = true;
  try {
    servers.value = await serverApi.list(auth.token);
  } catch {
    servers.value = [];
  } finally {
    serverLoading.value = false;
  }
}

async function openProject(projectId: string) {
  const orgId = orgStore.currentOrg?.id;
  if (!orgId) return;
  const project =
    projectStore.projects.find((item) => item.id === projectId) ??
    recentProjects.value.find((item) => item.id === projectId);
  if (project) {
    await projectStore.selectProject(project);
  }
  router.push(`/orgs/${orgId}/projects/${projectId}/editor`);
}

function goToProjects(query?: Record<string, string>) {
  const orgId = orgStore.currentOrg?.id;
  if (orgId) {
    router.push({ path: `/orgs/${orgId}/projects`, query });
    return;
  }
  router.push('/orgs');
}

function goToMarketplace() {
  router.push('/marketplace');
}

function goToCreateOrg() {
  router.push('/orgs');
}

function goToMembers() {
  const orgId = orgStore.currentOrg?.id;
  if (orgId) {
    router.push(`/orgs/${orgId}/members`);
    return;
  }
  router.push('/orgs');
}

function openServerRegistry() {
  router.push('/servers');
}

async function handleTemplateCreated(project: Project) {
  projectStore.projects.unshift(project);
  await projectStore.selectProject(project);
  router.push(`/orgs/${project.org_id}/projects/${project.id}/editor`);
}

function formatDate(iso: string) {
  return new Date(iso).toLocaleDateString(
    locale.value === 'zh-TW' ? 'zh-TW' : locale.value === 'zh-CN' ? 'zh-CN' : 'en-US',
    { year: 'numeric', month: 'short', day: 'numeric' }
  );
}
</script>

<template>
  <div class="workspace-page">
    <div class="workspace-toolbar">
      <div>
        <h2 class="workspace-title">{{ t('dashboard.workspaceTitle') }}</h2>
      </div>

      <div class="workspace-toolbar__actions">
        <t-button
          v-if="isAdmin && currentOrgId"
          variant="outline"
          @click="showTemplateDialog = true"
        >
          <t-icon name="gesture-applause" />
          {{ t('template.fromTemplate') }}
        </t-button>
        <t-button theme="primary" @click="currentOrgId ? goToProjects() : goToCreateOrg()">
          <t-icon name="add" />
          {{ currentOrgId ? t('dashboard.newProject') : t('org.new') }}
        </t-button>
      </div>
    </div>

    <div class="workspace-stats">
      <div v-for="item in workspaceStats" :key="item.label" class="stat-tile">
        <div class="stat-tile__value">{{ item.value }}</div>
        <div class="stat-tile__label">{{ item.label }}</div>
        <div class="stat-tile__hint">{{ item.hint }}</div>
      </div>
    </div>

    <div class="workspace-grid">
      <section class="panel panel--main">
        <div class="panel__header">
          <div>
            <h3>{{ t('dashboard.recentProjects') }}</h3>
            <p>{{ t('dashboard.recentProjectsHint') }}</p>
          </div>
          <t-button variant="text" @click="goToProjects()">
            {{ t('dashboard.viewAll') }}
          </t-button>
        </div>

        <div v-if="dash.loading" class="panel-list">
          <div v-for="i in 5" :key="i" class="project-row project-row--loading">
            <t-skeleton
              theme="paragraph"
              animation="gradient"
              :row-col="[{ width: '42%' }, { width: '18%' }, { width: '16%' }]"
            />
          </div>
        </div>

        <div v-else-if="recentProjects.length === 0" class="panel-empty">
          <strong>{{ t('dashboard.noRecentProjects') }}</strong>
          <p>{{ t('dashboard.noRecentProjectsDesc') }}</p>
          <t-button theme="primary" size="small" @click="goToProjects()">
            {{ t('dashboard.newProject') }}
          </t-button>
        </div>

        <div v-else class="panel-list">
          <button
            v-for="project in recentProjects"
            :key="project.id"
            class="project-row"
            @click="openProject(project.id)"
          >
            <div class="project-row__main">
              <strong>{{ project.name }}</strong>
              <span>{{ project.description || t('project.noDesc') }}</span>
            </div>
            <div class="project-row__meta">{{ formatDate(project.created_at) }}</div>
            <div class="project-row__meta project-row__meta--tag">
              {{ orgStore.currentOrg?.name ?? t('shell.noOrg') }}
            </div>
          </button>
        </div>
      </section>

      <div class="workspace-rail">
        <section class="panel">
          <div class="panel__header">
            <div>
              <h3>{{ t('dashboard.templatesTitle') }}</h3>
              <p>{{ t('dashboard.templatesHint') }}</p>
            </div>
          </div>

          <div class="stack-metric">
            <strong>{{ templateStore.templates.length }}</strong>
            <span>{{ t('dashboard.templateCountLabel') }}</span>
          </div>

          <div class="panel__actions">
            <t-button
              v-if="isAdmin && currentOrgId"
              variant="outline"
              size="small"
              @click="showTemplateDialog = true"
            >
              {{ t('template.fromTemplate') }}
            </t-button>
            <t-button variant="text" size="small" @click="goToMarketplace">
              {{ t('marketplace.title') }}
            </t-button>
          </div>
        </section>

        <section class="panel">
          <div class="panel__header">
            <div>
              <h3>{{ t('dashboard.serverFleetTitle') }}</h3>
              <p>{{ t('dashboard.serverFleetHint') }}</p>
            </div>
          </div>

          <div v-if="serverLoading" class="stack-loading">
            <t-skeleton
              theme="paragraph"
              animation="gradient"
              :row-col="[{ width: '70%' }, { width: '55%' }]"
            />
          </div>

          <template v-else>
            <div class="fleet-summary">
              <div class="fleet-summary__item">
                <strong>{{ onlineServers }}</strong>
                <span>{{ t('dashboard.serverOnline') }}</span>
              </div>
              <div class="fleet-summary__item">
                <strong>{{ offlineServers }}</strong>
                <span>{{ t('dashboard.serverOffline') }}</span>
              </div>
            </div>

            <div class="panel__actions">
              <t-button variant="text" size="small" @click="openServerRegistry">
                {{ t('dashboard.openRegistry') }}
              </t-button>
            </div>
          </template>
        </section>

        <section class="panel">
          <div class="panel__header">
            <div>
              <h3>{{ t('dashboard.orgSummaryTitle') }}</h3>
              <p>{{ t('dashboard.orgSummaryHint') }}</p>
            </div>
          </div>

          <div class="stack-metric">
            <strong>{{ orgStore.members.length }}</strong>
            <span>{{ t('dashboard.memberCountLabel') }}</span>
          </div>

          <div class="panel__actions">
            <t-button variant="text" size="small" @click="goToMembers">
              {{ t('dashboard.manageMembers') }}
            </t-button>
          </div>
        </section>
      </div>
    </div>

    <CreateFromTemplateDialog
      v-if="currentOrgId"
      v-model:visible="showTemplateDialog"
      :org-id="currentOrgId"
      @created="handleTemplateCreated"
    />
  </div>
</template>

<style scoped>
.workspace-page {
  height: 100%;
  overflow-y: auto;
  padding: 22px;
  display: flex;
  flex-direction: column;
  gap: 18px;
}

.workspace-toolbar {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.workspace-toolbar__actions {
  display: flex;
  gap: 10px;
  flex-shrink: 0;
}

.workspace-title {
  margin: 0;
  font-size: 22px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.workspace-subtitle {
  margin: 6px 0 0;
  font-size: 13px;
  line-height: 1.5;
  color: var(--ordo-text-secondary);
}

.workspace-stats {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
}

.stat-tile,
.panel {
  background: var(--ordo-bg-panel);
  border: 1px solid var(--ordo-border-color);
  border-radius: 10px;
}

.stat-tile {
  padding: 16px;
}

.stat-tile__value {
  font-size: 24px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.stat-tile__label {
  margin-top: 6px;
  font-size: 12px;
  font-weight: 600;
  color: var(--ordo-text-secondary);
}

.stat-tile__hint {
  margin-top: 4px;
  font-size: 12px;
  color: var(--ordo-text-tertiary);
}

.workspace-grid {
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1.5fr) minmax(280px, 0.85fr);
  gap: 14px;
}

.workspace-rail {
  display: grid;
  gap: 14px;
  align-content: start;
}

.panel {
  padding: 16px;
}

.panel--main {
  min-height: 520px;
}

.panel__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 14px;
}

.panel__header h3 {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.panel__header p {
  margin: 4px 0 0;
  font-size: 12px;
  line-height: 1.45;
  color: var(--ordo-text-secondary);
}

.panel-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.project-row {
  width: 100%;
  min-height: 58px;
  padding: 0 14px;
  border: 1px solid var(--ordo-border-color);
  border-radius: 8px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 110px 140px;
  gap: 12px;
  align-items: center;
  text-align: left;
  cursor: pointer;
  background: var(--ordo-bg-panel);
}

.project-row:hover {
  border-color: var(--ordo-border-color);
  background: var(--ordo-bg-item-hover);
}

.project-row--loading {
  cursor: default;
}

.project-row__main {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.project-row__main strong {
  font-size: 13px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.project-row__main span {
  font-size: 12px;
  color: var(--ordo-text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.project-row__meta {
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.project-row__meta--tag {
  justify-self: start;
  padding: 6px 9px;
  border-radius: 999px;
  background: var(--ordo-bg-item-hover);
}

.panel-empty {
  min-height: 320px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  text-align: center;
}

.panel-empty strong {
  font-size: 15px;
  color: var(--ordo-text-primary);
}

.panel-empty p {
  margin: 0 0 8px;
  max-width: 280px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.stack-metric {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.stack-metric strong {
  font-size: 24px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.stack-metric span {
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.fleet-summary {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}

.fleet-summary__item {
  padding: 12px;
  border: 1px solid var(--ordo-border-color);
  border-radius: 8px;
  background: var(--ordo-bg-app);
}

.fleet-summary__item strong {
  display: block;
  font-size: 20px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.fleet-summary__item span {
  display: block;
  margin-top: 4px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.panel__actions {
  margin-top: 14px;
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.stack-loading {
  padding: 8px 0;
}

@media (max-width: 1180px) {
  .workspace-grid {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 820px) {
  .workspace-toolbar,
  .workspace-toolbar__actions {
    flex-direction: column;
    align-items: stretch;
  }

  .workspace-stats {
    grid-template-columns: 1fr;
  }

  .project-row {
    grid-template-columns: 1fr;
    padding: 12px 14px;
  }
}
</style>
