<script setup lang="ts">
import { onMounted, computed, provide } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAuthStore } from '@/stores/auth'
import { LOCALE_KEY, type Lang } from '@ordo-engine/editor-vue'
import CommandPalette from '@/components/CommandPalette.vue'

const auth = useAuthStore()
const { locale } = useI18n()

// Bridge Studio's vue-i18n locale to the editor component's LOCALE_KEY.
const editorLocale = computed<Lang>({
  get: () => {
    const v = locale.value
    if (v === 'en' || v === 'zh-CN' || v === 'zh-TW') return v
    return 'en'
  },
  set: (v) => { locale.value = v },
})
provide(LOCALE_KEY, editorLocale)

onMounted(async () => {
  // Restore user info if a token exists
  if (auth.token && !auth.user) {
    await auth.fetchMe()
  }
})
</script>

<template>
  <router-view />
  <CommandPalette />
</template>
