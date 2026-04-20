<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { MessagePlugin } from 'tdesign-vue-next'
import {
  OrdoDecisionTable,
  OrdoFlowEditor,
  decompileStepsToTable,
  type DecisionTable,
  type ExecutionTraceData,
  type RuleSet,
} from '@ordo-engine/editor-vue'
import { rulesetDraftApi } from '@/api/platform-client'
import { normalizeRuleset } from '@/utils/ruleset'
import { useAuthStore } from '@/stores/auth'
import { useProjectStore } from '@/stores/project'
import { useTestStore } from '@/stores/test'
import type {
  RulesetTestSummary,
  TestCase,
  TestExecutionTraceStep,
  TestRunResult,
} from '@/api/types'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const auth = useAuthStore()
const projectStore = useProjectStore()
const testStore = useTestStore()

const orgId = computed(() => route.params.orgId as string)
const projectId = computed(() => route.params.projectId as string)

const selectedRuleset = ref<string | null>(null)
const selectedTestId = ref<string | null>(null)
const selectedDetailTab = ref<'diff' | 'trace' | 'flow' | 'table'>('diff')
const selectedDraft = ref<RuleSet | null>(null)
const draftLoading = ref(false)
const contentContainer = ref<HTMLElement | null>(null)
const resultsPaneWidth = ref(520)
const isResizingResultsPane = ref(false)
const viewportWidth = ref(typeof window !== 'undefined' ? window.innerWidth : 1440)

const projectResult = computed(() => testStore.projectRunResult)
const running = computed(() => testStore.projectRunning)
const summaryMap = computed(
  () => new Map(projectResult.value?.rulesets.map((ruleset) => [ruleset.ruleset_name, ruleset]) ?? []),
)
const rulesetRows = computed(() =>
  projectStore.rulesets.map((ruleset) => ({
    name: ruleset.name,
    version: ruleset.version,
    summary: summaryMap.value.get(ruleset.name) ?? null,
  })),
)

const selectedSummary = computed<RulesetTestSummary | null>(
  () => summaryMap.value.get(selectedRuleset.value ?? '') ?? null,
)
const selectedTests = computed<TestCase[]>(
  () => testStore.testsByRuleset.get(selectedRuleset.value ?? '') ?? [],
)
const selectedResultsOrPending = computed(() => {
  if (selectedSummary.value?.results.length) {
    return selectedSummary.value.results.map((result) => ({
      kind: 'result' as const,
      key: result.test_id,
      name: result.test_name,
      duration_us: result.duration_us,
      actual_code: result.actual_code ?? null,
      passed: result.passed,
      failures: result.failures,
      result,
    }))
  }

  return selectedTests.value.map((test) => ({
    kind: 'pending' as const,
    key: test.id,
    name: test.name,
    duration_us: 0,
    actual_code: null,
    passed: false,
    failures: [],
    result: null,
  }))
})
const selectedResult = computed<TestRunResult | null>(() => {
  const summary = selectedSummary.value
  if (!summary) return null
  if (!summary.results.length) return null
  return summary.results.find((result) => result.test_id === selectedTestId.value) ?? summary.results[0]
})
const selectedTestCase = computed<TestCase | null>(
  () => selectedTests.value.find((test) => test.id === selectedResult.value?.test_id) ?? null,
)

const expectedCode = computed(() => selectedTestCase.value?.expect.code ?? null)
const expectedMessage = computed(() => selectedTestCase.value?.expect.message ?? null)
const expectedOutput = computed(() => selectedTestCase.value?.expect.output ?? null)
const actualCode = computed(() => selectedResult.value?.actual_code ?? null)
const actualMessage = computed(() => selectedResult.value?.actual_message ?? null)
const actualOutput = computed(() => selectedResult.value?.actual_output ?? null)
const rawExpected = computed(() => ({
  code: expectedCode.value,
  message: expectedMessage.value,
  output: expectedOutput.value ?? null,
}))
const rawActual = computed(() => ({
  code: actualCode.value,
  message: actualMessage.value,
  output: actualOutput.value ?? null,
}))

