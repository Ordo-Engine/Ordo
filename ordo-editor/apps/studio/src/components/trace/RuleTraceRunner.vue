<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { MessagePlugin } from 'tdesign-vue-next'
import { convertToEngineFormat } from '@ordo-engine/editor-core'
import { engineApi, rulesetDraftApi, testApi } from '@/api/platform-client'
import { useAuthStore } from '@/stores/auth'
import { isEngineRuleset, normalizeRuleset } from '@/utils/ruleset'
import type { ProjectRulesetMeta, TestCase } from '@/api/types'

const props = defineProps<{
  orgId: string
  projectId: string
  rulesets: ProjectRulesetMeta[]
}>()

const { t } = useI18n()
const auth = useAuthStore()

// ── State ─────────────────────────────────────────────────────────────────────

const selectedRuleset = ref('')
const inputJson = ref('{\n  \n}')
const running = ref(false)

type TraceResult = {
  code: string
  message: string
  output: Record<string, unknown>
  duration_us: number
  trace?: {
    path: string
    steps: Array<{ id: string; name: string; duration_us: number }>
  }
}

const result = ref<TraceResult | null>(null)
const error = ref('')

// ── Test cases ────────────────────────────────────────────────────────────────

const testCases = ref<TestCase[]>([])
const testCasesLoading = ref(false)
const testCasesExpanded = ref(true)
const activeTestCase = ref<TestCase | null>(null)

watch(selectedRuleset, async (name) => {
  testCases.value = []
  activeTestCase.value = null
  result.value = null
  error.value = ''
  if (!name || !auth.token) return
  testCasesLoading.value = true
  try {
    testCases.value = await testApi.list(auth.token, props.projectId, name)
  } catch {
    // ignore — test cases are optional
  } finally {
    testCasesLoading.value = false
  }
})

function loadTestCase(tc: TestCase) {
  activeTestCase.value = tc
  inputJson.value = JSON.stringify(tc.input, null, 2)
}

// ── Execution history ──────────────────────────────────────────────────────────

type HistoryEntry = {
  rulesetName: string
  input: string
  result: TraceResult | null
  error: string
  timestamp: Date
}

const history = ref<HistoryEntry[]>([])
const rightTab = ref<'result' | 'history'>('result')

function addHistory(entry: Omit<HistoryEntry, 'timestamp'>) {
  history.value.unshift({ ...entry, timestamp: new Date() })
  if (history.value.length > 10) history.value.length = 10
}

function restoreHistory(entry: HistoryEntry) {
  selectedRuleset.value = entry.rulesetName
  inputJson.value = entry.input
  result.value = entry.result
  error.value = entry.error
  activeTestCase.value = null
  rightTab.value = 'result'
}

function formatHistoryTime(ts: Date) {
  return ts.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
}

// ── Save as test case ─────────────────────────────────────────────────────────

const saveDialogVisible = ref(false)
const saveName = ref('')
const saving = ref(false)

function openSaveDialog() {
  saveName.value = ''
  saveDialogVisible.value = true
}

async function saveAsTestCase() {
  if (!saveName.value.trim() || !result.value || !auth.token) return
  saving.value = true
  try {
    let parsed: Record<string, unknown> = {}
    try { parsed = JSON.parse(inputJson.value) } catch { /**/ }
    await testApi.create(auth.token, props.projectId, selectedRuleset.value, {
      name: saveName.value.trim(),
      input: parsed,
      expect: { code: result.value.code, output: result.value.output },
      tags: [],
    })
    // Refresh test cases
    testCases.value = await testApi.list(auth.token, props.projectId, selectedRuleset.value)
    MessagePlugin.success(t('trace.saveAsTestSuccess'))
    saveDialogVisible.value = false
  } catch (e: any) {
    MessagePlugin.error(e.message ?? String(e))
  } finally {
    saving.value = false
  }
}

// ── Execution ─────────────────────────────────────────────────────────────────

const expandedSteps = ref<Set<string>>(new Set())

const rulesetOptions = computed(() =>
  props.rulesets.map((r) => ({ label: r.name, value: r.name })),
)

