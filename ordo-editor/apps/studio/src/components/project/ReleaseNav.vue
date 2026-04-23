<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'

const route = useRoute()
const { t } = useI18n()

const orgId = computed(() => route.params.orgId as string)
const projectId = computed(() => route.params.projectId as string)
const base = computed(() => `/orgs/${orgId.value}/projects/${projectId.value}/releases`)

const tabs = computed(() => [
  { key: 'overview', label: t('releaseCenter.navOverview'), to: `${base.value}`, active: route.path === `${base.value}` },
  { key: 'requests', label: t('releaseCenter.navRequests'), to: `${base.value}/requests`, active: route.path.includes('/releases/requests') },
  { key: 'policies', label: t('releaseCenter.navPolicies'), to: `${base.value}/policies`, active: route.path.endsWith('/releases/policies') },
  { key: 'history', label: t('releaseCenter.navHistory'), to: `${base.value}/history` , active: route.path.endsWith('/releases/history') || route.path.endsWith('/deployments') },
])
</script>

<template>
  <nav class="release-nav">
    <RouterLink
      v-for="tab in tabs"
      :key="tab.key"
      :to="tab.to"
      class="release-nav__link"
      :class="{ 'is-active': tab.active }"
    >
      {{ tab.label }}
    </RouterLink>
  </nav>
</template>

<style scoped>
.release-nav {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 20px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--ordo-border-color);
}

.release-nav__link {
  padding: 8px 12px;
  border-radius: 10px;
  font-size: 13px;
  color: var(--ordo-text-secondary);
  text-decoration: none;
  transition: background 0.2s ease, color 0.2s ease;
}

.release-nav__link:hover {
  background: var(--ordo-hover-bg);
  color: var(--ordo-text-primary);
}

.release-nav__link.is-active {
  background: color-mix(in srgb, var(--td-brand-color) 12%, transparent);
  color: var(--td-brand-color);
}
</style>