const outputDiffs = computed(() => {
  const expected = expectedOutput.value ?? {}
  const actual = actualOutput.value ?? {}
  const keys = Array.from(new Set([...Object.keys(expected), ...Object.keys(actual)])).sort()
  return keys.map((key) => {
    const expectedValue = key in expected ? expected[key] : undefined
    const actualValue = key in actual ? actual[key] : undefined
    return {
      key,
      expected: expectedValue,
      actual: actualValue,
      matches: isEqual(expectedValue, actualValue),
    }
  })
})

const traceSummary = computed(() => selectedResult.value?.trace ?? null)
const flowTrace = computed<ExecutionTraceData | null>(() => {
  const trace = traceSummary.value
  const result = selectedResult.value
  if (!trace || !result) return null
  return {
    path: trace.path,
    steps: trace.steps.map((step) => ({
      id: step.id,
      name: step.name,
      duration_us: step.duration_us,
      result: step.is_terminal ? (result.actual_code ?? trace.result_code) : null,
    })),
    resultCode: result.actual_code ?? trace.result_code,
    resultMessage: result.actual_message ?? '',
    output: result.actual_output ?? {},
  }
})
const selectedDecisionTable = computed<DecisionTable | null>(() => {
  const draft = selectedDraft.value
  if (!draft) return null
  return decompileStepsToTable(draft.steps, draft.startStepId) ?? null
})
const contentGridStyle = computed(() => {
  if (viewportWidth.value <= 1200) return {}
  return {
    gridTemplateColumns: `240px minmax(320px, ${resultsPaneWidth.value}px) 10px minmax(420px, 1fr)`,
  }
})

watch(selectedDecisionTable, (table) => {
  if (!table && selectedDetailTab.value === 'table') {
    selectedDetailTab.value = 'diff'
  }
})

watch(
  rulesetRows,
  (rows) => {
    if (rows.length === 0) {
      selectedRuleset.value = null
      return
    }
    if (!selectedRuleset.value || !rows.some((row) => row.name === selectedRuleset.value)) {
      selectedRuleset.value = rows[0].name
    }
  },
  { immediate: true },
)

watch(
  selectedSummary,
  (summary) => {
    if (!summary || summary.results.length === 0) {
      selectedTestId.value = null
      return
    }
    if (!selectedTestId.value || !summary.results.some((result) => result.test_id === selectedTestId.value)) {
      selectedTestId.value = summary.results[0].test_id
    }
  },
  { immediate: true },
)

watch(
  selectedRuleset,
  async (name) => {
    selectedDraft.value = null
    if (!name || !auth.token) return

    try {
      await testStore.fetchTests(projectId.value, name)
    } catch {
      // Test cases are optional for diagnostics.
    }

    draftLoading.value = true
    try {
      const draft = await rulesetDraftApi.get(auth.token, orgId.value, projectId.value, name)
      selectedDraft.value = normalizeRuleset(draft.draft, name)
    } catch {
      selectedDraft.value = null
    } finally {
      draftLoading.value = false
    }
  },
  { immediate: true },
)

async function runAll() {
  if (rulesetRows.value.length === 0) {
    MessagePlugin.warning(t('test.project.noRulesets'))
    return
  }
  try {
    await testStore.runProjectTests(
      orgId.value,
      projectId.value,
      rulesetRows.value.map((ruleset) => ruleset.name),
    )
    if (!selectedRuleset.value && rulesetRows.value.length > 0) {
      selectedRuleset.value = rulesetRows.value[0].name
    }
    MessagePlugin.success(t('test.runSuccess'))
  } catch (e: any) {
    MessagePlugin.error(e?.message ?? t('test.saveFailed'))
  }
}

function selectRuleset(name: string) {
  selectedRuleset.value = name
}

function selectResult(result: TestRunResult) {
  selectedTestId.value = result.test_id
}

function goToEditor(rulesetName: string) {
  router.push(`/orgs/${orgId.value}/projects/${projectId.value}/editor/${rulesetName}`)
}

function displayTestName(name: string): string {
  if (!name.startsWith('i18n:')) return name
  const key = name.slice(5)
  const fallback = key.split('.').pop() ?? key
  return fallback
    .replace(/[-_]+/g, ' ')
    .replace(/\b\w/g, (char) => char.toUpperCase())
}

