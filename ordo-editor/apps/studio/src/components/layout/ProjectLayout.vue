<script setup lang="ts">
import { computed, onMounted, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { useProjectStore } from '@/stores/project';
import { useCatalogStore } from '@/stores/catalog';

const route = useRoute();
const router = useRouter();
const projectStore = useProjectStore();
const catalogStore = useCatalogStore();
const { t } = useI18n();

const orgId = computed(() => route.params.orgId as string);
const projectId = computed(() => route.params.projectId as string);

const base = computed(() => `/orgs/${orgId.value}/projects/${projectId.value}`);

const tabs = computed(() => [
  {
    value: 'editor',
    label: t('projectNav.rules'),
    icon: 'file-code',
    to: `${base.value}/editor`,
    active: route.path.includes('/editor'),
  },
  {
    value: 'facts',
    label: t('projectNav.facts'),
    icon: 'data',
    to: `${base.value}/facts`,
    active: route.path.endsWith('/facts'),
  },
  {
    value: 'concepts',
    label: t('projectNav.concepts'),
    icon: 'share',
    to: `${base.value}/concepts`,
    active: route.path.endsWith('/concepts'),
  },
  {
    value: 'contracts',
    label: t('projectNav.contracts'),
    icon: 'file-safety',
    to: `${base.value}/contracts`,
    active: route.path.endsWith('/contracts'),
  },
  {
    value: 'sub-rules',
    label: t('projectNav.subRules'),
    icon: 'git-branch',
    to: `${base.value}/sub-rules`,
    active: route.path.endsWith('/sub-rules'),
  },
  {
    value: 'tests',
    label: t('projectNav.tests'),
    icon: 'task-checked',
    to: `${base.value}/tests`,
    active: route.path.endsWith('/tests'),
  },
  {
    value: 'versions',
    label: t('projectNav.versions'),
    icon: 'history',
    to: `${base.value}/versions`,
    active: route.path.endsWith('/versions'),
  },
  {
    value: 'releases',
    label: t('projectNav.releases'),
    icon: 'cloud-upload',
    to: `${base.value}/releases`,
    active: route.path.includes('/releases') || route.path.endsWith('/deployments'),
  },
  {
    value: 'environments',
    label: t('projectNav.environments'),
    icon: 'server',
    to: `${base.value}/environments`,
    active: route.path.endsWith('/environments'),
  },
  {
    value: 'instances',
    label: t('projectNav.instances'),
    icon: 'internet',
    to: `${base.value}/instances`,
    active: route.path.endsWith('/instances'),
  },
  {
    value: 'settings',
    label: t('projectNav.settings'),
    icon: 'setting',
    to: `${base.value}/settings`,
    active: route.path.endsWith('/settings'),
  },
]);

// Load catalog when project changes
async function loadProject(pid: string) {
  if (!pid) return;
  // Ensure project is selected in store
  const existing = projectStore.projects.find((p) => p.id === pid);
  if (existing && (!projectStore.currentProject || projectStore.currentProject.id !== pid)) {
    await projectStore.selectProject(existing);
  }
  await catalogStore.fetchAll(pid);
}

onMounted(() => loadProject(projectId.value));
watch(projectId, (pid) => {
  if (pid) loadProject(pid);
});
</script>

<template>
  <div class="project-layout">
    <!-- Project tab navigation -->
    <nav class="project-nav">
      <div class="project-nav__tabs">
        <RouterLink
          v-for="tab in tabs"
          :key="tab.value"
          :to="tab.to"
          class="project-nav__tab"
          :class="{ 'is-active': tab.active }"
        >
          <t-icon :name="tab.icon" size="14px" />
          <span>{{ tab.label }}</span>
        </RouterLink>
      </div>
      <div class="project-nav__project-name">
        <t-icon name="layers" size="12px" style="opacity: 0.5" />
        {{ projectStore.currentProject?.name ?? '...' }}
      </div>
    </nav>

    <!-- Routed child view -->
    <div class="project-body">
      <RouterView v-slot="{ Component }">
        <Transition name="page-fade" mode="out-in">
          <component :is="Component" :key="route.path" />
        </Transition>
      </RouterView>
    </div>
  </div>
</template>

<style scoped>
.project-layout {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.project-nav {
  height: 40px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: var(--ordo-bg-panel);
  border-bottom: 1px solid var(--ordo-border-color);
  padding: 0 12px 0 0;
}

.project-nav__tabs {
  display: flex;
  align-items: stretch;
  height: 100%;
}

.project-nav__tab {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 14px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
  text-decoration: none;
  border-bottom: 2px solid transparent;
  transition:
    color 0.1s,
    border-color 0.1s;
  white-space: nowrap;
}

.project-nav__tab:hover {
  color: var(--ordo-text-primary);
  background: var(--ordo-hover-bg);
}

.project-nav__tab.is-active {
  color: var(--ordo-accent);
  border-bottom-color: var(--ordo-accent);
  background: transparent;
}

.project-nav__project-name {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  color: var(--ordo-text-tertiary);
  font-family: 'JetBrains Mono', monospace;
  max-width: 160px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.project-body {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
</style>
