<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { MessagePlugin } from 'tdesign-vue-next'
import { useTestStore } from '@/stores/test'
import { useProjectStore } from '@/stores/project'
import type { RulesetTestSummary, TestRunResult } from '@/api/types'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const testStore = useTestStore()
const projectStore = useProjectStore()

const orgId = computed(() => route.params.orgId as string)
const projectId = computed(() => route.params.projectId as string)

const selectedRuleset = ref<string | null>(null)

// ── Computed from store ───────────────────────────────────────────────────────

const result = computed(() => testStore.projectRunResult)
const running = computed(() => testStore.projectRunning)

const rulesets = computed(() => result.value?.rulesets ?? [])

const selectedSummary = computed<RulesetTestSummary | null>(() =>
  rulesets.value.find((r) => r.ruleset_name === selectedRuleset.value) ?? null,
)

// ── Actions ───────────────────────────────────────────────────────────────────

async function runAll() {
  try {
    await testStore.runProjectTests(projectId.value)
    if (result.value && rulesets.value.length > 0 && !selectedRuleset.value) {
      selectedRuleset.value = rulesets.value[0].ruleset_name
    }
    MessagePlugin.success(t('test.runSuccess'))
  } catch (e: any) {
    MessagePlugin.error(e?.message ?? t('test.saveFailed'))
  }
}

function selectRuleset(name: string) {
  selectedRuleset.value = name
}

function goToEditor(rulesetName: string) {
  router.push(`/orgs/${orgId.value}/projects/${projectId.value}/editor/${rulesetName}`)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function passRateBadgeClass(r: RulesetTestSummary): string {
  if (r.total === 0) return 'badge--gray'
  if (r.failed === 0) return 'badge--pass'
  return 'badge--fail'
}

function resultStatusClass(r: TestRunResult): string {
  return r.passed ? 'status--pass' : 'status--fail'
}

function durationMs(us: number): string {
  return (us / 1000).toFixed(1) + 'ms'
}

onMounted(() => {
  // If we already have a project run result, pre-select first ruleset
  if (rulesets.value.length > 0 && !selectedRuleset.value) {
    selectedRuleset.value = rulesets.value[0].ruleset_name
  }
})
</script>

<template>
  <div class="test-view">
    <!-- Header -->
    <div class="test-view__header">
      <div class="test-view__header-left">
        <h2 class="test-view__title">{{ t('test.project.title') }}</h2>
        <p class="test-view__subtitle">{{ t('test.subtitle') }}</p>
      </div>
      <div class="test-view__header-right">
        <div v-if="result" class="test-view__summary-badge">
          {{ t('test.project.summary', { passed: result.passed, total: result.total }) }}
        </div>
        <t-button theme="primary" :loading="running" @click="runAll">
          <t-icon name="play-circle" size="14px" />
          {{ t('test.project.runAll') }}
        </t-button>
      </div>
    </div>

    <!-- No results yet -->
    <div v-if="!result && !running" class="test-view__empty">
      <t-icon name="task-checked" size="48px" style="opacity:0.25" />
      <p>{{ t('test.project.noTests') }}</p>
      <t-button variant="outline" @click="runAll">{{ t('test.project.runAll') }}</t-button>
    </div>

    <!-- Running indicator -->
    <div v-else-if="running" class="test-view__loading">
      <t-loading size="medium" :text="t('common.loading')" />
    </div>

    <!-- Results: two-panel layout -->
    <div v-else-if="result" class="test-view__content">
      <!-- Left: ruleset list -->
      <div class="test-view__ruleset-list">
        <div
          v-for="rs in rulesets"
          :key="rs.ruleset_name"
          class="ruleset-row"
          :class="{ 'ruleset-row--active': selectedRuleset === rs.ruleset_name }"
          @click="selectRuleset(rs.ruleset_name)"
        >
          <span class="ruleset-row__name">{{ rs.ruleset_name }}</span>
          <span class="ruleset-row__badge" :class="passRateBadgeClass(rs)">
            {{ rs.passed }}/{{ rs.total }}
          </span>
        </div>

        <div v-if="rulesets.length === 0" class="ruleset-row__empty">
          {{ t('test.project.noTests') }}
        </div>
      </div>

      <!-- Right: test results for selected ruleset -->
      <div class="test-view__result-panel">
        <div v-if="!selectedSummary" class="test-view__result-placeholder">
          <t-icon name="arrow-left" size="14px" />
          {{ t('contracts.rulesetsTitle') }}
        </div>

        <template v-else>
          <!-- Ruleset header -->
          <div class="result-panel__header">
            <span class="result-panel__name">{{ selectedSummary.ruleset_name }}</span>
            <span class="result-panel__stats">
              {{ selectedSummary.passed }}/{{ selectedSummary.total }} {{ t('test.statusPassed').toLowerCase() }}
            </span>
            <t-button
              size="small"
              variant="text"
              @click="goToEditor(selectedSummary.ruleset_name)"
            >
              <t-icon name="edit" size="12px" />
              {{ t('contracts.openEditor') }}
            </t-button>
          </div>

          <!-- Test result rows -->
          <div class="result-list">
            <div
              v-for="r in selectedSummary.results"
              :key="r.test_id"
              class="result-row"
              :class="r.passed ? 'result-row--pass' : 'result-row--fail'"
            >
              <span class="result-row__icon">{{ r.passed ? '✓' : '✗' }}</span>
              <div class="result-row__body">
                <span class="result-row__name">{{ r.test_name }}</span>
                <span v-if="!r.passed" class="result-row__failures">
                  {{ r.failures.join(' · ') }}
                </span>
                <span v-if="r.actual_code" class="result-row__code">
                  code: {{ r.actual_code }}
                </span>
              </div>
              <span class="result-row__duration">{{ durationMs(r.duration_us) }}</span>
            </div>

            <div v-if="selectedSummary.results.length === 0" class="result-list__empty">
              {{ t('test.noTests') }}
            </div>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.test-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
  background: var(--ordo-bg-main);
}