function durationMs(us: number): string {
  return `${(us / 1000).toFixed(1)}ms`
}

function formatJson(value: unknown): string {
  if (value === undefined) return '—'
  if (value === null) return 'null'
  return JSON.stringify(value, null, 2)
}

function isEqual(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b)
}

function resultClass(result: TestRunResult): string {
  return result.passed ? 'is-pass' : 'is-fail'
}

function summaryBadgeClass(summary: RulesetTestSummary | null): string {
  if (!summary) return 'badge--idle'
  if (summary.total === 0) return 'badge--idle'
  if (summary.failed === 0) return 'badge--pass'
  return 'badge--fail'
}

function stepLabel(step: TestExecutionTraceStep, index: number): string {
  return step.name || step.id || `${index + 1}`
}

function stopResultsResize() {
  if (!isResizingResultsPane.value) return
  isResizingResultsPane.value = false
  document.body.style.cursor = ''
  document.body.style.userSelect = ''
}

function resizeResultsPane(event: MouseEvent) {
  if (!isResizingResultsPane.value || !contentContainer.value) return

  const bounds = contentContainer.value.getBoundingClientRect()
  const leftPaneWidth = 240
  const splitterWidth = 10
  const leftOffset = event.clientX - bounds.left
  const nextWidth = leftOffset - leftPaneWidth
  const minWidth = 320
  const minDiagnosticsWidth = 420
  const maxWidth = Math.max(
    minWidth,
    bounds.width - leftPaneWidth - splitterWidth - minDiagnosticsWidth,
  )

  resultsPaneWidth.value = Math.max(minWidth, Math.min(maxWidth, nextWidth))
}

function startResultsResize(event: MouseEvent) {
  isResizingResultsPane.value = true
  document.body.style.cursor = 'col-resize'
  document.body.style.userSelect = 'none'
  resizeResultsPane(event)
}

function syncViewportWidth() {
  viewportWidth.value = window.innerWidth
}

onMounted(() => {
  if (projectStore.currentProject?.id === projectId.value) {
    void projectStore.fetchRulesets()
  }
  window.addEventListener('resize', syncViewportWidth)
  window.addEventListener('mousemove', resizeResultsPane)
  window.addEventListener('mouseup', stopResultsResize)
})

onUnmounted(() => {
  window.removeEventListener('resize', syncViewportWidth)
  window.removeEventListener('mousemove', resizeResultsPane)
  window.removeEventListener('mouseup', stopResultsResize)
  stopResultsResize()
})
</script>

