import { defineStore } from 'pinia'
import { ref, watch } from 'vue'

type Theme = 'light' | 'dark' | 'system'

export const usePreferencesStore = defineStore('preferences', () => {
  const theme = ref<Theme>((localStorage.getItem('ordo_theme') as Theme) ?? 'light')
  const sidebarCollapsed = ref(false)

  function setTheme(t: Theme) {
    theme.value = t
    localStorage.setItem('ordo_theme', t)
    applyTheme(t)
  }

  function applyTheme(t: Theme) {
    const effectiveTheme =
      t === 'system'
        ? window.matchMedia('(prefers-color-scheme: dark)').matches
          ? 'dark'
          : 'light'
        : t
    document.documentElement.setAttribute('data-theme', effectiveTheme)
  }

  // Apply on init
  applyTheme(theme.value)

  watch(theme, applyTheme)

  return { theme, sidebarCollapsed, setTheme }
})
