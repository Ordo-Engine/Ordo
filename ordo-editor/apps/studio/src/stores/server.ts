import { defineStore } from 'pinia'
import { ref } from 'vue'
import { serverApi } from '@/api/platform-client'
import { useAuthStore } from './auth'
import type { ServerInfo } from '@/api/types'

export const useServerStore = defineStore('server', () => {
  const auth = useAuthStore()

  const servers = ref<ServerInfo[]>([])
  const loading = ref(false)

  async function fetchServers() {
    if (!auth.token) return
    loading.value = true
    try {
      servers.value = await serverApi.list(auth.token)
    } finally {
      loading.value = false
    }
  }

  async function deleteServer(id: string) {
    if (!auth.token) throw new Error('Not authenticated')
    await serverApi.delete(auth.token, id)
    servers.value = servers.value.filter((s) => s.id !== id)
  }

  function getById(id: string): ServerInfo | undefined {
    return servers.value.find((s) => s.id === id)
  }

  return { servers, loading, fetchServers, deleteServer, getById }
})
