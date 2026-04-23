import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

export type NotifType = 'success' | 'error' | 'warning' | 'info'

export interface Notification {
  id: string
  type: NotifType
  title: string
  message?: string
  timestamp: Date
  read: boolean
}

export const useNotificationStore = defineStore('notification', () => {
  const notifications = ref<Notification[]>([])
  const unreadCount = computed(() => notifications.value.filter((n) => !n.read).length)

  function push(type: NotifType, title: string, message?: string) {
    notifications.value.unshift({
      id: crypto.randomUUID(),
      type,
      title,
      message,
      timestamp: new Date(),
      read: false,
    })
    if (notifications.value.length > 50) notifications.value.pop()
  }

  function markAllRead() {
    notifications.value.forEach((n) => (n.read = true))
  }

  function clear() {
    notifications.value = []
  }

  return { notifications, unreadCount, push, markAllRead, clear }
})
