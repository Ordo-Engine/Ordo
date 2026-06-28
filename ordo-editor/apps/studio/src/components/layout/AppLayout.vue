<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { MessagePlugin } from 'tdesign-vue-next';
import platformLogo from '@/assets/platform-logo.png';
import { useAuthStore } from '@/stores/auth';
import { useOrgStore } from '@/stores/org';
import { useProjectStore } from '@/stores/project';
import { useServerStore } from '@/stores/server';
import { useNotificationStore } from '@/stores/notification';
import { usePersistentNotificationStore } from '@/stores/persistentNotifications';
import { i18n, LOCALE_OPTIONS, setLocale, type Locale } from '@/i18n';

const router = useRouter();
const route = useRoute();
const auth = useAuthStore();
const orgStore = useOrgStore();
const projectStore = useProjectStore();
const serverStore = useServerStore();
const notifStore = useNotificationStore();
const persistentNotifStore = usePersistentNotificationStore();
const { t } = useI18n();

const currentLocale = computed(() => (i18n.global.locale as any).value as Locale);
const currentLocaleLabel = computed(
  () => LOCALE_OPTIONS.find((option) => option.value === currentLocale.value)?.label ?? 'EN'
);

const currentOrgId = computed(() => orgStore.currentOrg?.id ?? '');
const currentProjectId = computed(() =>
  typeof route.params.projectId === 'string' ? route.params.projectId : null
);

const currentProject = computed(() =>
  currentProjectId.value
    ? projectStore.projects.find((project) => project.id === currentProjectId.value) ?? null
    : projectStore.currentProject
);

// ── Topbar dropdown state ────────────────────────────────────────────────────

const projectPickerOpen = ref(false);
const orgSwitcherOpen = ref(false);
const healthOpen = ref(false);
const notifOpen = ref(false);
const orgSwitcherRef = ref<HTMLElement | null>(null);
const projectPickerRef = ref<HTMLElement | null>(null);
const healthRef = ref<HTMLElement | null>(null);
const notifRef = ref<HTMLElement | null>(null);
const showCreateOrg = ref(false);
const creatingOrg = ref(false);
const newOrgName = ref('');
const newOrgDesc = ref('');

function onDocumentClick(e: MouseEvent) {
  const target = e.target as Node;
  if (orgSwitcherRef.value && !orgSwitcherRef.value.contains(target)) orgSwitcherOpen.value = false;
  if (projectPickerRef.value && !projectPickerRef.value.contains(target))
    projectPickerOpen.value = false;
  if (healthRef.value && !healthRef.value.contains(target)) healthOpen.value = false;
  if (notifRef.value && !notifRef.value.contains(target)) notifOpen.value = false;
}

// ── Server health ────────────────────────────────────────────────────────────

const boundServer = computed(() => {
  if (!currentProject.value?.server_id) return null;
  return serverStore.getById(currentProject.value.server_id) ?? null;
});

const boundServerStatus = computed(() => boundServer.value?.status ?? 'default');

function statusTheme(status: string) {
  if (status === 'online') return 'success';
  if (status === 'degraded') return 'warning';
  if (status === 'default') return 'default';
  return 'danger';
}

// ── Project picker ───────────────────────────────────────────────────────────

const projectOptions = computed(() =>
  projectStore.projects.map((p) => ({
    id: p.id,
    name: p.name,
    active: p.id === currentProject.value?.id,
  }))
);

function switchProject(projectId: string) {
  projectPickerOpen.value = false;
  navigate(`/orgs/${currentOrgId.value}/projects/${projectId}/editor`);
}

// ── Notifications ────────────────────────────────────────────────────────────

function formatNotifTime(date: Date) {
  const diff = Date.now() - date.getTime();
  if (diff < 60_000) return t('shell.justNow');
  if (diff < 3_600_000) return t('shell.minutesAgo', { n: Math.floor(diff / 60_000) });
  if (diff < 86_400_000) return t('shell.hoursAgo', { n: Math.floor(diff / 3_600_000) });
  return date.toLocaleDateString();
}

function notifIcon(type: string) {
  if (type === 'success') return 'check-circle';
  if (type === 'error') return 'close-circle';
  if (type === 'warning') return 'error-circle';
  return 'info-circle';
}

// ── Page info ────────────────────────────────────────────────────────────────

