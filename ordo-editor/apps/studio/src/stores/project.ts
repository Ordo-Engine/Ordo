import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { projectApi } from '@/api/platform-client'
import { engineApi } from '@/api/engine-client'
import { convertFromEngineFormat } from '@ordo-engine/editor-core'
import { useAuthStore } from './auth'
import { useOrgStore } from './org'
import type { Project, RuleSetInfo } from '@/api/types'
import type { RuleSet } from '@ordo-engine/editor-core'

const CURRENT_PROJECT_KEY = 'ordo_studio_current_project'

export interface OpenTab {
  name: string
  ruleset: RuleSet
  dirty: boolean
}

export const useProjectStore = defineStore('project', () => {
  const auth = useAuthStore()
  const orgStore = useOrgStore()

  const projects = ref<Project[]>([])
  const currentProject = ref<Project | null>(null)
  const rulesets = ref<RuleSetInfo[]>([])
  const openTabs = ref<OpenTab[]>([])
  const activeTabName = ref<string | null>(null)
  const loading = ref(false)

  const currentProjectId = computed(() => currentProject.value?.id ?? null)
  const activeTab = computed(() => openTabs.value.find((t) => t.name === activeTabName.value) ?? null)

  async function fetchProjects(orgId: string) {
    if (!auth.token) return
    loading.value = true
    try {
      projects.value = await projectApi.list(auth.token, orgId)
      const savedId = localStorage.getItem(CURRENT_PROJECT_KEY)
      if (!currentProject.value && projects.value.length > 0) {
        const target = projects.value.find((p) => p.id === savedId) ?? projects.value[0]
        await selectProject(target)
      }
    } finally {
      loading.value = false
    }
  }

  async function selectProject(project: Project) {
    currentProject.value = project
    localStorage.setItem(CURRENT_PROJECT_KEY, project.id)
    await fetchRulesets()
  }

  async function fetchRulesets() {
    if (!auth.token || !currentProject.value) return
    rulesets.value = await engineApi.listRulesets(auth.token, currentProject.value.id)
  }

  async function openRuleset(name: string) {
    if (!auth.token || !currentProject.value) return
    // Already open?
    const existing = openTabs.value.find((t) => t.name === name)
    if (existing) {
      activeTabName.value = name
      return
    }
    // Fetch from engine
    const engineData = await engineApi.getRuleset(auth.token, currentProject.value.id, name)
    const ruleset = convertFromEngineFormat(engineData as any)
    openTabs.value.push({ name, ruleset, dirty: false })
    activeTabName.value = name
  }

  function updateActiveRuleset(ruleset: RuleSet) {
    const tab = openTabs.value.find((t) => t.name === activeTabName.value)
    if (tab) {
      tab.ruleset = ruleset
      tab.dirty = true
    }
  }

  function setTabRuleset(name: string, ruleset: RuleSet, dirty = true) {
    const tab = openTabs.value.find((t) => t.name === name)
    if (!tab) return
    tab.ruleset = ruleset
    tab.dirty = dirty
  }

  async function saveRuleset(name: string) {
    if (!auth.token || !currentProject.value) throw new Error('No active project')
    const tab = openTabs.value.find((t) => t.name === name)
    if (!tab) throw new Error('Ruleset not open')

    const { convertToEngineFormat } = await import('@ordo-engine/editor-core')
    const engineFormat = convertToEngineFormat(tab.ruleset)
    await engineApi.saveRuleset(auth.token, currentProject.value.id, name, engineFormat)
    tab.dirty = false
    // Refresh ruleset list
    await fetchRulesets()
  }

  async function createRuleset(ruleset: RuleSet) {
    if (!auth.token || !currentProject.value) throw new Error('No active project')
    const { convertToEngineFormat } = await import('@ordo-engine/editor-core')
    const engineFormat = convertToEngineFormat(ruleset)
    await engineApi.createRuleset(auth.token, currentProject.value.id, engineFormat)
    await fetchRulesets()
  }

  async function deleteRuleset(name: string) {
    if (!auth.token || !currentProject.value) throw new Error('No active project')
    await engineApi.deleteRuleset(auth.token, currentProject.value.id, name)
    closeTab(name)
    await fetchRulesets()
  }

  function closeTab(name: string) {
    const idx = openTabs.value.findIndex((t) => t.name === name)
    if (idx === -1) return
    openTabs.value.splice(idx, 1)
    if (activeTabName.value === name) {
      activeTabName.value = openTabs.value[Math.max(0, idx - 1)]?.name ?? null
    }
  }

  async function createProject(orgId: string, name: string, description?: string) {
    if (!auth.token) throw new Error('Not authenticated')
    const p = await projectApi.create(auth.token, orgId, name, description)
    projects.value.push(p)
    return p
  }

  async function deleteProject(orgId: string, projectId: string) {
    if (!auth.token) throw new Error('Not authenticated')
    await projectApi.delete(auth.token, orgId, projectId)
    projects.value = projects.value.filter((p) => p.id !== projectId)
    if (currentProject.value?.id === projectId) {
      currentProject.value = null
      rulesets.value = []
      openTabs.value = []
      activeTabName.value = null
    }
  }

  return {
    projects,
    currentProject,
    currentProjectId,
    rulesets,
    openTabs,
    activeTabName,
    activeTab,
    loading,
    fetchProjects,
    selectProject,
    fetchRulesets,
    openRuleset,
    updateActiveRuleset,
    setTabRuleset,
    saveRuleset,
    createRuleset,
    deleteRuleset,
    closeTab,
    createProject,
    deleteProject,
  }
})