<template>
  <div class="test-view">
    <div class="test-view__header">
      <div>
        <h2 class="test-view__title">{{ t('test.project.title') }}</h2>
        <p class="test-view__subtitle">{{ t('test.project.diagnosticsSubtitle') }}</p>
      </div>
      <div class="test-view__header-actions">
        <span class="mode-chip">{{ t('test.project.localDraftMode') }}</span>
        <div v-if="projectResult" class="summary-chip">
          {{ t('test.project.summary', { passed: projectResult.passed, total: projectResult.total }) }}
        </div>
        <t-button theme="primary" :loading="running" @click="runAll">
          <t-icon name="play-circle" size="14px" />
          {{ t('test.project.runAll') }}
        </t-button>
      </div>
    </div>

    <div v-if="rulesetRows.length === 0" class="test-view__empty">
      <t-empty :title="t('test.project.noRulesets')" :description="t('test.project.noRulesetsHint')" />
    </div>

    <div v-else-if="running" class="test-view__loading">
      <t-loading size="medium" :text="t('test.project.runningLocal')" />
    </div>

    <div ref="contentContainer" v-else class="test-view__content" :style="contentGridStyle">
      <aside class="test-pane test-pane--rulesets">
        <div class="pane-title">{{ t('contracts.rulesetsTitle') }}</div>
        <button
          v-for="ruleset in rulesetRows"
          :key="ruleset.name"
          class="ruleset-row"
          :class="{ 'is-active': selectedRuleset === ruleset.name }"
          @click="selectRuleset(ruleset.name)"
        >
          <div class="ruleset-row__main">
            <span class="ruleset-row__name">{{ ruleset.name }}</span>
            <span class="ruleset-row__version">{{ ruleset.version }}</span>
          </div>
          <span class="ruleset-row__badge" :class="summaryBadgeClass(ruleset.summary)">
            <template v-if="ruleset.summary">{{ ruleset.summary.passed }}/{{ ruleset.summary.total }}</template>
            <template v-else>{{ t('test.statusPending') }}</template>
          </span>
        </button>
      </aside>

      <section class="test-pane test-pane--results">
        <template v-if="!selectedRuleset">
          <div class="placeholder-block">
            <t-empty :title="t('test.project.selectRuleset')" />
          </div>
        </template>

        <template v-else-if="!selectedSummary && selectedTests.length === 0">
          <div class="results-header">
            <div>
              <div class="results-header__name">{{ selectedRuleset }}</div>
              <div class="results-header__hint">{{ t('test.project.localDraftHint') }}</div>
            </div>
            <t-button size="small" variant="text" @click="goToEditor(selectedRuleset)">
              <t-icon name="edit" size="12px" />
              {{ t('contracts.openEditor') }}
            </t-button>
          </div>
          <div class="placeholder-block">
            <t-empty
              :title="t('test.project.runPromptTitle')"
              :description="t('test.project.runPromptBody')"
            />
          </div>
        </template>

        <template v-else>
          <div class="results-header">
            <div>
              <div class="results-header__name">{{ selectedSummary?.ruleset_name ?? selectedRuleset }}</div>
              <div class="results-header__hint">
                <template v-if="selectedSummary">
                  {{ t('test.project.summary', { passed: selectedSummary.passed, total: selectedSummary.total }) }}
                </template>
                <template v-else>
                  {{ t('test.project.testCount', { total: selectedTests.length }) }}
                </template>
              </div>
            </div>
            <t-button size="small" variant="text" @click="goToEditor(selectedSummary?.ruleset_name ?? selectedRuleset!)">
              <t-icon name="edit" size="12px" />
              {{ t('contracts.openEditor') }}
            </t-button>
          </div>

          <div v-if="selectedResultsOrPending.length === 0" class="placeholder-block">
            <t-empty :title="t('test.project.noTests')" />
          </div>

          <div v-else class="result-list">
            <button
              v-for="resultItem in selectedResultsOrPending"
              :key="resultItem.key"
              class="result-card"
              :class="[
                resultItem.kind === 'result' ? resultClass(resultItem.result!) : 'is-pending',
                { 'is-selected': selectedResult?.test_id === resultItem.result?.test_id },
              ]"
              @click="resultItem.result && selectResult(resultItem.result)"
            >
              <div class="result-card__header">
                <div>
                  <div class="result-card__name">{{ displayTestName(resultItem.name) }}</div>
                  <div class="result-card__meta">
                    <span>{{ resultItem.kind === 'result' ? durationMs(resultItem.duration_us) : t('test.statusPending') }}</span>
                    <span v-if="resultItem.actual_code">{{ resultItem.actual_code }}</span>
                  </div>
                </div>
                <span class="result-card__status">
                  {{
                    resultItem.kind === 'result'
                      ? (resultItem.passed ? t('test.statusPassed') : t('test.statusFailed'))
                      : t('test.statusPending')
                  }}
                </span>
              </div>
              <div v-if="resultItem.failures.length > 0" class="result-card__failures">
                {{ resultItem.failures.join(' · ') }}
              </div>
              <div v-else-if="resultItem.kind === 'pending'" class="result-card__pending-note">
                {{ t('test.project.pendingNote') }}
              </div>
            </button>
          </div>
        </template>
      </section>

      <div
        class="test-view__splitter"
        :class="{ 'is-active': isResizingResultsPane }"
        @mousedown="startResultsResize"
      />

      <aside class="test-pane test-pane--diagnostics">
        <template v-if="!selectedResult">
          <div class="placeholder-block">
            <t-empty
              :title="t('test.project.diagnosticsTitle')"
              :description="t('test.project.diagnosticsHint')"
            />
          </div>
        </template>

        <template v-else>
          <div class="diagnostics-header">
            <div>
              <div class="diagnostics-header__title">{{ displayTestName(selectedResult.test_name) }}</div>
              <div class="diagnostics-header__meta">
                <span>{{ durationMs(selectedResult.duration_us) }}</span>
                <span v-if="traceSummary">{{ traceSummary.path_string }}</span>
              </div>
            </div>
            <span class="diagnostics-header__status" :class="resultClass(selectedResult)">
              {{ selectedResult.passed ? t('test.statusPassed') : t('test.statusFailed') }}
            </span>
          </div>

          <div class="diagnostics-tabs">
            <button
              class="diagnostics-tab"
              :class="{ 'is-active': selectedDetailTab === 'diff' }"
              @click="selectedDetailTab = 'diff'"
            >
              {{ t('test.project.expectedActual') }}
            </button>
            <button
              class="diagnostics-tab"
              :class="{ 'is-active': selectedDetailTab === 'trace' }"
              @click="selectedDetailTab = 'trace'"
            >
              {{ t('test.project.traceDetails') }}
            </button>
            <button
              class="diagnostics-tab"
              :class="{ 'is-active': selectedDetailTab === 'flow' }"
              @click="selectedDetailTab = 'flow'"
            >
              {{ t('test.project.flowReplay') }}
            </button>
            <button
              v-if="selectedDecisionTable"
              class="diagnostics-tab"
              :class="{ 'is-active': selectedDetailTab === 'table' }"
              @click="selectedDetailTab = 'table'"
            >
              {{ t('test.project.decisionTableTrace') }}
            </button>
          </div>

          <div v-if="selectedDetailTab === 'diff'" class="diagnostics-body">
            <div v-if="selectedResult.failures.length > 0" class="section-card">
              <div class="section-card__title">{{ t('test.project.failures') }}</div>
              <div class="failure-list">
                <span
                  v-for="failure in selectedResult.failures"
                  :key="failure"
                  class="failure-pill"
                >
                  {{ failure }}
                </span>
              </div>
            </div>

            <div class="section-card">
              <div class="section-card__title">{{ t('test.project.code') }}</div>
              <div class="compare-grid">
                <div class="compare-block" :class="{ mismatch: !isEqual(expectedCode, actualCode) }">
                  <div class="compare-block__label">{{ t('test.result.expected') }}</div>
                  <pre>{{ formatJson(expectedCode) }}</pre>
                </div>
                <div class="compare-block" :class="{ mismatch: !isEqual(expectedCode, actualCode) }">
                  <div class="compare-block__label">{{ t('test.result.actual') }}</div>
                  <pre>{{ formatJson(actualCode) }}</pre>
                </div>
              </div>
            </div>

            <div class="section-card">
              <div class="section-card__title">{{ t('test.project.message') }}</div>
              <div class="compare-grid">
                <div class="compare-block" :class="{ mismatch: !isEqual(expectedMessage, actualMessage) }">
                  <div class="compare-block__label">{{ t('test.result.expected') }}</div>
                  <pre>{{ formatJson(expectedMessage) }}</pre>
                </div>
                <div class="compare-block" :class="{ mismatch: !isEqual(expectedMessage, actualMessage) }">
                  <div class="compare-block__label">{{ t('test.result.actual') }}</div>
                  <pre>{{ formatJson(actualMessage) }}</pre>
                </div>
              </div>
            </div>

            <div class="section-card">
              <div class="section-card__title">{{ t('test.project.output') }}</div>
              <div v-if="outputDiffs.length > 0" class="output-diff-list">
                <div
                  v-for="diff in outputDiffs"
                  :key="diff.key"
                  class="output-diff-row"
                  :class="{ mismatch: !diff.matches }"
                >
                  <div class="output-diff-row__key">{{ diff.key }}</div>
                  <div class="output-diff-row__values">
                    <pre>{{ formatJson(diff.expected) }}</pre>
                    <pre>{{ formatJson(diff.actual) }}</pre>
                  </div>
                </div>
              </div>
              <div v-else class="compare-grid">
                <div class="compare-block">
                  <div class="compare-block__label">{{ t('test.result.expected') }}</div>
                  <pre>{{ formatJson(expectedOutput) }}</pre>
                </div>
                <div class="compare-block">
                  <div class="compare-block__label">{{ t('test.result.actual') }}</div>
                  <pre>{{ formatJson(actualOutput) }}</pre>
                </div>
              </div>
            </div>

            <div class="section-card">
              <div class="section-card__title">{{ t('test.project.rawPayloads') }}</div>
              <div class="compare-grid">
                <div class="compare-block">
                  <div class="compare-block__label">{{ t('test.project.rawExpected') }}</div>
                  <pre>{{ formatJson(rawExpected) }}</pre>
                </div>
                <div class="compare-block">
                  <div class="compare-block__label">{{ t('test.project.rawActual') }}</div>
                  <pre>{{ formatJson(rawActual) }}</pre>
                </div>
              </div>
            </div>
          </div>

          <div v-else-if="selectedDetailTab === 'trace'" class="diagnostics-body">
            <div v-if="!traceSummary" class="placeholder-block">
              <t-empty :title="t('test.project.noTrace')" />
            </div>
            <template v-else>
              <div class="trace-summary">
                <div class="trace-summary__item">
                  <span>{{ t('test.project.path') }}</span>
                  <strong>{{ traceSummary.path_string }}</strong>
                </div>
                <div class="trace-summary__item">
                  <span>{{ t('test.project.totalDuration') }}</span>
                  <strong>{{ durationMs(traceSummary.total_duration_us) }}</strong>
                </div>
              </div>

              <div class="trace-step-list">
                <div
                  v-for="(step, index) in traceSummary.steps"
                  :key="`${step.id}-${index}`"
                  class="trace-step-card"
                >
                  <div class="trace-step-card__header">
                    <div>
                      <div class="trace-step-card__name">{{ stepLabel(step, index) }}</div>
                      <div class="trace-step-card__meta">
                        <span>{{ step.id }}</span>
                        <span>{{ durationMs(step.duration_us) }}</span>
                        <span v-if="step.next_step">{{ step.next_step }}</span>
                      </div>
                    </div>
                    <span v-if="step.is_terminal" class="trace-terminal">
                      {{ t('test.project.terminalStep') }}
                    </span>
                  </div>
                  <div class="trace-step-card__snapshots">
                    <div class="snapshot-block">
                      <div class="snapshot-block__label">{{ t('test.project.inputSnapshot') }}</div>
                      <pre>{{ formatJson(step.input_snapshot ?? null) }}</pre>
                    </div>
                    <div class="snapshot-block">
                      <div class="snapshot-block__label">{{ t('test.project.variablesSnapshot') }}</div>
                      <pre>{{ formatJson(step.variables_snapshot ?? null) }}</pre>
                    </div>
                  </div>
                </div>
              </div>
            </template>
          </div>

          <div v-else-if="selectedDetailTab === 'flow'" class="diagnostics-body diagnostics-body--flow">
            <div v-if="draftLoading" class="placeholder-block">
              <t-loading size="medium" :text="t('common.loading')" />
            </div>
            <div v-else-if="!selectedDraft || !flowTrace" class="placeholder-block">
              <t-empty
                :title="t('test.project.noFlowReplay')"
                :description="t('test.project.noFlowReplayHint')"
              />
            </div>
            <div v-else class="flow-replay">
              <OrdoFlowEditor
                :model-value="selectedDraft"
                :disabled="true"
                :execution-trace="flowTrace"
                :trace-mode="true"
              />
            </div>
          </div>

          <div v-else class="diagnostics-body diagnostics-body--table">
            <div v-if="!selectedDecisionTable || !selectedTestCase" class="placeholder-block">
              <t-empty
                :title="t('test.project.noDecisionTableTrace')"
                :description="t('test.project.noDecisionTableTraceHint')"
              />
            </div>
            <div v-else class="decision-table-trace">
              <OrdoDecisionTable
                :model-value="selectedDecisionTable"
                :disabled="true"
                :trace-input="selectedTestCase.input"
                :trace-result-code="selectedResult.actual_code ?? null"
                :trace-output="selectedResult.actual_output ?? null"
              />
            </div>
          </div>
        </template>
      </aside>
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
  gap: 16px;
  padding: 20px 24px 16px;
  border-bottom: 1px solid var(--ordo-border-color);
}