const pageInfo = computed(() => {
  const name = route.name?.toString();
  switch (name) {
    case 'dashboard':
      return {
        title: t('nav.dashboard'),
        subtitle: t('shell.dashboardSubtitle', {
          projects: projectStore.projects.length,
          members: orgStore.members.length,
          servers: serverStore.servers.filter((s) => s.status === 'online').length,
        }),
      };
    case 'projects':
      return { title: t('nav.projects'), subtitle: t('shell.projectsSubtitle') };
    case 'org-members':
      return { title: t('nav.members'), subtitle: t('shell.membersSubtitle') };
    case 'org-roles':
    case 'org-role-create':
    case 'org-role-edit':
      return { title: t('nav.roles'), subtitle: t('shell.rolesSubtitle') };
    case 'org-settings':
      return { title: t('nav.orgSettings'), subtitle: t('shell.orgSettingsSubtitle') };
    case 'settings':
      return { title: t('nav.settings'), subtitle: t('shell.settingsSubtitle') };
    case 'servers':
      return { title: t('settings.serverRegistry.title'), subtitle: t('shell.serversSubtitle') };
    case 'org-servers':
      return { title: t('settings.serverRegistry.title'), subtitle: t('shell.serversSubtitle') };
    case 'editor':
    case 'editor-ruleset':
      return {
        title: currentProject.value?.name ?? t('projectNav.rules'),
        subtitle: t('shell.editorSubtitle'),
      };
    case 'facts':
      return { title: t('projectNav.facts'), subtitle: t('facts.desc') };
    case 'concepts':
      return { title: t('projectNav.concepts'), subtitle: t('concepts.desc') };
    case 'contracts':
      return { title: t('projectNav.contracts'), subtitle: t('contracts.desc') };
    case 'project-sub-rules':
      return { title: t('projectNav.subRules'), subtitle: t('subRules.desc') };
    case 'tests':
      return { title: t('projectNav.tests'), subtitle: t('shell.testsSubtitle') };
    case 'versions':
      return { title: t('projectNav.versions'), subtitle: t('shell.versionsSubtitle') };
    case 'project-releases':
    case 'project-release-requests':
    case 'project-release-request-detail':
    case 'project-release-policies':
    case 'project-release-history':
      return { title: t('projectNav.releases'), subtitle: t('releaseCenter.subtitle') };
    case 'project-instances':
      return { title: t('projectNav.instances'), subtitle: t('projectInstances.bindingDesc') };
    case 'project-settings':
      return { title: t('projectNav.settings'), subtitle: t('projectSettings.serverBindingDesc') };
    case 'marketplace':
      return { title: t('marketplace.title'), subtitle: t('marketplace.subtitle') };
    case 'marketplace-detail':
      return { title: t('marketplace.detail'), subtitle: t('marketplace.subtitle') };
    default:
      return {
        title: t('nav.dashboard'),
        subtitle: t('shell.dashboardSubtitle', {
          projects: projectStore.projects.length,
          members: orgStore.members.length,
          servers: serverStore.servers.filter((s) => s.status === 'online').length,
        }),
      };
  }
});

// ── Nav items ────────────────────────────────────────────────────────────────

const workspaceItems = computed(() => {
  const projectTarget = currentOrgId.value ? `/orgs/${currentOrgId.value}/projects` : '/orgs';
  return [
    {
      value: 'dashboard',
      label: t('nav.dashboard'),
      icon: 'dashboard-1',
      active: route.path.startsWith('/dashboard'),
      action: () => navigate('/dashboard'),
    },
    {
      value: 'projects',
      label: t('nav.projects'),
      icon: 'layers',
      active: route.path.includes('/projects') && route.query.openTemplate !== '1',
      action: () => navigate(projectTarget),
    },
    {
      value: 'templates',
      label: t('template.fromTemplate'),
      icon: 'gesture-applause',
      active: route.path.includes('/projects') && route.query.openTemplate === '1',
      action: () => navigate(projectTarget, { openTemplate: '1' }),
    },
  ];
});

const orgItems = computed(() => {
  if (!currentOrgId.value) return [];
  return [
    {
      value: 'members',
      label: t('nav.members'),
      icon: 'usergroup',
      active: route.path.includes('/members'),
      action: () => navigate(`/orgs/${currentOrgId.value}/members`),
    },
    {
      value: 'roles',
      label: t('nav.roles'),
      icon: 'secured',
      active: route.path.includes('/roles'),
      action: () => navigate(`/orgs/${currentOrgId.value}/roles`),
    },
    {
      value: 'servers',
      label: t('settings.serverRegistry.title'),
      icon: 'internet',
      active: route.path.includes(`/orgs/${currentOrgId.value}/servers`),
      action: () => navigate(`/orgs/${currentOrgId.value}/servers`),
    },
    {
      value: 'orgSettings',
      label: t('nav.orgSettings'),
      icon: 'setting-1',
      active: route.path.includes(`/orgs/${currentOrgId.value}/settings`),
      action: () => navigate(`/orgs/${currentOrgId.value}/settings`),
    },
  ];
});

