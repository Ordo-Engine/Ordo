<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import platformLogo from '@/assets/platform-logo.png'
import { useAuthStore } from '@/stores/auth'
import { useOrgStore } from '@/stores/org'
import { useProjectStore } from '@/stores/project'
import { useServerStore } from '@/stores/server'
import { useNotificationStore } from '@/stores/notification'
import { i18n, LOCALE_OPTIONS, setLocale, type Locale } from '@/i18n'

const router = useRouter()
const route = useRoute()
const auth = useAuthStore()
const orgStore = useOrgStore()
const projectStore = useProjectStore()
const serverStore = useServerStore()
const notifStore = useNotificationStore()
const { t } = useI18n()

const currentLocale = computed(() => (i18n.global.locale as any).value as Locale)
const currentLocaleLabel = computed(
  () => LOCALE_OPTIONS.find((option) => option.value === currentLocale.value)?.label ?? 'EN',
)

const currentOrgId = computed(() => orgStore.currentOrg?.id ?? '')
const currentProjectId = computed(() =>
  typeof route.params.projectId === 'string' ? route.params.projectId : null,
)

const currentProject = computed(() =>
  currentProjectId.value
    ? projectStore.projects.find((project) => project.id === currentProjectId.value) ?? null
    : projectStore.currentProject,
)

// ── Topbar dropdown state ────────────────────────────────────────────────────

const projectPickerOpen = ref(false)
const healthOpen = ref(false)
const notifOpen = ref(false)
const projectPickerRef = ref<HTMLElement | null>(null)
const healthRef = ref<HTMLElement | null>(null)
const notifRef = ref<HTMLElement | null>(null)

function onDocumentClick(e: MouseEvent) {
  const target = e.target as Node
  if (projectPickerRef.value && !projectPickerRef.value.contains(target)) projectPickerOpen.value = false
  if (healthRef.value && !healthRef.value.contains(target)) healthOpen.value = false
  if (notifRef.value && !notifRef.value.contains(target)) notifOpen.value = false
}

// ── Server health ────────────────────────────────────────────────────────────

const boundServer = computed(() => {
  if (!currentProject.value?.server_id) return null
  return serverStore.getById(currentProject.value.server_id) ?? null
})

const boundServerStatus = computed(() => boundServer.value?.status ?? 'default')

function statusTheme(status: string) {
  if (status === 'online') return 'success'
  if (status === 'degraded') return 'warning'
  if (status === 'default') return 'default'
  return 'danger'
}

// ── Project picker ───────────────────────────────────────────────────────────

const projectOptions = computed(() =>
  projectStore.projects.map((p) => ({
    id: p.id,
    name: p.name,
    active: p.id === currentProject.value?.id,
  })),
)

function switchProject(projectId: string) {
  projectPickerOpen.value = false
  navigate(`/orgs/${currentOrgId.value}/projects/${projectId}/editor`)
}

// ── Notifications ────────────────────────────────────────────────────────────

function formatNotifTime(date: Date) {
  const diff = Date.now() - date.getTime()
  if (diff < 60_000) return t('shell.justNow')
  if (diff < 3_600_000) return t('shell.minutesAgo', { n: Math.floor(diff / 60_000) })
  if (diff < 86_400_000) return t('shell.hoursAgo', { n: Math.floor(diff / 3_600_000) })
  return date.toLocaleDateString()
}

function notifIcon(type: string) {
  if (type === 'success') return 'check-circle'
  if (type === 'error') return 'close-circle'
  if (type === 'warning') return 'error-circle'
  return 'info-circle'
}

// ── Page info ────────────────────────────────────────────────────────────────