.test-view__header-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.test-view__title {
  margin: 0 0 4px;
  font-size: 18px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.test-view__subtitle {
  margin: 0;
  font-size: 12px;
  color: var(--ordo-text-tertiary);
}

.mode-chip,
.summary-chip {
  display: inline-flex;
  align-items: center;
  height: 28px;
  padding: 0 10px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 500;
}

.mode-chip {
  background: rgba(36, 104, 242, 0.08);
  color: var(--ordo-accent);
}

.summary-chip {
  background: var(--ordo-hover-bg);
  color: var(--ordo-text-secondary);
}

.test-view__empty,
.test-view__loading {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

.test-view__content {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 240px minmax(320px, 520px) 10px minmax(420px, 1fr);
}

.test-pane {
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--ordo-border-color);
}

.test-pane--diagnostics {
  border-right: none;
}

.test-view__splitter {
  position: relative;
  cursor: col-resize;
  background: transparent;
}

.test-view__splitter::before {
  content: '';
  position: absolute;
  top: 0;
  bottom: 0;
  left: 4px;
  width: 2px;
  background: var(--ordo-border-color);
  transition: background 0.15s ease;
}

.test-view__splitter:hover::before,
.test-view__splitter.is-active::before {
  background: var(--ordo-accent);
}

.pane-title {
  padding: 14px 16px 10px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--ordo-text-tertiary);
}

