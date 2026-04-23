import { defineStore } from 'pinia';
import { ref, watch } from 'vue';

type Theme = 'light' | 'dark' | 'system';

export const usePreferencesStore = defineStore('preferences', () => {
  const theme = ref<Theme>((localStorage.getItem('ordo_theme') as Theme) ?? 'light');
  const sidebarCollapsed = ref(false);

  function setTheme(t: Theme) {
    theme.value = t;
    localStorage.setItem('ordo_theme', t);
    applyTheme(t);
  }

  function applyTheme(t: Theme) {
    const effectiveTheme =
      t === 'system'
        ? window.matchMedia('(prefers-color-scheme: dark)').matches
          ? 'dark'
          : 'light'
        : t;
    // Our custom ordo CSS tokens (ordo-theme.css, tdesign-override.css)
    document.documentElement.setAttribute('data-theme', effectiveTheme);
    // TDesign Vue Next 1.x activates dark mode via `theme-mode` attribute on <html>
    document.documentElement.setAttribute('theme-mode', effectiveTheme);
  }

  // Re-apply when OS dark/light preference changes (only relevant for 'system' mode)
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
    if (theme.value === 'system') applyTheme('system');
  });

  // Apply on init
  applyTheme(theme.value);

  watch(theme, applyTheme);

  return { theme, sidebarCollapsed, setTheme };
});