const pageInfo = computed(() => {
  const name = route.name?.toString()
  switch (name) {
    case 'dashboard':      return { title: t('nav.dashboard'), subtitle: t('shell.dashboardSubtitle') }
    case 'projects':       return { title: t('nav.projects'), subtitle: t('shell.projectsSubtitle') }
    case 'org-members':    return { title: t('nav.members'), subtitle: t('shell.membersSubtitle') }
    case 'org-roles':
    case 'org-role-create':
    case 'org-role-edit':  return { title: t('nav.roles'), subtitle: t('shell.rolesSubtitle') }
    case 'org-settings':   return { title: t('nav.orgSettings'), subtitle: t('shell.orgSettingsSubtitle') }
    case 'settings':       return { title: t('nav.settings'), subtitle: t('shell.settingsSubtitle') }
    case 'servers':        return { title: t('settings.serverRegistry.title'), subtitle: t('shell.serversSubtitle') }
    case 'org-servers':    return { title: t('settings.serverRegistry.title'), subtitle: t('shell.serversSubtitle') }
    case 'editor':
    case 'editor-ruleset': return { title: currentProject.value?.name ?? t('projectNav.rules'), subtitle: t('shell.editorSubtitle') }
    case 'facts':          return { title: t('projectNav.facts'), subtitle: t('facts.desc') }
    case 'concepts':       return { title: t('projectNav.concepts'), subtitle: t('concepts.desc') }
    case 'contracts':      return { title: t('projectNav.contracts'), subtitle: t('contracts.desc') }
    case 'tests':          return { title: t('projectNav.tests'), subtitle: t('shell.testsSubtitle') }
    case 'versions':       return { title: t('projectNav.versions'), subtitle: t('shell.versionsSubtitle') }
    case 'project-instances': return { title: t('projectNav.instances'), subtitle: t('projectInstances.bindingDesc') }
    case 'project-settings':    return { title: t('projectNav.settings'), subtitle: t('projectSettings.serverBindingDesc') }
    case 'marketplace':         return { title: t('marketplace.title'), subtitle: t('marketplace.subtitle') }
    case 'marketplace-detail':  return { title: t('marketplace.detail'), subtitle: t('marketplace.subtitle') }
    default:               return { title: t('nav.dashboard'), subtitle: t('shell.dashboardSubtitle') }
  }
})

// ── Nav items ────────────────────────────────────────────────────────────────

const workspaceItems = computed(() => {
  const projectTarget = currentOrgId.value ? `/orgs/${currentOrgId.value}/projects` : '/orgs'
  return [
    { value: 'dashboard', label: t('nav.dashboard'), icon: 'dashboard-1', active: route.path.startsWith('/dashboard'), action: () => navigate('/dashboard') },
    { value: 'projects',  label: t('nav.projects'), icon: 'layers', active: route.path.includes('/projects') && route.query.openTemplate !== '1', action: () => navigate(projectTarget) },
    { value: 'templates', label: t('template.fromTemplate'), icon: 'gesture-applause', active: route.path.includes('/projects') && route.query.openTemplate === '1', action: () => navigate(projectTarget, { openTemplate: '1' }) },
    { value: 'marketplace', label: t('marketplace.title'), icon: 'shop', active: route.path.startsWith('/marketplace'), action: () => navigate('/marketplace') },
  ]
})

const orgItems = computed(() => {
  if (!currentOrgId.value) return []
  return [
    { value: 'members',    label: t('nav.members'), icon: 'usergroup', active: route.path.includes('/members'), action: () => navigate(`/orgs/${currentOrgId.value}/members`) },
    { value: 'roles',      label: t('nav.roles'), icon: 'secured', active: route.path.includes('/roles'), action: () => navigate(`/orgs/${currentOrgId.value}/roles`) },
    { value: 'servers',    label: t('settings.serverRegistry.title'), icon: 'internet', active: route.path.includes(`/orgs/${currentOrgId.value}/servers`), action: () => navigate(`/orgs/${currentOrgId.value}/servers`) },
    { value: 'orgSettings', label: t('nav.orgSettings'), icon: 'setting-1', active: route.path.includes(`/orgs/${currentOrgId.value}/settings`), action: () => navigate(`/orgs/${currentOrgId.value}/settings`) },
  ]
})

const systemItems = computed(() => [
  { value: 'settings', label: t('nav.settings'), icon: 'setting', active: route.path.startsWith('/settings'), action: () => navigate('/settings') },
])

const orgOptions = computed(() =>
  orgStore.orgs.map((org) => ({ label: org.name, value: org.id })),
)

