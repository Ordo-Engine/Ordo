<script setup lang="ts">
import { onMounted, onBeforeUnmount, computed, provide } from 'vue';
import { useI18n } from 'vue-i18n';
import { useAuthStore } from '@/stores/auth';
import { useProjectStore } from '@/stores/project';
import { LOCALE_KEY, type Lang } from '@ordo-engine/editor-vue';
import CommandPalette from '@/components/CommandPalette.vue';

const auth = useAuthStore();
const projectStore = useProjectStore();
const { locale } = useI18n();

// Warn before a browser refresh / tab-close / navigation-away wipes unsaved
// rule edits — open drafts live only in memory (Pinia), so a reload loses them.
function guardUnsavedTabs(e: BeforeUnloadEvent) {
  if (projectStore.openTabs.some((t) => t.dirty)) {
    e.preventDefault();
    // Legacy browsers require returnValue to be set to trigger the prompt.
    e.returnValue = '';
  }
}

// Bridge Studio's vue-i18n locale to the editor component's LOCALE_KEY.
const editorLocale = computed<Lang>({
  get: () => {
    const v = locale.value;
    if (v === 'en' || v === 'zh-CN' || v === 'zh-TW') return v;
    return 'en';
  },
  set: (v) => {
    locale.value = v;
  },
});
provide(LOCALE_KEY, editorLocale);

onMounted(async () => {
  window.addEventListener('beforeunload', guardUnsavedTabs);
  // Restore user info if a token exists
  if (auth.token && !auth.user) {
    await auth.fetchMe();
  }
});

onBeforeUnmount(() => {
  window.removeEventListener('beforeunload', guardUnsavedTabs);
});
</script>

<template>
  <router-view />
  <CommandPalette />
</template>
