import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { useOrgStore } from './org'
import { useProjectStore } from './project'

export const useDashboardStore = defineStore('dashboard', () => {
  const orgStore = useOrgStore()
  const projectStore = useProjectStore()

  const loading = ref(false)

  const totalOrgs = computed(() => orgStore.orgs.length)
  const totalProjects = computed(() => projectStore.projects.length)
  const totalRulesets = computed(() => projectStore.rulesets?.length ?? 0)

  const recentProjects = computed(() =>
    [...projectStore.projects]
      .sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime())
      .slice(0, 6),
  )

  async function fetchDashboardData() {
    loading.value = true
    try {
      await orgStore.fetchOrgs()
      if (orgStore.currentOrg) {
        await projectStore.fetchProjects(orgStore.currentOrg.id)
      }
    } finally {
      loading.value = false
    }
  }

  return { loading, totalOrgs, totalProjects, totalRulesets, recentProjects, fetchDashboardData }
})