.ruleset-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 12px 16px;
  border: none;
  background: transparent;
  text-align: left;
  cursor: pointer;
  border-top: 1px solid transparent;
  border-bottom: 1px solid transparent;
}

.ruleset-row:hover,
.ruleset-row.is-active {
  background: var(--ordo-hover-bg);
}

.ruleset-row.is-active {
  border-color: rgba(36, 104, 242, 0.16);
}

.ruleset-row__main {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.ruleset-row__name {
  font-size: 13px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.ruleset-row__version {
  font-size: 11px;
  color: var(--ordo-text-tertiary);
  font-family: 'JetBrains Mono', monospace;
}

.ruleset-row__badge {
  flex-shrink: 0;
  border-radius: 999px;
  padding: 4px 8px;
  font-size: 11px;
  font-weight: 600;
}

.badge--idle {
  color: var(--ordo-text-tertiary);
  background: var(--ordo-hover-bg);
}

.badge--pass {
  color: #117a44;
  background: rgba(17, 122, 68, 0.12);
}

.badge--fail {
  color: #b83a2d;
  background: rgba(184, 58, 45, 0.12);
}

.results-header,
.diagnostics-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 16px;
  border-bottom: 1px solid var(--ordo-border-color);
}

.results-header__name,
.diagnostics-header__title {
  font-size: 15px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.results-header__hint,
.diagnostics-header__meta {
  margin-top: 4px;
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  font-size: 12px;
  color: var(--ordo-text-tertiary);
}

.result-list,
.diagnostics-body,
.trace-step-list {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.result-list {
  padding: 12px;
}

.result-card {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 14px;
  margin-bottom: 10px;
  border-radius: 12px;
  border: 1px solid var(--ordo-border-color);
  background: var(--ordo-bg-panel);
  text-align: left;
  cursor: pointer;
}

.result-card.is-selected {
  border-color: rgba(36, 104, 242, 0.35);
  box-shadow: 0 0 0 1px rgba(36, 104, 242, 0.08);
}

.result-card__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.result-card__name {
  font-size: 13px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.result-card__meta {
  margin-top: 5px;
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  font-size: 11px;
  color: var(--ordo-text-tertiary);
}

.result-card__status,
.diagnostics-header__status {
  flex-shrink: 0;
  border-radius: 999px;
  padding: 4px 9px;
  font-size: 11px;
  font-weight: 600;
}

.is-pass .result-card__status,
.diagnostics-header__status.is-pass {
  color: #117a44;
  background: rgba(17, 122, 68, 0.12);
}

.is-fail .result-card__status,
.diagnostics-header__status.is-fail {
  color: #b83a2d;
  background: rgba(184, 58, 45, 0.12);
}

.is-pending .result-card__status {
  color: var(--ordo-text-tertiary);
  background: var(--ordo-hover-bg);
}

.result-card__failures {
  font-size: 12px;
  color: #b83a2d;
  line-height: 1.5;
}

.result-card__pending-note {
  font-size: 12px;
  color: var(--ordo-text-tertiary);
  line-height: 1.5;
}

.diagnostics-tabs {
  display: flex;
  gap: 6px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--ordo-border-color);
}

.diagnostics-tab {
  border: 1px solid var(--ordo-border-color);
  background: transparent;
  color: var(--ordo-text-secondary);
  border-radius: 999px;
  padding: 6px 12px;
  font-size: 12px;
  cursor: pointer;
}

.diagnostics-tab.is-active {
  color: var(--ordo-accent);
  border-color: rgba(36, 104, 242, 0.24);
  background: rgba(36, 104, 242, 0.06);
}

.diagnostics-body {
  padding: 16px;
}

.section-card,
.trace-step-card {
  border: 1px solid var(--ordo-border-color);
  background: var(--ordo-bg-panel);
  border-radius: 12px;
  padding: 14px;
  margin-bottom: 12px;
}

.section-card__title {
  font-size: 12px;
  font-weight: 600;
  color: var(--ordo-text-primary);
  margin-bottom: 12px;
}

.failure-list {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.failure-pill {
  border-radius: 999px;
  padding: 5px 10px;
  font-size: 11px;
  color: #b83a2d;
  background: rgba(184, 58, 45, 0.12);
}

.compare-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}

.compare-block,
.snapshot-block {
  min-width: 0;
  border-radius: 10px;
  background: rgba(15, 23, 42, 0.03);
  border: 1px solid transparent;
  padding: 10px;
}

.compare-block.mismatch,
.output-diff-row.mismatch {
  border-color: rgba(184, 58, 45, 0.22);
  background: rgba(184, 58, 45, 0.05);
}

.compare-block__label,
.snapshot-block__label {
  margin-bottom: 6px;
  font-size: 11px;
  font-weight: 600;
  color: var(--ordo-text-tertiary);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

pre {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  font-family: 'JetBrains Mono', monospace;
  font-size: 11px;
  line-height: 1.55;
  color: var(--ordo-text-secondary);
}

.output-diff-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.output-diff-row {
  border: 1px solid var(--ordo-border-color);
  border-radius: 10px;
  padding: 10px;
}

.output-diff-row__key {
  margin-bottom: 8px;
  font-size: 12px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.output-diff-row__values {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}

.trace-summary {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
  margin-bottom: 12px;
}

.trace-summary__item {
  border-radius: 10px;
  background: rgba(15, 23, 42, 0.03);
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  font-size: 12px;
  color: var(--ordo-text-tertiary);
}

.trace-summary__item strong {
  color: var(--ordo-text-primary);
}

.trace-step-card__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 12px;
}

.trace-step-card__name {
  font-size: 13px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.trace-step-card__meta {
  margin-top: 5px;
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  font-size: 11px;
  color: var(--ordo-text-tertiary);
}

.trace-terminal {
  border-radius: 999px;
  padding: 4px 8px;
  font-size: 11px;
  background: rgba(36, 104, 242, 0.08);
  color: var(--ordo-accent);
}

.trace-step-card__snapshots {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}

.placeholder-block {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 240px;
  padding: 24px;
}

.diagnostics-body--flow,
.diagnostics-body--table,
.flow-replay,
.decision-table-trace {
  height: 100%;
  min-height: 0;
}

.flow-replay {
  border: 1px solid var(--ordo-border-color);
  border-radius: 12px;
  overflow: hidden;
}

.decision-table-trace {
  border: 1px solid var(--ordo-border-color);
  border-radius: 12px;
  overflow: auto;
  background: var(--ordo-bg-panel);
  padding: 12px;
}

@media (max-width: 1500px) {
  .test-view__content {
    grid-template-columns: 220px minmax(320px, 520px) 10px minmax(360px, 1fr);
  }
}

@media (max-width: 1200px) {
  .test-view__content {
    grid-template-columns: 220px 1fr;
  }

  .test-view__splitter {
    display: none;
  }

  .test-pane--diagnostics {
    grid-column: 1 / -1;
    border-top: 1px solid var(--ordo-border-color);
  }
}
</style>
