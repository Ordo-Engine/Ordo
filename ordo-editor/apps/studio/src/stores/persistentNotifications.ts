import { defineStore } from 'pinia'
import { ref } from 'vue'
import { notificationApi } from '@/api/platform-client'
import { useAuthStore } from './auth'
import type { PlatformNotification } from '@/api/types'

export const usePersistentNotificationStore = defineStore('persistentNotifications', () => {
  const auth = useAuthStore()

  const notifications = ref<PlatformNotification[]>([])
  const unreadCount = ref(0)
  let pollTimer: ReturnType<typeof setInterval> | null = null

  async function fetchCount(orgId: string) {
    if (!auth.token || !orgId) return
    try {
      const result = await notificationApi.count(auth.token, orgId)
      unreadCount.value = result.unread
    } catch {
      // ignore poll errors
    }
  }

  async function fetchNotifications(orgId: string, unreadOnly = false) {
    if (!auth.token || !orgId) return
    try {
      notifications.value = await notificationApi.list(auth.token, orgId, {
        unread_only: unreadOnly,
        limit: 50,
      })
      unreadCount.value = unreadOnly
        ? notifications.value.length
        : notifications.value.filter((n) => !n.read_at).length
    } catch {
      // ignore
    }
  }

  async function markRead(orgId: string, notifId: string) {
    if (!auth.token) return
    await notificationApi.markRead(auth.token, orgId, notifId)
    const n = notifications.value.find((n) => n.id === notifId)
    if (n) {
      n.read_at = new Date().toISOString()
      unreadCount.value = Math.max(0, unreadCount.value - 1)
    }
  }

  async function markAllRead(orgId: string) {
    if (!auth.token) return
    await notificationApi.markAllRead(auth.token, orgId)
    notifications.value.forEach((n) => {
      if (!n.read_at) n.read_at = new Date().toISOString()
    })
    unreadCount.value = 0
  }

  function startPolling(orgId: string) {
    if (pollTimer) return
    fetchCount(orgId)
    pollTimer = setInterval(() => fetchCount(orgId), 60_000)
  }

  function stopPolling() {
    if (pollTimer) {
      clearInterval(pollTimer)
      pollTimer = null
    }
  }

  return {
    notifications,
    unreadCount,
    fetchCount,
    fetchNotifications,
    markRead,
    markAllRead,
    startPolling,
    stopPolling,
  }
})