const systemItems = computed(() => [
  {
    value: 'settings',
    label: t('nav.settings'),
    icon: 'setting',
    active: route.path.startsWith('/settings'),
    action: () => navigate('/settings'),
  },
]);

// Org switcher list: root orgs first, then their sub-orgs indented below each parent
const orgOptions = computed(() => {
  const roots = orgStore.orgs.filter((o) => o.depth === 0);
  const result: { label: string; value: string; depth: number }[] = [];
  for (const root of roots) {
    result.push({ label: root.name, value: root.id, depth: 0 });
    const subs = orgStore.orgs.filter((o) => o.parent_org_id === root.id);
    for (const sub of subs) {
      result.push({ label: sub.name, value: sub.id, depth: 1 });
    }
  }
  return result;
});

// ── Lifecycle ────────────────────────────────────────────────────────────────

onMounted(async () => {
  await orgStore.fetchOrgs();
  if (orgStore.currentOrg) {
    await projectStore.fetchProjects(orgStore.currentOrg.id);
    persistentNotifStore.startPolling(orgStore.currentOrg.id);
  }
  await serverStore.fetchServers();
  document.addEventListener('click', onDocumentClick, true);
});

onUnmounted(() => {
  persistentNotifStore.stopPolling();
  document.removeEventListener('click', onDocumentClick, true);
});

watch(currentOrgId, async (id) => {
  if (id) {
    await serverStore.fetchServers();
    persistentNotifStore.stopPolling();
    persistentNotifStore.startPolling(id);
  }
});

// ── Helpers ──────────────────────────────────────────────────────────────────

function navigate(to: string, query?: Record<string, string>) {
  router.push(query ? { path: to, query } : to);
}

async function onOrgChange(value: string) {
  await orgStore.selectOrg(value);
  await projectStore.fetchProjects(value);
  orgSwitcherOpen.value = false;
  navigate(`/orgs/${value}/projects`);
}

function cycleLocale() {
  const locales = LOCALE_OPTIONS.map((option) => option.value);
  const index = locales.indexOf(currentLocale.value);
  setLocale(locales[(index + 1) % locales.length]);
}

function triggerGlobalCommandPalette() {
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', ctrlKey: true, bubbles: true }));
}

function handleLogout() {
  auth.logout();
  router.push('/login');
}