// ── Lifecycle ────────────────────────────────────────────────────────────────

onMounted(async () => {
  await orgStore.fetchOrgs()
  if (orgStore.currentOrg) await projectStore.fetchProjects(orgStore.currentOrg.id)
  await serverStore.fetchServers()
  document.addEventListener('click', onDocumentClick, true)
})

onUnmounted(() => {
  document.removeEventListener('click', onDocumentClick, true)
})

watch(currentOrgId, async (id) => {
  if (id) await serverStore.fetchServers()
})

// ── Helpers ──────────────────────────────────────────────────────────────────

function navigate(to: string, query?: Record<string, string>) {
  router.push(query ? { path: to, query } : to)
}

async function onOrgChange(value: string) {
  await orgStore.selectOrg(value)
  await projectStore.fetchProjects(value)
  navigate(`/orgs/${value}/projects`)
}

function cycleLocale() {
  const locales = LOCALE_OPTIONS.map((option) => option.value)
  const index = locales.indexOf(currentLocale.value)
  setLocale(locales[(index + 1) % locales.length])
}

function triggerGlobalCommandPalette() {
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', ctrlKey: true, bubbles: true }))
}

function handleLogout() {
  auth.logout()
  router.push('/login')
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
        <button v-for="item in workspaceItems" :key="item.value" class="sidebar-link" :class="{ 'is-active': item.active }" @click="item.action">
          <t-icon :name="item.icon" />
          <span>{{ item.label }}</span>
        </button>
      </section>

      <section v-if="orgItems.length" class="sidebar__section">
        <div class="sidebar__section-label">{{ t('shell.orgContext') }}</div>
        <button v-for="item in orgItems" :key="item.value" class="sidebar-link" :class="{ 'is-active': item.active }" @click="item.action">
          <t-icon :name="item.icon" />
          <span>{{ item.label }}</span>
        </button>
      </section>

      <section class="sidebar__section sidebar__section--grow">
        <div class="sidebar__section-label">{{ t('shell.system') }}</div>
        <button v-for="item in systemItems" :key="item.value" class="sidebar-link" :class="{ 'is-active': item.active }" @click="item.action">
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
          <div v-if="orgOptions.length > 0" class="topbar-control topbar-select-shell">
            <t-select
              borderless
              :value="currentOrgId"
              :options="orgOptions"
              class="topbar__org-select"
              @change="(value: any) => onOrgChange(value)"
            />
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
              <span class="topbar-pill__text">{{ currentProject?.name ?? t('shell.noProject') }}</span>
              <t-icon name="chevron-down" size="12px" class="topbar-pill__arrow" :class="{ 'is-flipped': projectPickerOpen }" />
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
                <t-icon v-if="proj.active" name="check" size="13px" class="topbar-dropdown__check" />
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
                  <strong>{{ boundServer.last_seen ? new Date(boundServer.last_seen).toLocaleString() : t('shell.never') }}</strong>
                </div>
                <button class="health-detail-link" @click="healthOpen = false; navigate(`/orgs/${currentOrgId}/servers`)">
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
              :class="{ 'is-open': notifOpen, 'has-unread': notifStore.unreadCount > 0 }"
              @click="notifOpen = !notifOpen"
            >
              <t-icon name="notification" size="16px" />
              <span v-if="notifStore.unreadCount > 0" class="notif-badge">
                {{ notifStore.unreadCount > 99 ? '99+' : notifStore.unreadCount }}
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
                <t-icon name="notification" size="24px" style="opacity:0.25" />
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
</template>

<style scoped>
.app-shell {
  height: 100vh;
  display: grid;
  grid-template-columns: 244px minmax(0, 1fr);
  background: #f6f5f1;
  color: #1f2328;
  overflow: hidden;
}

/* ── Sidebar ────────────────────────────────────────────────────────────────── */

.sidebar {
  display: flex;
  flex-direction: column;
  min-height: 100vh;
  padding: 18px 14px;
  border-right: 1px solid #e4e1d9;
  background: #f2f0ea;
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
  color: #6b7280;
}

.sidebar__org {
  margin-top: 18px;
  padding: 12px 10px;
  border: 1px solid #e7e3d8;
  border-radius: 10px;
  background: #faf8f3;
}

.sidebar__org-name {
  margin-top: 4px;
  font-size: 13px;
  font-weight: 600;
  color: #1f2328;
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
  color: #7f7669;
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
  color: #5f6570;
  background: transparent;
  transition: background 0.12s ease, color 0.12s ease;
}

.sidebar-link:hover {
  color: #1f2328;
  background: rgba(255, 255, 255, 0.55);
}

.sidebar-link.is-active {
  color: #1f2328;
  font-weight: 600;
  background: #ffffff;
  border-color: #e2ddd2;
}

.sidebar-link :deep(.t-icon) {
  font-size: 15px;
  color: #7f7669;
}

.sidebar-link.is-active :deep(.t-icon) {
  color: #1f2328;
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
  color: #5f6570;
  background: #e7e5df;
}

.sidebar__footer {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-top: 10px;
  border-top: 1px solid #e4e1d9;
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
  background: rgba(255, 255, 255, 0.55);
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
  color: #1f2328;
  background: #ddd7c9;
}

.sidebar-user__copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.sidebar-user__copy strong {
  font-size: 12px;
  font-weight: 600;
  color: #1f2328;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.sidebar-user__copy span {
  font-size: 11px;
  color: #6b7280;
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
  border-bottom: 1px solid #e4e1d9;
  background: rgba(250, 249, 245, 0.92);
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
  color: #1f2328;
}

.topbar__title p {
  margin: 4px 0 0;
  font-size: 12px;
  color: #6b7280;
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
  border: 1px solid #ddd7c9;
  border-radius: 10px;
  display: flex;
  align-items: center;
  gap: 8px;
  background: #ffffff;
  color: #6b7280;
  cursor: pointer;
  transition: border-color 0.12s;
}

.command-trigger:hover {
  border-color: #c9c3b8;
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
  border: 1px solid #e3ddd0;
  border-radius: 6px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font: inherit;
  font-size: 11px;
  font-weight: 500;
  color: #8a8274;
  background: #f7f5ef;
}

/* Org select shell */
.topbar__org-select {
  width: 100%;
}

.topbar-control {
  flex-shrink: 0;
}

.topbar-select-shell {
  width: 200px;
  height: 36px;
  padding: 0 2px 0 10px;
  border: 1px solid #ddd7c9;
  border-radius: 10px;
  background: #ffffff;
  display: flex;
  align-items: center;
}

.topbar-select-shell:focus-within {
  border-color: #c5d4e8;
  box-shadow: 0 0 0 3px rgba(48, 101, 164, 0.08);
}

.topbar__org-select:deep(.t-input),
.topbar__org-select:deep(.t-select__wrap) {
  width: 100%;
  height: 100%;
}

.topbar__org-select:deep(.t-input__wrap) {
  height: 34px;
  padding: 0 10px 0 0;
  border: 0;
  border-radius: 8px;
  background: transparent;
  box-shadow: none;
  display: flex;
  align-items: center;
}

.topbar__org-select:deep(.t-input:hover),
.topbar__org-select:deep(.t-input__wrap:hover),
.topbar__org-select:deep(.t-input.t-is-focused),
.topbar__org-select:deep(.t-input__wrap.t-input__wrap--focused) {
  border: 0;
  background: transparent;
  box-shadow: none;
}

.topbar__org-select:deep(.t-input__inner),
.topbar__org-select:deep(.t-select__input-text) {
  font-size: 13px;
  font-weight: 500;
  line-height: 34px;
  color: #1f2328;
}

.topbar__org-select:deep(.t-input__inner) {
  padding-left: 0;
  height: 34px;
}

.topbar__org-select:deep(.t-input__suffix),
.topbar__org-select:deep(.t-select__right-icon) {
  color: #7f7669;
}

.topbar__org-select:deep(.t-select__right-icon) {
  margin-right: 0;
}

.topbar__org-select:deep(.t-fake-arrow) {
  font-size: 14px;
}

/* ── Pill buttons + dropdowns ───────────────────────────────────────────────── */

.topbar-pill-wrap {
  position: relative;
  flex-shrink: 0;
}

.topbar-pill {
  height: 36px;
  padding: 0 12px;
  border: 1px solid #ddd7c9;
  border-radius: 10px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  font-weight: 500;
  color: #444b55;
  background: #ffffff;
  cursor: pointer;
  transition: border-color 0.12s, background 0.12s, color 0.12s;
  white-space: nowrap;
}

.topbar-pill:hover {
  border-color: #c9c3b8;
  color: #1f2328;
}

.topbar-pill.is-open {
  border-color: #b8d1eb;
  background: #f0f6fc;
  color: #1a4f7a;
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
  color: #9e9589;
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
  border: 1px solid #e4e1d9;
  border-radius: 12px;
  background: #ffffff;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.10), 0 2px 6px rgba(0, 0, 0, 0.06);
  overflow: hidden;
}

.topbar-dropdown--wide {
  min-width: 240px;
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
  color: #7f7669;
  border-bottom: 1px solid #f0ede6;
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
  color: #444b55;
  cursor: pointer;
  transition: background 0.1s;
  text-align: left;
}

.topbar-dropdown__item:hover {
  background: #f7f5ef;
  color: #1f2328;
}

.topbar-dropdown__item.is-active {
  color: #1f2328;
  font-weight: 600;
}

.topbar-dropdown__check {
  margin-left: auto;
  color: #3065a4;
}

/* ── Health dropdown ─────────────────────────────────────────────────────────── */

.health-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.health-dot--online   { background: #34c759; box-shadow: 0 0 0 2px rgba(52, 199, 89, 0.2); }
.health-dot--degraded { background: #ff9500; box-shadow: 0 0 0 2px rgba(255, 149, 0, 0.2); }
.health-dot--offline  { background: #ff3b30; box-shadow: 0 0 0 2px rgba(255, 59, 48, 0.2); }
.health-dot--default  { background: #aeaaa2; }

.health-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 14px;
  font-size: 12px;
  color: #6b7280;
  border-bottom: 1px solid #f5f3ee;
}

.health-row strong {
  color: #1f2328;
}

.health-url {
  font-family: 'JetBrains Mono', monospace;
  font-size: 11px;
  color: #5f6570;
  word-break: break-all;
  text-align: right;
  max-width: 160px;
}

.health-default-msg {
  padding: 16px 14px;
  font-size: 12px;
  color: #6b7280;
  text-align: center;
}

.health-detail-link {
  display: block;
  width: 100%;
  padding: 8px 14px;
  border: none;
  background: transparent;
  font-size: 12px;
  color: #3065a4;
  cursor: pointer;
  text-align: left;
  transition: background 0.1s;
}

.health-detail-link:hover {
  background: #f0f6fc;
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
  color: #ffffff;
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
  color: #3065a4;
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
  color: #9e9589;
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
  border-bottom: 1px solid #f5f3ee;
  transition: background 0.1s;
}

.notif-item:last-child {
  border-bottom: none;
}

.notif-item:hover {
  background: #faf8f3;
}

.notif-item--unread {
  background: #fafbff;
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

.notif-icon--success { background: #e8f8ef; color: #00a854; }
.notif-icon--error   { background: #fff1f0; color: #f53f3f; }
.notif-icon--warning { background: #fff8e8; color: #e8a805; }
.notif-icon--info    { background: #edf4ff; color: #3065a4; }

.notif-content {
  min-width: 0;
  flex: 1;
}

.notif-title {
  font-size: 13px;
  font-weight: 500;
  color: #1f2328;
  line-height: 1.4;
}

.notif-message {
  margin-top: 2px;
  font-size: 12px;
  color: #6b7280;
  line-height: 1.4;
  word-break: break-word;
}

.notif-time {
  margin-top: 4px;
  font-size: 11px;
  color: #9e9589;
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
    border-bottom: 1px solid #e4e1d9;
  }

  .main-shell,
  .app-content {
    overflow: visible;
  }
}
</style>