function toggleStep(id: string) {
  if (expandedSteps.value.has(id)) {
    expandedSteps.value.delete(id)
  } else {
    expandedSteps.value.add(id)
  }
}

function stepIconName(idx: number, steps: unknown[]) {
  return idx === steps.length - 1 ? 'check-circle' : 'arrow-right-circle'
}

function formatDuration(us: number) {
  if (us < 1000) return `${us}µs`
  return `${(us / 1000).toFixed(1)}ms`
}

// Compare active test case expectation vs result
const expectationMatch = computed(() => {
  if (!activeTestCase.value || !result.value) return null
  const tc = activeTestCase.value
  const r = result.value
  const codeMatch = !tc.expect.code || tc.expect.code === r.code
  const outputDiffs: Array<{ key: string; expected: unknown; actual: unknown }> = []
  if (tc.expect.output) {
    for (const [k, v] of Object.entries(tc.expect.output)) {
      const actual = r.output?.[k]
      if (JSON.stringify(actual) !== JSON.stringify(v)) {
        outputDiffs.push({ key: k, expected: v, actual })
      }
    }
  }
  return { codeMatch, outputDiffs, pass: codeMatch && outputDiffs.length === 0 }
})

async function runTrace() {
  if (!selectedRuleset.value) {
    MessagePlugin.warning(t('trace.selectRuleset'))
    return
  }
  if (!auth.token) return

  let parsed: Record<string, unknown>
  try {
    parsed = JSON.parse(inputJson.value)
  } catch {
    MessagePlugin.error(t('trace.invalidJson'))
    return
  }

  running.value = true
  error.value = ''
  result.value = null
  rightTab.value = 'result'

  try {
    // Fetch draft and convert from editor format to engine format using the adapter.
    const draft = await rulesetDraftApi.get(auth.token, props.orgId, props.projectId, selectedRuleset.value)
    const engineRuleset = isEngineRuleset(draft.draft)
      ? draft.draft as unknown as Record<string, unknown>
      : convertToEngineFormat(normalizeRuleset(draft.draft, selectedRuleset.value)) as unknown as Record<string, unknown>

    result.value = await engineApi.executeWithTrace(
      auth.token,
      props.orgId,
      props.projectId,
      selectedRuleset.value,
      parsed,
      engineRuleset,
    )
    addHistory({ rulesetName: selectedRuleset.value, input: inputJson.value, result: result.value, error: '' })
  } catch (e: any) {
    error.value = e.message ?? String(e)
    MessagePlugin.error(error.value)
    addHistory({ rulesetName: selectedRuleset.value, input: inputJson.value, result: null, error: error.value })
  } finally {
    running.value = false
  }
}
</script>