.test-view__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  padding: 20px 24px 16px;
  border-bottom: 1px solid var(--ordo-border-color);
  flex-shrink: 0;
  gap: 16px;
}

.test-view__header-right {
  display: flex;
  align-items: center;
  gap: 10px;
}

.test-view__title {
  font-size: 16px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  margin: 0 0 2px;
}

.test-view__subtitle {
  font-size: 12px;
  color: var(--ordo-text-tertiary);
  margin: 0;
}

.test-view__summary-badge {
  font-size: 13px;
  font-weight: 500;
  color: var(--ordo-text-secondary);
  background: var(--ordo-hover-bg);
  border-radius: 6px;
  padding: 4px 10px;
}

.test-view__empty,
.test-view__loading {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: var(--ordo-text-tertiary);
  font-size: 13px;
}

.test-view__content {
  flex: 1;
  display: flex;
  overflow: hidden;
}

/* ── Ruleset list ── */

.test-view__ruleset-list {
  width: 200px;
  flex-shrink: 0;
  border-right: 1px solid var(--ordo-border-color);
  overflow-y: auto;
  padding: 8px 0;
}

.ruleset-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 7px 14px;
  cursor: pointer;
  font-size: 12px;
  gap: 8px;
  transition: background 0.1s;
}

.ruleset-row:hover {
  background: var(--ordo-hover-bg);
}

.ruleset-row--active {
  background: var(--ordo-hover-bg);
  color: var(--ordo-accent);
}

.ruleset-row__name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: 'JetBrains Mono', monospace;
}

.ruleset-row__badge {
  font-size: 10px;
  font-weight: 600;
  padding: 1px 5px;
  border-radius: 3px;
}

.badge--pass {
  background: rgba(34, 197, 94, 0.15);
  color: #16a34a;
}

.badge--fail {
  background: rgba(239, 68, 68, 0.12);
  color: #dc2626;
}

.badge--gray {
  background: var(--ordo-hover-bg);
  color: var(--ordo-text-tertiary);
}

.ruleset-row__empty {
  padding: 14px;
  font-size: 12px;
  color: var(--ordo-text-tertiary);
}

/* ── Result panel ── */

.test-view__result-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.test-view__result-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  height: 100%;
  font-size: 13px;
  color: var(--ordo-text-tertiary);
}

.result-panel__header {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 16px;
  border-bottom: 1px solid var(--ordo-border-color);
  flex-shrink: 0;
  font-size: 12px;
}

.result-panel__name {
  font-family: 'JetBrains Mono', monospace;
  font-weight: 500;
  color: var(--ordo-text-primary);
}

.result-panel__stats {
  color: var(--ordo-text-tertiary);
  margin-right: auto;
}

.result-list {
  flex: 1;
  overflow-y: auto;
}

.result-row {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 7px 16px;
  font-size: 12px;
  border-bottom: 1px solid var(--ordo-border-color);
}

.result-row--pass {
  border-left: 2px solid #22c55e;
}

.result-row--fail {
  border-left: 2px solid #ef4444;
}

.result-row__icon {
  font-size: 11px;
  font-weight: bold;
  flex-shrink: 0;
  padding-top: 1px;
}

.result-row--pass .result-row__icon { color: #16a34a; }
.result-row--fail .result-row__icon { color: #dc2626; }

.result-row__body {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.result-row__name {
  font-weight: 500;
  color: var(--ordo-text-primary);
}

.result-row__failures {
  font-size: 11px;
  color: #ef4444;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.result-row__code {
  font-family: 'JetBrains Mono', monospace;
  font-size: 11px;
  color: var(--ordo-text-tertiary);
}

.result-row__duration {
  font-size: 10px;
  color: var(--ordo-text-tertiary);
  flex-shrink: 0;
}

.result-list__empty {
  padding: 16px;
  font-size: 12px;
  color: var(--ordo-text-tertiary);
}
</style>
