<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useAuthStore } from '@/stores/auth'
import { useOrgStore } from '@/stores/org'
import { useProjectStore } from '@/stores/project'
import { i18n, setLocale, LOCALE_OPTIONS, type Locale } from '@/i18n'

const router = useRouter()
const route = useRoute()
const auth = useAuthStore()
const orgStore = useOrgStore()
const projectStore = useProjectStore()
const { t } = useI18n()

const currentLocale = computed(() => (i18n.global.locale as any).value as Locale)
const currentLocaleLabel = computed(
  () => LOCALE_OPTIONS.find((o) => o.value === currentLocale.value)?.label ?? 'EN',
)

const navItems = computed(() => [
  {
    value: 'dashboard',
    label: t('nav.dashboard'),
    icon: 'dashboard',
    to: '/dashboard',
    active: route.path.startsWith('/dashboard'),
  },
  {
    value: 'orgs',
    label: t('nav.orgs'),
    icon: 'home',
    to: '/orgs',
    active: route.path === '/orgs',
  },
])

const orgId = computed(() => orgStore.currentOrg?.id)

const orgSubItems = computed(() => {
  if (!orgId.value) return []
  return [
    {
      value: 'projects',
      label: t('nav.projects'),
      icon: 'layers',
      to: `/orgs/${orgId.value}/projects`,
      active: route.path.includes('/projects'),
    },
    {
      value: 'members',
      label: t('nav.members'),
      icon: 'usergroup',
      to: `/orgs/${orgId.value}/members`,
      active: route.path.includes('/members'),
    },
    {
      value: 'orgSettings',
      label: t('nav.orgSettings'),
      icon: 'setting-1',
      to: `/orgs/${orgId.value}/settings`,
      active: route.path.includes(`/orgs/${orgId.value}/settings`),
    },
  ]
})

onMounted(async () => {
  await orgStore.fetchOrgs()
  if (orgStore.currentOrg) {
    await projectStore.fetchProjects(orgStore.currentOrg.id)
  }
})

async function handleLogout() {
  auth.logout()
  router.push('/login')
}

function navigate(to: string) {
  router.push(to)
}

function cycleLocale() {
  const opts = LOCALE_OPTIONS.map((o) => o.value)
  const idx = opts.indexOf(currentLocale.value)
  setLocale(opts[(idx + 1) % opts.length])
}
</script>

<template>
  <div class="app-shell">
    <!-- ── Sidebar ── -->
    <nav class="sidebar">
      <!-- Logo + app name -->
      <div class="sidebar__brand" @click="navigate('/dashboard')">
        <svg class="brand-logo" width="28" height="28" viewBox="0 0 24 24" fill="none">
          <rect x="3" y="3" width="8" height="8" rx="1.5" fill="var(--ordo-accent)" />
          <rect x="13" y="3" width="8" height="8" rx="1.5" fill="var(--ordo-accent)" opacity=".6" />
          <rect x="3" y="13" width="8" height="8" rx="1.5" fill="var(--ordo-accent)" opacity=".6" />
          <rect x="13" y="13" width="8" height="8" rx="1.5" fill="var(--ordo-accent)" opacity=".3" />
        </svg>
        <div class="brand-text">
          <span class="brand-name">Ordo Studio</span>
          <span class="brand-org">{{ orgStore.currentOrg?.name ?? '—' }}</span>
        </div>
      </div>

      <!-- Main nav -->
      <div class="sidebar__nav">
        <button
          v-for="item in navItems"
          :key="item.value"
          class="nav-item"
          :class="{ 'is-active': item.active }"
          @click="navigate(item.to)"
        >
          <t-icon :name="item.icon" class="nav-item__icon" />
          <span class="nav-item__label">{{ item.label }}</span>
        </button>

        <!-- Org sub-nav -->
        <template v-if="orgSubItems.length">
          <div class="org-subnav">
            <div class="org-subnav__header">
              <span class="org-subnav__name">{{ orgStore.currentOrg?.name }}</span>
            </div>
            <button
              v-for="sub in orgSubItems"
              :key="sub.value"
              class="nav-item nav-item--sub"
              :class="{ 'is-active': sub.active }"
              @click="navigate(sub.to)"
            >
              <t-icon :name="sub.icon" class="nav-item__icon" />
              <span class="nav-item__label">{{ sub.label }}</span>
            </button>
          </div>
        </template>
      </div>

      <!-- Bottom section -->
      <div class="sidebar__bottom">
        <div class="sidebar__divider" />

        <!-- Settings -->
        <button
          class="nav-item"
          :class="{ 'is-active': route.path.startsWith('/settings') }"
          @click="navigate('/settings')"
        >
          <t-icon name="setting" class="nav-item__icon" />
          <span class="nav-item__label">{{ t('nav.settings') }}</span>
        </button>

        <!-- Language -->
        <button class="nav-item lang-item" @click="cycleLocale">
          <span class="lang-badge">{{ currentLocaleLabel }}</span>
          <span class="nav-item__label">{{ t('settings.languageLabel') }}</span>
        </button>

        <!-- User -->
        <t-dropdown
          :options="[{ content: t('nav.logout'), value: 'logout', prefixIcon: 'logout' }]"
          @click="handleLogout"
          placement="right-end"
          trigger="click"
        >
          <div class="user-row">
            <div class="user-avatar">
              {{ auth.user?.display_name?.[0]?.toUpperCase() ?? '?' }}
            </div>
            <div class="user-info">
              <span class="user-name">{{ auth.user?.display_name }}</span>
              <span class="user-email">{{ auth.user?.email }}</span>
            </div>
            <t-icon name="chevron-right" class="user-chevron" />
          </div>
        </t-dropdown>
      </div>
    </nav>

    <!-- ── Main content ── -->
    <main class="app-content">
      <RouterView v-slot="{ Component }">
        <Transition name="page-fade" mode="out-in">
          <component :is="Component" :key="route.path" />
        </Transition>
      </RouterView>
    </main>
  </div>