<template>
  <div class="trace-runner">
    <!-- ── Left panel ── -->
    <div class="trace-runner__left">
      <div class="trace-panel">
        <div class="trace-panel__label">{{ t('trace.rulesetLabel') }}</div>
        <t-select
          v-model="selectedRuleset"
          :options="rulesetOptions"
          :placeholder="t('trace.selectRulesetPlaceholder')"
          filterable
          style="width: 100%; margin-bottom: 16px"
        />

        <div class="trace-panel__label">{{ t('trace.inputLabel') }}</div>
        <textarea
          v-model="inputJson"
          class="trace-json-input"
          rows="14"
          spellcheck="false"
        />

        <!-- Test cases collapsible -->
        <div v-if="selectedRuleset" class="test-cases-section">
          <button class="test-cases-header" @click="testCasesExpanded = !testCasesExpanded">
            <t-icon :name="testCasesExpanded ? 'chevron-down' : 'chevron-right'" size="12px" />
            <span>{{ t('trace.testCases') }}</span>
            <span v-if="testCases.length" class="test-cases-count">{{ testCases.length }}</span>
          </button>
          <div v-if="testCasesExpanded" class="test-cases-list">
            <div v-if="testCasesLoading" class="test-cases-loading">
              <t-loading size="small" />
            </div>
            <div v-else-if="testCases.length === 0" class="test-cases-empty">
              {{ t('trace.noTestCases') }}
            </div>
            <button
              v-for="tc in testCases"
              :key="tc.id"
              class="test-case-row"
              :class="{ 'test-case-row--active': activeTestCase?.id === tc.id }"
              @click="loadTestCase(tc)"
            >
              <t-icon name="file-paste" size="12px" class="test-case-row__icon" />
              <span class="test-case-row__name">{{ tc.name }}</span>
              <t-tag v-if="tc.expect.code" size="small" class="test-case-row__tag">{{ tc.expect.code }}</t-tag>
            </button>
          </div>
        </div>

        <t-button
          theme="primary"
          :loading="running"
          style="width: 100%; margin-top: 12px"
          @click="runTrace"
        >
          <template #icon><t-icon name="play-circle" /></template>
          {{ t('trace.runButton') }}
        </t-button>
      </div>
    </div>

    <!-- ── Right panel ── -->
    <div class="trace-runner__right">
      <!-- Tab bar -->
      <div class="right-tabs">
        <button
          class="right-tab"
          :class="{ 'right-tab--active': rightTab === 'result' }"
          @click="rightTab = 'result'"
        >{{ t('trace.tabResult') }}</button>
        <button
          class="right-tab"
          :class="{ 'right-tab--active': rightTab === 'history' }"
          @click="rightTab = 'history'"
        >
          {{ t('trace.history') }}
          <span v-if="history.length" class="right-tab__badge">{{ history.length }}</span>
        </button>
      </div>

      <!-- Result tab -->
      <template v-if="rightTab === 'result'">
        <div v-if="!result && !error && !running" class="trace-empty">
          <t-icon name="play-circle" size="40px" style="opacity: 0.2" />
          <p>{{ t('trace.noTrace') }}</p>
        </div>

        <div v-if="running" class="trace-empty">
          <t-loading />
        </div>

        <template v-if="result">
          <!-- Result card -->
          <div
            class="trace-result-card"
            :class="`trace-result-card--${result.code === 'error' ? 'error' : 'success'}`"
          >
            <div class="trace-result-card__code">
              <span class="code-badge">{{ result.code }}</span>
              <span class="trace-result-card__duration">{{ formatDuration(result.duration_us) }}</span>
              <span v-if="activeTestCase" class="test-case-label">
                <t-icon name="file-paste" size="12px" />
                {{ activeTestCase.name }}
              </span>
            </div>
            <div class="trace-result-card__message">{{ result.message }}</div>
            <pre v-if="Object.keys(result.output ?? {}).length > 0" class="trace-result-card__output">{{ JSON.stringify(result.output, null, 2) }}</pre>

            <!-- Expectation comparison (only when loaded from test case) -->
            <template v-if="expectationMatch">
              <div class="expect-divider">
                <span
                  class="expect-badge"
                  :class="expectationMatch.pass ? 'expect-badge--pass' : 'expect-badge--fail'"
                >
                  {{ expectationMatch.pass ? '✓ PASS' : '✗ FAIL' }}
                </span>
                <span class="expect-label">{{ t('trace.expected') }}: {{ activeTestCase?.expect.code }}</span>
              </div>
              <div v-if="expectationMatch.outputDiffs.length > 0" class="expect-diffs">
                <div
                  v-for="diff in expectationMatch.outputDiffs"
                  :key="diff.key"
                  class="expect-diff-row"
                >
                  <span class="expect-diff-key">{{ diff.key }}</span>
                  <span class="expect-diff-exp">{{ JSON.stringify(diff.expected) }}</span>
                  <t-icon name="arrow-right" size="10px" style="opacity:0.4" />
                  <span class="expect-diff-act">{{ JSON.stringify(diff.actual) }}</span>
                </div>
              </div>
            </template>

            <!-- Save as test case -->
            <div class="result-actions">
              <t-button
                size="small"
                variant="outline"
                @click="openSaveDialog"
              >
                <template #icon><t-icon name="save" /></template>
                {{ t('trace.saveAsTest') }}
              </t-button>
            </div>
          </div>

          <!-- Step waterfall -->
          <div v-if="result.trace && result.trace.steps.length > 0" class="trace-steps">
            <div class="trace-steps__header">
              <span>{{ t('trace.steps') }}</span>
              <span class="trace-steps__path">{{ result.trace.path }}</span>
            </div>
            <div
              v-for="(step, idx) in result.trace.steps"
              :key="step.id"
              class="trace-step"
              :class="{ 'trace-step--terminal': idx === result.trace!.steps.length - 1 }"
              @click="toggleStep(step.id)"
            >
              <span v-if="idx > 0" class="trace-step__connector" />
              <div class="trace-step__row">
                <t-icon :name="stepIconName(idx, result.trace!.steps)" size="14px" class="trace-step__icon" />
                <span class="trace-step__name">{{ step.name }}</span>
                <span class="trace-step__duration">{{ formatDuration(step.duration_us) }}</span>
              </div>
            </div>
          </div>
        </template>

        <div v-if="error" class="trace-error">
          <t-icon name="close-circle" size="16px" />
          <span>{{ error }}</span>
        </div>
      </template>

      <!-- History tab -->
      <template v-else-if="rightTab === 'history'">
        <div v-if="history.length === 0" class="trace-empty">
          <t-icon name="history" size="40px" style="opacity: 0.2" />
          <p>{{ t('trace.noHistory') }}</p>
        </div>
        <div
          v-for="(entry, idx) in history"
          :key="idx"
          class="history-entry"
          @click="restoreHistory(entry)"
        >
          <div class="history-entry__top">
            <span class="history-entry__ruleset">{{ entry.rulesetName }}</span>
            <span class="history-entry__time">{{ formatHistoryTime(entry.timestamp) }}</span>
          </div>
          <div class="history-entry__meta">
            <span
              v-if="entry.result"
              class="code-badge code-badge--sm"
              :style="entry.result.code === 'error' ? 'color:#ff4d4f' : ''"
            >{{ entry.result.code }}</span>
            <span v-else class="code-badge code-badge--sm" style="color:#ff4d4f">error</span>
            <span class="history-entry__duration" v-if="entry.result">{{ formatDuration(entry.result.duration_us) }}</span>
          </div>
        </div>
      </template>
    </div>
  </div>

  <!-- Save as test case dialog -->
  <t-dialog
    v-model:visible="saveDialogVisible"
    :header="t('trace.saveAsTestTitle')"
    :confirm-btn="t('common.save')"
    :cancel-btn="t('common.cancel')"
    :confirm-loading="saving"
    width="400px"
    @confirm="saveAsTestCase"
    @cancel="saveDialogVisible = false"
  >
    <t-form label-align="top" style="padding: 8px 0">
      <t-form-item :label="t('trace.testCaseName')">
        <t-input
          v-model="saveName"
          :placeholder="t('trace.testCaseNamePlaceholder')"
          autofocus
          @keydown.enter="saveAsTestCase"
        />
      </t-form-item>
    </t-form>
  </t-dialog>