async function handleCreateOrg() {
  if (!newOrgName.value.trim()) {
    MessagePlugin.warning(t('org.nameRequired'));
    return;
  }
  creatingOrg.value = true;
  try {
    const org = await orgStore.createOrg(newOrgName.value.trim(), newOrgDesc.value || undefined);
    newOrgName.value = '';
    newOrgDesc.value = '';
    showCreateOrg.value = false;
    orgSwitcherOpen.value = false;
    await projectStore.fetchProjects(org.id);
    MessagePlugin.success(t('org.createSuccess'));
    navigate(`/orgs/${org.id}/projects`);
  } catch (error: any) {
    MessagePlugin.error(error.message);
  } finally {
    creatingOrg.value = false;
  }
}
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar">
      <div class="sidebar__brand">
        <div class="sidebar__logo" aria-hidden="true">
          <img :src="platformLogo" alt="" class="sidebar__logo-image" />
        </div>
        <div class="sidebar__brand-copy">
          <strong>Ordo</strong>
          <span>Studio</span>
        </div>
      </div>

      <div class="sidebar__org">
        <div class="sidebar__section-label">{{ t('shell.currentOrg') }}</div>
        <div class="sidebar__org-name">{{ orgStore.currentOrg?.name ?? t('shell.noOrg') }}</div>
      </div>

      <section class="sidebar__section">
        <div class="sidebar__section-label">{{ t('shell.workspace') }}</div>
        <button
          v-for="item in workspaceItems"
          :key="item.value"
          class="sidebar-link"
          :class="{ 'is-active': item.active }"
          @click="item.action"
        >
          <t-icon :name="item.icon" />
          <span>{{ item.label }}</span>
        </button>
      </section>

      <section v-if="orgItems.length" class="sidebar__section">
        <div class="sidebar__section-label">{{ t('shell.orgContext') }}</div>
        <button
          v-for="item in orgItems"
          :key="item.value"
          class="sidebar-link"
          :class="{ 'is-active': item.active }"
          @click="item.action"
        >
          <t-icon :name="item.icon" />
          <span>{{ item.label }}</span>
        </button>
      </section>

      <section class="sidebar__section sidebar__section--grow">
        <div class="sidebar__section-label">{{ t('shell.system') }}</div>
        <button
          v-for="item in systemItems"
          :key="item.value"
          class="sidebar-link"
          :class="{ 'is-active': item.active }"
          @click="item.action"
        >
          <t-icon :name="item.icon" />
          <span>{{ item.label }}</span>
        </button>
      </section>

      <div class="sidebar__footer">
        <button class="sidebar-link" @click="cycleLocale">
          <span class="sidebar-link__badge">{{ currentLocaleLabel }}</span>
          <span>{{ t('settings.languageLabel') }}</span>
        </button>

        <t-dropdown
          :options="[{ content: t('nav.logout'), value: 'logout', prefixIcon: 'logout' }]"
          placement="right-end"
          trigger="click"
          @click="handleLogout"
        >
          <div class="sidebar-user">
            <div class="sidebar-user__avatar">
              {{ auth.user?.display_name?.[0]?.toUpperCase() ?? '?' }}
            </div>
            <div class="sidebar-user__copy">
              <strong>{{ auth.user?.display_name ?? t('nav.userMenu') }}</strong>
              <span>{{ auth.user?.email }}</span>
            </div>
          </div>
        </t-dropdown>
      </div>
    </aside>

    <section class="main-shell">
      <header class="topbar">
        <div class="topbar__title">
          <h1>{{ pageInfo.title }}</h1>
          <p>{{ pageInfo.subtitle }}</p>
        </div>

        <div class="topbar__actions">
          <!-- Search -->
          <button class="command-trigger" @click="triggerGlobalCommandPalette">
            <t-icon name="search" />
            <span>{{ t('shell.searchPlaceholder') }}</span>
            <kbd>{{ t('shell.searchShortcut') }}</kbd>
          </button>

          <!-- Org switcher -->
          <div ref="orgSwitcherRef" class="topbar-pill-wrap">
            <button
              class="topbar-pill topbar-pill--org"
              :class="{ 'is-open': orgSwitcherOpen }"
              @click="orgSwitcherOpen = !orgSwitcherOpen"
            >
              <t-icon name="institution" size="13px" />
              <span class="topbar-pill__text">{{
                orgStore.currentOrg?.name ?? t('shell.noOrg')
              }}</span>
              <t-icon
                name="chevron-down"
                size="12px"
                class="topbar-pill__arrow"
                :class="{ 'is-flipped': orgSwitcherOpen }"
              />
            </button>

            <div v-show="orgSwitcherOpen" class="topbar-dropdown topbar-dropdown--orgs">
              <div class="topbar-dropdown__header">{{ t('shell.currentOrg') }}</div>
              <button
                v-for="org in orgOptions"
                :key="org.value"
                class="topbar-dropdown__item"
                :class="{ 'is-active': org.value === currentOrgId, 'is-suborg': org.depth > 0 }"
                @click="onOrgChange(org.value)"
              >
                <span
                  v-if="org.depth > 0"
                  class="suborg-indent-connector"
                  aria-hidden="true"
                ></span>
                <t-icon :name="org.depth > 0 ? 'root-list' : 'institution'" size="13px" />
                <span>{{ org.label }}</span>
                <t-icon
                  v-if="org.value === currentOrgId"
                  name="check"
                  size="13px"
                  class="topbar-dropdown__check"
                />
              </button>
              <div class="topbar-dropdown__footer">
                <button class="topbar-dropdown__action" @click="showCreateOrg = true">
                  <t-icon name="add" size="13px" />
                  <span>{{ t('org.new') }}</span>
                </button>
              </div>
            </div>
          </div>

          <!-- ── Project switcher ── -->
          <div
            v-if="currentOrgId && projectStore.projects.length > 0"
            ref="projectPickerRef"
            class="topbar-pill-wrap"
          >
            <button
              class="topbar-pill"
              :class="{ 'is-open': projectPickerOpen }"
              @click="projectPickerOpen = !projectPickerOpen"
            >
              <t-icon name="layers" size="13px" />
              <span class="topbar-pill__text">{{
                currentProject?.name ?? t('shell.noProject')
              }}</span>
              <t-icon
                name="chevron-down"
                size="12px"
                class="topbar-pill__arrow"
                :class="{ 'is-flipped': projectPickerOpen }"
              />
            </button>

            <div v-show="projectPickerOpen" class="topbar-dropdown topbar-dropdown--wide">
              <div class="topbar-dropdown__header">{{ t('shell.switchProject') }}</div>
              <button
                v-for="proj in projectOptions"
                :key="proj.id"
                class="topbar-dropdown__item"
                :class="{ 'is-active': proj.active }"
                @click="switchProject(proj.id)"
              >
                <t-icon name="layers" size="13px" />
                <span>{{ proj.name }}</span>
                <t-icon
                  v-if="proj.active"
                  name="check"
                  size="13px"
                  class="topbar-dropdown__check"
                />
              </button>
            </div>
          </div>

          <!-- ── Server health indicator ── -->
          <div v-if="currentProject" ref="healthRef" class="topbar-pill-wrap">
            <button
              class="topbar-pill"
              :class="{ 'is-open': healthOpen }"
              @click="healthOpen = !healthOpen"
            >
              <span class="health-dot" :class="`health-dot--${boundServerStatus}`" />
              <span class="topbar-pill__text">
                {{ boundServer?.name ?? t('shell.defaultEngine') }}
              </span>
            </button>

            <div v-show="healthOpen" class="topbar-dropdown topbar-dropdown--health">
              <div class="topbar-dropdown__header">{{ t('shell.engineStatus') }}</div>
              <template v-if="boundServer">
                <div class="health-row">
                  <span>{{ t('shell.healthStatus') }}</span>
                  <t-tag :theme="statusTheme(boundServer.status)" variant="light" size="small">
                    {{ boundServer.status }}
                  </t-tag>
                </div>
                <div class="health-row">
                  <span>{{ t('shell.healthVersion') }}</span>
                  <strong>{{ boundServer.version || '-' }}</strong>
                </div>
                <div class="health-row">
                  <span>URL</span>
                  <span class="health-url">{{ boundServer.url }}</span>
                </div>
                <div class="health-row">
                  <span>{{ t('shell.healthLastSeen') }}</span>
                  <strong>{{
                    boundServer.last_seen
                      ? new Date(boundServer.last_seen).toLocaleString()
                      : t('shell.never')
                  }}</strong>
                </div>
                <button
                  class="health-detail-link"
                  @click="
                    healthOpen = false;
                    navigate(`/orgs/${currentOrgId}/servers`);
                  "
                >
                  {{ t('shell.openRegistry') }} →
                </button>
              </template>
              <div v-else class="health-default-msg">{{ t('shell.defaultEngineHint') }}</div>
            </div>
          </div>

          <!-- ── Notification bell ── -->
          <div ref="notifRef" class="topbar-pill-wrap">
            <button
              class="topbar-pill topbar-pill--icon"
              :class="{
                'is-open': notifOpen,
                'has-unread': persistentNotifStore.unreadCount > 0 || notifStore.unreadCount > 0,
              }"
              @click="notifOpen = !notifOpen"
            >
              <t-icon name="notification" size="16px" />
              <span
                v-if="persistentNotifStore.unreadCount > 0 || notifStore.unreadCount > 0"
                class="notif-badge"
              >
                {{
                  persistentNotifStore.unreadCount + notifStore.unreadCount > 99
                    ? '99+'
                    : persistentNotifStore.unreadCount + notifStore.unreadCount
                }}
              </span>
            </button>

            <div v-show="notifOpen" class="topbar-dropdown topbar-dropdown--notif">
              <div class="topbar-dropdown__header">
                <span>{{ t('shell.notifications') }}</span>
                <button
                  v-if="notifStore.unreadCount > 0"
                  class="notif-mark-read"
                  @click="notifStore.markAllRead()"
                >
                  {{ t('shell.markAllRead') }}
                </button>
              </div>

              <div v-if="notifStore.notifications.length === 0" class="notif-empty">
                <t-icon name="notification" size="24px" style="opacity: 0.25" />
                <span>{{ t('shell.noNotifications') }}</span>
              </div>

              <div v-else class="notif-list">
                <div
                  v-for="n in notifStore.notifications"
                  :key="n.id"
                  class="notif-item"
                  :class="{ 'notif-item--unread': !n.read }"
                >
                  <span class="notif-icon" :class="`notif-icon--${n.type}`">
                    <t-icon :name="notifIcon(n.type)" size="14px" />
                  </span>
                  <div class="notif-content">
                    <div class="notif-title">{{ n.title }}</div>
                    <div v-if="n.message" class="notif-message">{{ n.message }}</div>
                    <div class="notif-time">{{ formatNotifTime(n.timestamp) }}</div>
                  </div>
                </div>
              </div>

              <div v-if="currentOrgId" class="topbar-dropdown__footer">
                <button
                  class="topbar-dropdown__action"
                  @click="
                    notifOpen = false;
                    navigate(`/orgs/${currentOrgId}/notifications`);
                  "
                >
                  <t-icon name="inbox" size="13px" />
                  <span>{{ t('notifications.viewInbox') }}</span>
                </button>
              </div>
            </div>
          </div>
        </div>
      </header>

      <main class="app-content">
        <RouterView v-slot="{ Component }">
          <Transition name="page-fade" mode="out-in">
            <component :is="Component" :key="route.fullPath" />
          </Transition>
        </RouterView>
      </main>
    </section>
  </div>

  <t-dialog
    v-model:visible="showCreateOrg"
    :header="t('org.createDialog')"
    :confirm-btn="{ content: t('common.create'), loading: creatingOrg }"
    width="460px"
    @confirm="handleCreateOrg"
    @close="showCreateOrg = false"
  >
    <t-form label-align="top">
      <t-form-item :label="t('org.nameLabel')" required>
        <t-input
          v-model="newOrgName"
          :placeholder="t('org.namePlaceholder')"
          autofocus
          @keyup.enter="handleCreateOrg"
        />
      </t-form-item>
      <t-form-item :label="t('org.descLabel')">
        <t-input v-model="newOrgDesc" :placeholder="t('org.descPlaceholder')" />
      </t-form-item>
    </t-form>
  </t-dialog>
