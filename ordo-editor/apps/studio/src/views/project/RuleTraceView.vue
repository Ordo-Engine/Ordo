<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useAuthStore } from '@/stores/auth'
import { rulesetDraftApi } from '@/api/platform-client'
import RuleTraceRunner from '@/components/trace/RuleTraceRunner.vue'
import type { ProjectRulesetMeta } from '@/api/types'

const route = useRoute()
const { t } = useI18n()
const auth = useAuthStore()

const orgId = computed(() => route.params.orgId as string)
const projectId = computed(() => route.params.projectId as string)

const rulesets = ref<ProjectRulesetMeta[]>([])
const loading = ref(false)

onMounted(async () => {
  if (!auth.token) return
  loading.value = true
  try {
    const result = await rulesetDraftApi.list(auth.token, orgId.value, projectId.value)
    rulesets.value = result
  } catch {
    // ignore
  } finally {
    loading.value = false
  }
})
</script>

<template>
  <div class="trace-view">
    <div v-if="loading" class="trace-view__loading">
      <t-loading />
    </div>
    <RuleTraceRunner
      v-else
      :org-id="orgId"
      :project-id="projectId"
      :rulesets="rulesets"
    />
  </div>
</template>

<style scoped>
.trace-view {
  padding: 24px;
  height: calc(100vh - 120px);
  display: flex;
  flex-direction: column;
}

.trace-view__loading {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
}
</style>