</template>

<style scoped>
.trace-runner {
  display: grid;
  grid-template-columns: 380px 1fr;
  gap: 24px;
  height: 100%;
}

/* ── Left panel ── */
.trace-panel {
  display: flex;
  flex-direction: column;
  overflow-y: auto;
}

.trace-panel__label {
  font-size: 12px;
  font-weight: 600;
  color: var(--ordo-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin-bottom: 6px;
}

.trace-json-input {
  font-family: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace;
  font-size: 13px;
  line-height: 1.6;
  padding: 12px;
  border: 1px solid var(--ordo-border);
  border-radius: 8px;
  background: var(--ordo-bg-card);
  color: var(--ordo-text-primary);
  resize: vertical;
  width: 100%;
  box-sizing: border-box;
  outline: none;
  transition: border-color 0.15s;
}

.trace-json-input:focus {
  border-color: var(--ordo-accent);
}

/* ── Test cases ── */
.test-cases-section {
  margin-top: 14px;
  border: 1px solid var(--ordo-border);
  border-radius: 8px;
  overflow: hidden;
}

.test-cases-header {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  padding: 8px 10px;
  background: var(--ordo-bg-card);
  border: none;
  cursor: pointer;
  font-size: 12px;
  font-weight: 600;
  color: var(--ordo-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  text-align: left;
  transition: background 0.15s;
}

.test-cases-header:hover {
  background: var(--ordo-bg-hover);
}

.test-cases-count {
  margin-left: auto;
  font-size: 11px;
  background: var(--ordo-bg-app);
  padding: 1px 6px;
  border-radius: 10px;
  font-weight: 500;
  color: var(--ordo-text-tertiary);
}

.test-cases-list {
  border-top: 1px solid var(--ordo-border);
  max-height: 180px;
  overflow-y: auto;
}

.test-cases-loading,
.test-cases-empty {
  padding: 12px;
  font-size: 12px;
  color: var(--ordo-text-tertiary);
  text-align: center;
}

.test-case-row {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  padding: 7px 10px;
  background: none;
  border: none;
  border-bottom: 1px solid var(--ordo-border);
  cursor: pointer;
  font-size: 12px;
  color: var(--ordo-text-primary);
  text-align: left;
  transition: background 0.12s;
}

.test-case-row:last-child {
  border-bottom: none;
}

.test-case-row:hover {
  background: var(--ordo-bg-hover);
}

.test-case-row--active {
  background: color-mix(in srgb, var(--ordo-accent) 8%, transparent);
}

.test-case-row__icon {
  color: var(--ordo-text-tertiary);
  flex-shrink: 0;
}

.test-case-row__name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.test-case-row__tag {
  flex-shrink: 0;
  font-size: 10px;
}

/* ── Right panel ── */
.trace-runner__right {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-height: 0;
  overflow-y: auto;
}

.right-tabs {
  display: flex;
  gap: 2px;
  border-bottom: 1px solid var(--ordo-border);
  padding-bottom: 0;
  flex-shrink: 0;
}

.right-tab {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 6px 12px;
  background: none;
  border: none;
  border-bottom: 2px solid transparent;
  cursor: pointer;
  font-size: 13px;
  color: var(--ordo-text-secondary);
  transition: color 0.15s;
  margin-bottom: -1px;
}

.right-tab:hover {
  color: var(--ordo-text-primary);
}

.right-tab--active {
  color: var(--ordo-accent);
  border-bottom-color: var(--ordo-accent);
  font-weight: 500;
}

.right-tab__badge {
  font-size: 10px;
  background: var(--ordo-bg-app);
  padding: 1px 5px;
  border-radius: 8px;
  color: var(--ordo-text-tertiary);
}

/* ── Empty state ── */
.trace-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  height: 200px;
  color: var(--ordo-text-tertiary);
}

/* ── Result card ── */
.trace-result-card {
  border-radius: 10px;
  padding: 16px;
  border: 1px solid var(--ordo-border);
  background: var(--ordo-bg-card);
}

.trace-result-card--success {
  border-color: color-mix(in srgb, #52c41a 30%, transparent);
  background: color-mix(in srgb, #52c41a 4%, var(--ordo-bg-card));
}

.trace-result-card--error {
  border-color: color-mix(in srgb, #ff4d4f 30%, transparent);
  background: color-mix(in srgb, #ff4d4f 4%, var(--ordo-bg-card));
}

.trace-result-card__code {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
  flex-wrap: wrap;
}

.code-badge {
  font-family: monospace;
  font-size: 12px;
  font-weight: 700;
  padding: 2px 8px;
  border-radius: 4px;
  background: color-mix(in srgb, var(--ordo-accent) 15%, transparent);
  color: var(--ordo-accent);
}

.code-badge--sm {
  font-size: 11px;
  padding: 1px 6px;
}

.trace-result-card__duration {
  font-size: 12px;
  color: var(--ordo-text-tertiary);
}

.test-case-label {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: var(--ordo-text-tertiary);
  margin-left: auto;
}

.trace-result-card__message {
  font-size: 14px;
  margin-bottom: 8px;
}

.trace-result-card__output {
  font-family: monospace;
  font-size: 12px;
  background: var(--ordo-bg-app);
  border-radius: 6px;
  padding: 10px;
  margin: 0 0 12px;
  overflow: auto;
  max-height: 200px;
}

/* ── Expectation comparison ── */
.expect-divider {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
  padding-top: 4px;
}

.expect-badge {
  font-size: 11px;
  font-weight: 700;
  padding: 2px 8px;
  border-radius: 4px;
}

.expect-badge--pass {
  background: color-mix(in srgb, #52c41a 15%, transparent);
  color: #52c41a;
}

.expect-badge--fail {
  background: color-mix(in srgb, #ff4d4f 15%, transparent);
  color: #ff4d4f;
}

.expect-label {
  font-size: 12px;
  color: var(--ordo-text-tertiary);
}

.expect-diffs {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-bottom: 8px;
}

.expect-diff-row {
  display: flex;
  align-items: center;
  gap: 6px;
  font-family: monospace;
  font-size: 11px;
}

.expect-diff-key {
  font-weight: 600;
  color: var(--ordo-text-secondary);
  min-width: 80px;
}

.expect-diff-exp {
  color: #52c41a;
}

.expect-diff-act {
  color: #ff4d4f;
}

/* ── Result actions ── */
.result-actions {
  display: flex;
  justify-content: flex-end;
  margin-top: 10px;
  padding-top: 10px;
  border-top: 1px solid var(--ordo-border);
}

/* ── Step waterfall ── */
.trace-steps {
  border: 1px solid var(--ordo-border);
  border-radius: 10px;
  padding: 16px;
  background: var(--ordo-bg-card);
}

.trace-steps__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 12px;
  font-weight: 600;
  color: var(--ordo-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin-bottom: 12px;
}

.trace-steps__path {
  font-family: monospace;
  font-weight: 400;
  text-transform: none;
  letter-spacing: 0;
  color: var(--ordo-text-tertiary);
  font-size: 11px;
  max-width: 60%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.trace-step {
  position: relative;
  cursor: pointer;
}

.trace-step__connector {
  display: block;
  width: 1px;
  height: 8px;
  background: var(--ordo-border);
  margin-left: 6px;
}

.trace-step__row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: 6px;
  transition: background 0.15s;
}

.trace-step:hover .trace-step__row {
  background: var(--ordo-bg-hover);
}

.trace-step__icon {
  color: var(--ordo-text-tertiary);
  flex-shrink: 0;
}

.trace-step--terminal .trace-step__icon {
  color: #52c41a;
}

.trace-step__name {
  flex: 1;
  font-size: 13px;
}

.trace-step__duration {
  font-size: 11px;
  color: var(--ordo-text-tertiary);
  font-family: monospace;
}

/* ── Error ── */
.trace-error {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 16px;
  border-radius: 8px;
  background: color-mix(in srgb, #ff4d4f 8%, var(--ordo-bg-card));
  border: 1px solid color-mix(in srgb, #ff4d4f 30%, transparent);
  color: #ff4d4f;
  font-size: 13px;
}

/* ── History ── */
.history-entry {
  padding: 10px 14px;
  border: 1px solid var(--ordo-border);
  border-radius: 8px;
  background: var(--ordo-bg-card);
  cursor: pointer;
  transition: background 0.15s;
}

.history-entry:hover {
  background: var(--ordo-bg-hover);
}

.history-entry__top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 4px;
}

.history-entry__ruleset {
  font-size: 13px;
  font-weight: 500;
  color: var(--ordo-text-primary);
}

.history-entry__time {
  font-size: 11px;
  color: var(--ordo-text-tertiary);
  font-family: monospace;
}

.history-entry__meta {
  display: flex;
  align-items: center;
  gap: 8px;
}

.history-entry__duration {
  font-size: 11px;
  color: var(--ordo-text-tertiary);
  font-family: monospace;
}
</style>