</template>

<style scoped>
.app-shell {
  display: flex;
  height: 100vh;
  overflow: hidden;
  background: var(--ordo-bg-app);
}

/* ── Sidebar ── */
.sidebar {
  width: 200px;
  flex-shrink: 0;
  background: var(--ordo-bg-sidebar);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border-right: 1px solid rgba(255, 255, 255, 0.06);
}

/* Brand */
.sidebar__brand {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 16px 14px 14px;
  cursor: pointer;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  flex-shrink: 0;
}

.brand-logo {
  flex-shrink: 0;
}

.brand-text {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}

.brand-name {
  font-size: 13px;
  font-weight: 700;
  color: #ffffff;
  letter-spacing: -0.01em;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.brand-org {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.4);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* Nav */
.sidebar__nav {
  flex: 1;
  padding: 8px 8px 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
  overflow-y: auto;
}

.nav-item {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 10px;
  height: 36px;
  border: none;
  background: transparent;
  cursor: pointer;
  border-radius: 6px;
  color: rgba(255, 255, 255, 0.5);
  transition: background 0.12s, color 0.12s;
  text-align: left;
  position: relative;
}

.nav-item:hover {
  background: rgba(255, 255, 255, 0.07);
  color: rgba(255, 255, 255, 0.85);
}

.nav-item.is-active {
  background: rgba(255, 255, 255, 0.1);
  color: #ffffff;
}

.nav-item.is-active::before {
  content: '';
  position: absolute;
  left: 0;
  top: 6px;
  bottom: 6px;
  width: 3px;
  background: var(--ordo-accent);
  border-radius: 0 3px 3px 0;
}

.nav-item__icon {
  font-size: 16px;
  flex-shrink: 0;
  opacity: 0.8;
}

.nav-item.is-active .nav-item__icon {
  opacity: 1;
}

.nav-item__label {
  font-size: 13px;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* Org sub-nav */
.org-subnav {
  margin-top: 4px;
  border-top: 1px solid rgba(255, 255, 255, 0.07);
  padding-top: 6px;
}

.org-subnav__header {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px 6px;
}

.org-subnav__name {
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: rgba(255, 255, 255, 0.28);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.nav-item--sub {
  padding-left: 16px;
}

/* Bottom */
.sidebar__bottom {
  padding: 0 8px 12px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  flex-shrink: 0;
}

.sidebar__divider {
  height: 1px;
  background: rgba(255, 255, 255, 0.07);
  margin: 6px 2px 8px;
}

/* Language badge */
.lang-item {
  color: rgba(255, 255, 255, 0.5);
}

.lang-badge {
  width: 28px;
  height: 20px;
  background: rgba(255, 255, 255, 0.1);
  border-radius: 4px;
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.04em;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  color: rgba(255, 255, 255, 0.7);
}

/* User row */
.user-row {
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 6px 10px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.12s;
  margin-top: 2px;
}

.user-row:hover {
  background: rgba(255, 255, 255, 0.07);
}

.user-avatar {
  width: 28px;
  height: 28px;
  border-radius: 8px;
  background: var(--ordo-accent);
  color: #fff;
  font-size: 12px;
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.user-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.user-name {
  font-size: 12px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.85);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.user-email {
  font-size: 10px;
  color: rgba(255, 255, 255, 0.35);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.user-chevron {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.25);
  flex-shrink: 0;
}

/* ── Content ── */
.app-content {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

/* Page transitions */
.page-fade-enter-active,
.page-fade-leave-active {
  transition: opacity 0.15s ease;
}
.page-fade-enter-from,
.page-fade-leave-to {
  opacity: 0;
}
</style>