</template>

<style scoped>
.app-shell {
  height: 100vh;
  display: grid;
  grid-template-columns: 244px minmax(0, 1fr);
  background: var(--ordo-bg-app);
  color: var(--ordo-text-primary);
  overflow: hidden;
}

/* ── Sidebar ────────────────────────────────────────────────────────────────── */

.sidebar {
  display: flex;
  flex-direction: column;
  min-height: 100vh;
  padding: 18px 14px;
  border-right: 1px solid var(--ordo-sidebar-border);
  background: var(--ordo-sidebar-bg);
}

.sidebar__brand {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 40px;
  padding: 0 10px;
}

.sidebar__logo {
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.sidebar__logo-image {
  width: 100%;
  height: 100%;
  display: block;
  object-fit: contain;
}

.sidebar__brand-copy {
  display: flex;
  flex-direction: column;
  line-height: 1.1;
}

.sidebar__brand-copy strong {
  font-size: 13px;
  font-weight: 600;
}

.sidebar__brand-copy span {
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.sidebar__org {
  margin-top: 18px;
  padding: 12px 10px;
  border: 1px solid var(--ordo-sidebar-org-border);
  border-radius: 10px;
  background: var(--ordo-sidebar-org-bg);
}

.sidebar__org-name {
  margin-top: 4px;
  font-size: 13px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.sidebar__section {
  margin-top: 18px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.sidebar__section--grow {
  flex: 1;
}

.sidebar__section-label {
  padding: 0 10px 6px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--ordo-text-tertiary);
}

.sidebar-link {
  width: 100%;
  min-height: 36px;
  padding: 0 10px;
  border: 1px solid transparent;
  border-radius: 8px;
  display: flex;
  align-items: center;
  gap: 10px;
  text-align: left;
  cursor: pointer;
  color: var(--ordo-text-secondary);
  background: transparent;
  transition:
    background 0.12s ease,
    color 0.12s ease;
}

.sidebar-link:hover {
  color: var(--ordo-text-primary);
  background: var(--ordo-sidebar-hover-bg);
}

.sidebar-link.is-active {
  color: var(--ordo-text-primary);
  font-weight: 600;
  background: var(--ordo-bg-panel);
  border-color: var(--ordo-border-color);
}

.sidebar-link :deep(.t-icon) {
  font-size: 15px;
  color: var(--ordo-text-tertiary);
}

.sidebar-link.is-active :deep(.t-icon) {
  color: var(--ordo-text-primary);
}

.sidebar-link__badge {
  min-width: 26px;
  height: 20px;
  border-radius: 999px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 10px;
  font-weight: 700;
  color: var(--ordo-text-secondary);
  background: var(--ordo-bg-item-hover);
}

.sidebar__footer {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-top: 10px;
  border-top: 1px solid var(--ordo-sidebar-footer-border);
}

.sidebar-user {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.12s ease;
}

.sidebar-user:hover {
  background: var(--ordo-sidebar-hover-bg);
}

.sidebar-user__avatar {
  width: 28px;
  height: 28px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: 700;
  color: var(--ordo-text-primary);
  background: var(--ordo-bg-item-hover);
}

.sidebar-user__copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.sidebar-user__copy strong {
  font-size: 12px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.sidebar-user__copy span {
  font-size: 11px;
  color: var(--ordo-text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* ── Main shell ─────────────────────────────────────────────────────────────── */

.main-shell {
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.topbar {
  position: relative;
  z-index: 999;
  min-height: 64px;
  padding: 14px 20px;
  border-bottom: 1px solid var(--ordo-border-color);
  background: color-mix(in srgb, var(--ordo-bg-app) 92%, transparent);
  backdrop-filter: blur(6px);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
}

.topbar__title {
  min-width: 0;
}

.topbar__title h1 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.topbar__title p {
  margin: 4px 0 0;
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.topbar__actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

/* Search */
.command-trigger {
  min-width: 280px;
  height: 36px;
  padding: 0 12px;
  border: 1px solid var(--ordo-border-color);
  border-radius: 10px;
  display: flex;
  align-items: center;
  gap: 8px;
  background: var(--ordo-bg-panel);
  color: var(--ordo-text-secondary);
  cursor: pointer;
  transition: border-color 0.12s;
}

.command-trigger:hover {
  border-color: var(--ordo-border-color);
}

.command-trigger span {
  flex: 1;
  text-align: left;
  font-size: 13px;
}

.command-trigger kbd {
  min-width: 44px;
  height: 22px;
  padding: 0 8px;
  border: 1px solid var(--ordo-border-color);
  border-radius: 6px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font: inherit;
  font-size: 11px;
  font-weight: 500;
  color: var(--ordo-text-tertiary);
  background: var(--ordo-bg-app);
}

/* ── Pill buttons + dropdowns ───────────────────────────────────────────────── */

.topbar-pill-wrap {
  position: relative;
  flex-shrink: 0;
}

.topbar-pill {
  height: 36px;
  padding: 0 12px;
  border: 1px solid var(--ordo-border-color);
  border-radius: 10px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  font-weight: 500;
  color: var(--ordo-text-primary);
  background: var(--ordo-bg-panel);
  cursor: pointer;
  transition:
    border-color 0.12s,
    background 0.12s,
    color 0.12s;
  white-space: nowrap;
}

.topbar-pill--org {
  min-width: 210px;
}

.topbar-pill:hover {
  border-color: var(--ordo-border-color);
  color: var(--ordo-text-primary);
}

.topbar-pill.is-open {
  border-color: var(--ordo-border-focus);
  background: var(--ordo-accent-bg);
  color: var(--ordo-accent);
}

.topbar-pill--icon {
  padding: 0 10px;
  position: relative;
}

.topbar-pill--icon.has-unread {
  border-color: #f0a830;
}

.topbar-pill__text {
  max-width: 140px;
  overflow: hidden;
  text-overflow: ellipsis;
}

.topbar-pill__arrow {
  margin-left: auto;
  color: var(--ordo-text-tertiary);
  transition: transform 0.15s;
}

.topbar-pill__arrow.is-flipped {
  transform: rotate(180deg);
}

/* Shared dropdown panel */
.topbar-dropdown {
  position: absolute;
  top: calc(100% + 6px);
  right: 0;
  z-index: 900;
  min-width: 220px;
  border: 1px solid var(--ordo-border-color);
  border-radius: 12px;
  background: var(--ordo-bg-panel);
  box-shadow:
    0 8px 24px rgba(0, 0, 0, 0.1),
    0 2px 6px rgba(0, 0, 0, 0.06);
  overflow: hidden;
}

.topbar-dropdown--wide {
  min-width: 240px;
}

.topbar-dropdown--orgs {
  min-width: 250px;
}

.topbar-dropdown--health {
  min-width: 260px;
}

.topbar-dropdown--notif {
  min-width: 320px;
  max-width: 360px;
}

.topbar-dropdown__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px 8px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--ordo-text-tertiary);
  border-bottom: 1px solid var(--ordo-border-light);
}

.topbar-dropdown__item {
  width: 100%;
  padding: 9px 14px;
  border: none;
  background: transparent;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: var(--ordo-text-primary);
  cursor: pointer;
  transition: background 0.1s;
  text-align: left;
}

.topbar-dropdown__item:hover {
  background: var(--ordo-bg-item-hover);
  color: var(--ordo-text-primary);
}

.topbar-dropdown__item.is-active {
  color: var(--ordo-text-primary);
  font-weight: 600;
}

.topbar-dropdown__check {
  margin-left: auto;
  color: var(--ordo-accent);
}

/* Sub-org items: indented with a connector line */
.topbar-dropdown__item.is-suborg {
  padding-left: 10px;
  background: var(--ordo-bg-app);
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.topbar-dropdown__item.is-suborg:hover {
  background: var(--ordo-bg-item-hover);
  color: var(--ordo-text-primary);
}

.topbar-dropdown__item.is-suborg.is-active {
  color: var(--ordo-text-primary);
}

.suborg-indent-connector {
  display: inline-block;
  width: 12px;
  height: 14px;
  flex-shrink: 0;
  border-left: 1.5px solid var(--ordo-border-color);
  border-bottom: 1.5px solid var(--ordo-border-color);
  border-bottom-left-radius: 3px;
  margin-right: -2px;
  margin-bottom: -2px;
  align-self: flex-end;
}

.topbar-dropdown__footer {
  padding: 8px;
  border-top: 1px solid var(--ordo-border-light);
}

.topbar-dropdown__action {
  width: 100%;
  min-height: 34px;
  padding: 0 10px;
  border: 1px dashed var(--ordo-border-color);
  border-radius: 8px;
  background: var(--ordo-bg-app);
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--ordo-text-primary);
  cursor: pointer;
  transition:
    background 0.1s,
    border-color 0.1s;
}

.topbar-dropdown__action:hover {
  background: var(--ordo-bg-item-hover);
  border-color: var(--ordo-border-color);
}

/* ── Health dropdown ─────────────────────────────────────────────────────────── */

.health-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.health-dot--online {
  background: #34c759;
  box-shadow: 0 0 0 2px rgba(52, 199, 89, 0.2);
}
.health-dot--degraded {
  background: #ff9500;
  box-shadow: 0 0 0 2px rgba(255, 149, 0, 0.2);
}
.health-dot--offline {
  background: #ff3b30;
  box-shadow: 0 0 0 2px rgba(255, 59, 48, 0.2);
}
.health-dot--default {
  background: #aeaaa2;
}

.health-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 14px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
  border-bottom: 1px solid var(--ordo-border-light);
}

.health-row strong {
  color: var(--ordo-text-primary);
}

.health-url {
  font-family: 'JetBrains Mono', monospace;
  font-size: 11px;
  color: var(--ordo-text-secondary);
  word-break: break-all;
  text-align: right;
  max-width: 160px;
}

.health-default-msg {
  padding: 16px 14px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
  text-align: center;
}

.health-detail-link {
  display: block;
  width: 100%;
  padding: 8px 14px;
  border: none;
  background: transparent;
  font-size: 12px;
  color: var(--ordo-accent);
  cursor: pointer;
  text-align: left;
  transition: background 0.1s;
}

.health-detail-link:hover {
  background: var(--ordo-accent-bg);
}

/* ── Notification dropdown ───────────────────────────────────────────────────── */

.notif-badge {
  position: absolute;
  top: 4px;
  right: 4px;
  min-width: 16px;
  height: 16px;
  padding: 0 4px;
  border-radius: 999px;
  background: #f53f3f;
  color: var(--ordo-text-inverse);
  font-size: 10px;
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
  line-height: 1;
  pointer-events: none;
}

.notif-mark-read {
  border: none;
  background: transparent;
  font-size: 11px;
  color: var(--ordo-accent);
  cursor: pointer;
  padding: 0;
  font-weight: 500;
}

.notif-mark-read:hover {
  text-decoration: underline;
}

.notif-empty {
  padding: 28px 16px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: var(--ordo-text-tertiary);
}

.notif-list {
  max-height: 360px;
  overflow-y: auto;
}

.notif-item {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--ordo-border-light);
  transition: background 0.1s;
}

.notif-item:last-child {
  border-bottom: none;
}

.notif-item:hover {
  background: var(--ordo-bg-item-hover);
}

.notif-item--unread {
  background: var(--ordo-bg-selected);
}

.notif-icon {
  width: 26px;
  height: 26px;
  border-radius: 7px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}

.notif-icon--success {
  background: #e8f8ef;
  color: #00a854;
}
.notif-icon--error {
  background: #fff1f0;
  color: #f53f3f;
}
.notif-icon--warning {
  background: #fff8e8;
  color: #e8a805;
}
.notif-icon--info {
  background: #edf4ff;
  color: #3065a4;
}

.notif-content {
  min-width: 0;
  flex: 1;
}

.notif-title {
  font-size: 13px;
  font-weight: 500;
  color: var(--ordo-text-primary);
  line-height: 1.4;
}

.notif-message {
  margin-top: 2px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
  line-height: 1.4;
  word-break: break-word;
}

.notif-time {
  margin-top: 4px;
  font-size: 11px;
  color: var(--ordo-text-tertiary);
}

/* ── App content ─────────────────────────────────────────────────────────────── */

.app-content {
  min-height: 0;
  flex: 1;
  overflow: hidden;
}

.page-fade-enter-active,
.page-fade-leave-active {
  transition: opacity 0.14s ease;
}

.page-fade-enter-from,
.page-fade-leave-to {
  opacity: 0;
}

@media (max-width: 1120px) {
  .topbar {
    flex-direction: column;
    align-items: stretch;
  }

  .topbar__actions {
    flex-wrap: wrap;
  }

  .command-trigger {
    min-width: 0;
    flex: 1;
  }
}

@media (max-width: 900px) {
  .app-shell {
    height: auto;
    grid-template-columns: 1fr;
    overflow: visible;
  }

  .sidebar {
    min-height: auto;
    border-right: none;
    border-bottom: 1px solid var(--ordo-sidebar-border);
  }

  .main-shell,
  .app-content {
    overflow: visible;
  }
}
</style>
