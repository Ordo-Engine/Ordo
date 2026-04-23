import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { systemApi } from '@/api/platform-client'
import type { SystemConfig } from '@/api/types'

export const useSystemStore = defineStore('system', () => {
  const config = ref<SystemConfig | null>(null)
  const loading = ref(false)

  const allowOrgCreation = computed(() => config.value?.allow_org_creation ?? false)
  const allowRegistration = computed(() => config.value?.allow_registration ?? false)

  async function fetchConfig() {
    if (config.value) return
    loading.value = true
    try {
      config.value = await systemApi.getConfig()
    } catch {
      // Non-critical: fall back to restrictive defaults
    } finally {
      loading.value = false
    }
  }

  return { config, loading, allowOrgCreation, allowRegistration, fetchConfig }
})
