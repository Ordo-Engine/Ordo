<script setup lang="ts">
/**
 * OrdoExecutionPanel - Bottom panel for rule execution (terminal-like)
 * 规则执行底部面板（类似终端）
 */
import { ref, computed, watch } from 'vue';
import type { RuleSet } from '@ordo/editor-core';
import { RuleExecutor, type ExecutionResult, type ExecutionOptions } from '@ordo/editor-core';
import { useI18n } from '../../locale';

export interface Props {
  /** RuleSet to execute */
  ruleset: RuleSet;
  /** Sample input data for this ruleset */
  sampleInput?: string;
  /** Panel height */
  height?: number;
  /** Whether the panel is visible */
  visible?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  sampleInput: '{\n  \n}',
  height: 300,
  visible: false,
});

const emit = defineEmits<{
  'update:visible': [value: boolean];
  'update:height': [value: number];
  'show-in-flow': [
    trace: {
      path: string[];
      steps: Array<{ id: string; name: string; duration_us: number; result?: string | null }>;
      resultCode: string;
      resultMessage: string;
      output?: Record<string, any>;
    },
  ];
  'clear-flow-trace': [];
}>();

const { t } = useI18n();

// Execution state
const executor = new RuleExecutor();
const executing = ref(false);
const result = ref<ExecutionResult | null>(null);
const error = ref<string | null>(null);
const executionHistory = ref<
  Array<{ input: string; result: ExecutionResult | null; error: string | null; timestamp: Date }>
>([]);

// Input
const inputJson = ref(props.sampleInput);

// Execution options
const executionMode = ref<'wasm' | 'http'>('wasm');
const httpEndpoint = ref('http://localhost:8080');
const includeTrace = ref(true);

// Panel state
const activeTab = ref<'input' | 'output' | 'trace' | 'history'>('input');
const panelHeight = ref(props.height);
const isResizing = ref(false);
const startY = ref(0);
const startHeight = ref(0);

// Watch for sample input changes
watch(
  () => props.sampleInput,
  (newVal) => {
    if (newVal && inputJson.value === '{\n  \n}') {
      inputJson.value = newVal;
    }
  }
);

// Watch for visibility changes
watch(
  () => props.visible,
  (newVal) => {
    if (newVal && inputJson.value === '{\n  \n}' && props.sampleInput) {
      inputJson.value = props.sampleInput;
    }
  }
);

// Computed
const isVisible = computed({
  get: () => props.visible,
  set: (value) => emit('update:visible', value),
});

const resultClass = computed(() => {
  if (!result.value) return '';
  return result.value.code.includes('ERROR') ||
    result.value.code.includes('FAIL') ||
    result.value.code.includes('BLOCK')
    ? 'result-error'
    : 'result-success';
});

// Methods
function close() {
  isVisible.value = false;
}

function loadSampleInput() {
  if (props.sampleInput) {
    inputJson.value = props.sampleInput;
  }
}

async function execute() {
  if (executing.value) return;

  // Clear previous results
  result.value = null;
  error.value = null;

  // Parse input
  let input: Record<string, any>;
  try {
    input = JSON.parse(inputJson.value);
  } catch (e) {
    error.value = `JSON Parse Error: ${e instanceof Error ? e.message : 'Unknown error'}`;
    activeTab.value = 'output';
    return;
  }

  // Execute
  executing.value = true;
  try {
    const options: ExecutionOptions = {
      mode: executionMode.value,
      httpEndpoint: executionMode.value === 'http' ? httpEndpoint.value : undefined,
      includeTrace: includeTrace.value,
    };

    result.value = await executor.execute(props.ruleset, input, options);
    activeTab.value = 'output';

    // Add to history
    executionHistory.value.unshift({
      input: inputJson.value,
      result: result.value,
      error: null,
      timestamp: new Date(),
    });
  } catch (e) {
    error.value = `Execution Error: ${e instanceof Error ? e.message : 'Unknown error'}`;
    activeTab.value = 'output';

    // Add to history
    executionHistory.value.unshift({
      input: inputJson.value,
      result: null,
      error: error.value,
      timestamp: new Date(),
    });
  } finally {
    executing.value = false;
  }

  // Limit history
  if (executionHistory.value.length > 20) {
    executionHistory.value = executionHistory.value.slice(0, 20);
  }
}

function formatOutput(output: any): string {
  return JSON.stringify(output, null, 2);
}

function clearHistory() {
  executionHistory.value = [];
}

function loadFromHistory(item: (typeof executionHistory.value)[0]) {
  inputJson.value = item.input;
  result.value = item.result;
  error.value = item.error;
  activeTab.value = 'output';
}

// Show execution trace in flow diagram
const isShowingInFlow = ref(false);

function showInFlow(execResult: ExecutionResult | null) {
  if (!execResult || !execResult.trace) return;

  // Parse the path string into array
  const pathStr = execResult.trace.path || '';
  const pathArray = pathStr.split(' -> ').filter((s) => s.trim());

  emit('show-in-flow', {
    path: pathArray,
    steps: execResult.trace.steps || [],
    resultCode: execResult.code,
    resultMessage: execResult.message,
    output: execResult.output as Record<string, any>,
  });

  isShowingInFlow.value = true;
}

function clearFlowTrace() {
  emit('clear-flow-trace');
  isShowingInFlow.value = false;
}

// Resize handlers
function startResize(e: MouseEvent) {
  isResizing.value = true;
  startY.value = e.clientY;
  startHeight.value = panelHeight.value;
  document.addEventListener('mousemove', handleResize);
  document.addEventListener('mouseup', stopResize);
  e.preventDefault();
}

function handleResize(e: MouseEvent) {
  if (!isResizing.value) return;
  const delta = startY.value - e.clientY;
  const newHeight = Math.max(150, Math.min(600, startHeight.value + delta));
  panelHeight.value = newHeight;
  emit('update:height', newHeight);
}

function stopResize() {
  isResizing.value = false;
  document.removeEventListener('mousemove', handleResize);
  document.removeEventListener('mouseup', stopResize);
}
</script>

<template>
  <Transition name="panel">
    <div v-if="isVisible" class="execution-panel" :style="{ height: `${panelHeight}px` }">
      <!-- Resize Handle -->
      <div class="resize-handle" @mousedown="startResize">
        <div class="resize-bar"></div>
      </div>

      <!-- Panel Header -->
      <div class="panel-header">
        <div class="panel-tabs">
          <button
            class="tab-btn"
            :class="{ active: activeTab === 'input' }"
            @click="activeTab = 'input'"
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <path d="M4 7V4h16v3" />
              <path d="M9 20h6" />
              <path d="M12 4v16" />
            </svg>
            {{ t('execution.input') }}
          </button>
          <button
            class="tab-btn"
            :class="{ active: activeTab === 'output' }"
            @click="activeTab = 'output'"
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <polyline points="4 17 10 11 4 5" />
              <line x1="12" y1="19" x2="20" y2="19" />
            </svg>
            {{ t('execution.output') }}
            <span v-if="result" class="result-badge" :class="resultClass">{{ result.code }}</span>
          </button>
          <button
            v-if="result?.trace"
            class="tab-btn"
            :class="{ active: activeTab === 'trace' }"
            @click="activeTab = 'trace'"
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <circle cx="12" cy="12" r="10" />
              <polyline points="12 6 12 12 16 14" />
            </svg>
            {{ t('execution.trace') }}
          </button>
          <button
            class="tab-btn"
            :class="{ active: activeTab === 'history' }"
            @click="activeTab = 'history'"
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <path d="M3 3v5h5" />
              <path d="M3.05 13A9 9 0 1 0 6 5.3L3 8" />
            </svg>
            {{ t('execution.history') }}
            <span v-if="executionHistory.length" class="count-badge">{{
              executionHistory.length
            }}</span>
          </button>
        </div>

        <div class="panel-actions">
          <select v-model="executionMode" class="mode-select" :title="t('execution.mode')">
            <option value="wasm">WASM</option>
            <option value="http">HTTP</option>
          </select>

          <label class="trace-checkbox" :title="t('execution.includeTrace')">
            <input type="checkbox" v-model="includeTrace" />
            <span>Trace</span>
          </label>

          <button
            class="execute-btn"
            :class="{ executing }"
            :disabled="executing"
            @click="execute"
            :title="t('execution.execute')"
          >
            <svg v-if="!executing" width="12" height="12" viewBox="0 0 24 24" fill="currentColor">
              <path d="M8 5v14l11-7z" />
            </svg>
            <svg v-else class="spinner" width="12" height="12" viewBox="0 0 24 24">
              <circle
                cx="12"
                cy="12"
                r="10"
                stroke="currentColor"
                stroke-width="3"
                fill="none"
                opacity="0.25"
              />
              <path
                d="M12 2a10 10 0 0 1 10 10"
                stroke="currentColor"
                stroke-width="3"
                fill="none"
              />
            </svg>
            {{ executing ? t('execution.executing') : t('execution.execute') }}
          </button>

          <button class="close-btn" @click="close" :title="t('common.close')">
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>
      </div>

      <!-- Panel Content -->
      <div class="panel-content">
        <!-- Input Tab -->
        <div v-show="activeTab === 'input'" class="tab-content input-tab">
          <div class="input-toolbar">
            <button class="toolbar-btn" @click="loadSampleInput" :title="t('execution.loadSample')">
              <svg
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
              >
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                <polyline points="7 10 12 15 17 10" />
                <line x1="12" y1="15" x2="12" y2="3" />
              </svg>
              {{ t('execution.loadSample') }}
            </button>
          </div>
          <textarea
            v-model="inputJson"
            class="input-editor"
            :placeholder="t('execution.inputPlaceholder')"
            spellcheck="false"
          />
        </div>

        <!-- Output Tab -->
        <div v-show="activeTab === 'output'" class="tab-content output-tab">
          <div v-if="result" class="result-display">
            <div class="result-header">
              <span class="code-badge" :class="resultClass">{{ result.code }}</span>
              <span class="message">{{ result.message }}</span>
              <span class="duration">{{ result.duration_us }}µs</span>
              <button
                v-if="result.trace"
                class="show-in-flow-btn-lg"
                :class="{ active: isShowingInFlow }"
                @click="isShowingInFlow ? clearFlowTrace() : showInFlow(result)"
              >
                <svg
                  width="14"
                  height="14"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                >
                  <path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z" />
                  <path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z" />
                </svg>
                {{ isShowingInFlow ? t('execution.hideFromFlow') : t('execution.showInFlow') }}
              </button>
            </div>
            <pre class="output-json">{{ formatOutput(result.output) }}</pre>
          </div>
          <div v-else-if="error" class="error-display">
            <pre class="error-text">{{ error }}</pre>
          </div>
          <div v-else class="no-output">
            <svg
              width="32"
              height="32"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              opacity="0.3"
            >
              <polyline points="4 17 10 11 4 5" />
              <line x1="12" y1="19" x2="20" y2="19" />
            </svg>
            <p>{{ t('execution.noResult') }}</p>
          </div>
        </div>

        <!-- Trace Tab -->
        <div v-show="activeTab === 'trace'" class="tab-content trace-tab">
          <div v-if="result?.trace" class="trace-display">
            <div class="trace-path"><strong>Path:</strong> {{ result.trace.path }}</div>
            <div class="trace-steps">
              <div v-for="(step, index) in result.trace.steps" :key="step.id" class="trace-step">
                <span class="step-index">{{ index + 1 }}</span>
                <span class="step-id">{{ step.id }}</span>
                <span class="step-name">{{ step.name }}</span>
                <span class="step-duration">{{ step.duration_us }}µs</span>
              </div>
            </div>
          </div>
          <div v-else class="no-trace">
            <p>{{ t('execution.noTrace') }}</p>
          </div>
        </div>

        <!-- History Tab -->
        <div v-show="activeTab === 'history'" class="tab-content history-tab">
          <div v-if="executionHistory.length" class="history-list">
            <div class="history-toolbar">
              <button class="toolbar-btn" @click="clearHistory">
                <svg
                  width="12"
                  height="12"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                >
                  <polyline points="3 6 5 6 21 6" />
                  <path
                    d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"
                  />
                </svg>
                {{ t('execution.clearHistory') }}
              </button>
            </div>
            <div
              v-for="(item, index) in executionHistory"
              :key="index"
              class="history-item"
              @click="loadFromHistory(item)"
            >
              <span class="history-time">{{ item.timestamp.toLocaleTimeString() }}</span>
              <span
                v-if="item.result"
                class="history-code"
                :class="item.result.code.includes('ERROR') ? 'error' : 'success'"
              >
                {{ item.result.code }}
              </span>
              <span v-else class="history-code error">ERROR</span>
              <span class="history-duration" v-if="item.result"
                >{{ item.result.duration_us }}µs</span
              >
              <button
                v-if="item.result?.trace"
                class="show-in-flow-btn"
                @click.stop="showInFlow(item.result)"
                :title="t('execution.showInFlow')"
              >
                <svg
                  width="12"
                  height="12"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                >
                  <path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z" />
                  <path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z" />
                </svg>
              </button>
            </div>
          </div>
          <div v-else class="no-history">
            <p>{{ t('execution.noHistory') }}</p>
          </div>
        </div>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
/* Panel Transition */
.panel-enter-active,
.panel-leave-active {
  transition:
    transform 0.2s ease,
    opacity 0.2s ease;
}

.panel-enter-from,
.panel-leave-to {
  transform: translateY(100%);
  opacity: 0;
}

/* Panel Container */
.execution-panel {
  position: relative;
  display: flex;
  flex-direction: column;
  background: var(--ordo-bg-panel);
  border-top: 1px solid var(--ordo-border-color);
}

/* Resize Handle */
.resize-handle {
  position: absolute;
  top: -4px;
  left: 0;
  right: 0;
  height: 8px;
  cursor: ns-resize;
  z-index: 10;
}

.resize-bar {
  position: absolute;
  top: 3px;
  left: 50%;
  transform: translateX(-50%);
  width: 40px;
  height: 3px;
  background: var(--ordo-border-color);
  border-radius: 2px;
  opacity: 0;
  transition: opacity 0.15s;
}

.resize-handle:hover .resize-bar {
  opacity: 1;
}

/* Panel Header */
.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 12px;
  height: 36px;
  background: var(--ordo-bg-secondary, var(--ordo-bg-panel));
  border-bottom: 1px solid var(--ordo-border-color);
  flex-shrink: 0;
}

.panel-tabs {
  display: flex;
  align-items: center;
  gap: 2px;
}

.tab-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  font-size: 12px;
  font-weight: 500;
  color: var(--ordo-text-secondary);
  background: transparent;
  border: none;
  border-radius: var(--ordo-radius-sm);
  cursor: pointer;
  transition: all 0.15s;
}

.tab-btn:hover {
  color: var(--ordo-text-primary);
  background: var(--ordo-bg-item-hover);
}

.tab-btn.active {
  color: var(--ordo-accent);
  background: var(--ordo-accent-bg);
}

.result-badge {
  font-size: 10px;
  font-weight: 600;
  padding: 1px 5px;
  border-radius: 3px;
  background: var(--ordo-success-bg, rgba(78, 201, 105, 0.15));
  color: var(--ordo-success, #4ec969);
}

.result-badge.result-error {
  background: var(--ordo-error-bg, rgba(231, 76, 60, 0.15));
  color: var(--ordo-error, #e74c3c);
}

.count-badge {
  font-size: 10px;
  font-weight: 600;
  padding: 1px 5px;
  border-radius: 3px;
  background: var(--ordo-bg-item);
  color: var(--ordo-text-tertiary);
}

/* Panel Actions */
.panel-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.mode-select {
  padding: 4px 8px;
  font-size: 11px;
  color: var(--ordo-text-secondary);
  background: var(--ordo-bg-editor);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
}

.trace-checkbox {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: var(--ordo-text-secondary);
  cursor: pointer;
}

.trace-checkbox input {
  cursor: pointer;
}

.execute-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 12px;
  font-size: 12px;
  font-weight: 600;
  color: #fff;
  background: var(--ordo-success, #4ec969);
  border: none;
  border-radius: var(--ordo-radius-sm);
  cursor: pointer;
  transition: all 0.15s;
}

.execute-btn:hover:not(:disabled) {
  opacity: 0.9;
}

.execute-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.execute-btn .spinner {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.close-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: none;
  border-radius: var(--ordo-radius-sm);
  background: transparent;
  color: var(--ordo-text-tertiary);
  cursor: pointer;
  transition: all 0.15s;
}

.close-btn:hover {
  background: var(--ordo-bg-item-hover);
  color: var(--ordo-text-primary);
}

/* Panel Content */
.panel-content {
  flex: 1;
  overflow: hidden;
  min-height: 0;
}

.tab-content {
  height: 100%;
  overflow: auto;
}

/* Input Tab */
.input-tab {
  display: flex;
  flex-direction: column;
}

.input-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--ordo-border-light);
  flex-shrink: 0;
}

.toolbar-btn {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 8px;
  font-size: 11px;
  color: var(--ordo-text-secondary);
  background: var(--ordo-bg-editor);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  cursor: pointer;
  transition: all 0.15s;
}

.toolbar-btn:hover {
  color: var(--ordo-text-primary);
  background: var(--ordo-bg-item-hover);
}

.input-editor {
  flex: 1;
  width: 100%;
  padding: 12px;
  font-family: var(--ordo-font-mono, 'Consolas', 'Monaco', monospace);
  font-size: 12px;
  line-height: 1.5;
  color: var(--ordo-text-primary);
  background: var(--ordo-bg-editor);
  border: none;
  resize: none;
}

.input-editor:focus {
  outline: none;
}

/* Output Tab */
.output-tab {
  padding: 12px;
}

.result-display {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.result-header {
  display: flex;
  align-items: center;
  gap: 12px;
}

.code-badge {
  font-family: var(--ordo-font-mono);
  font-size: 12px;
  font-weight: 700;
  padding: 3px 8px;
  background: var(--ordo-success-bg, rgba(78, 201, 105, 0.15));
  color: var(--ordo-success, #4ec969);
  border-radius: 4px;
}

.code-badge.result-error {
  background: var(--ordo-error-bg, rgba(231, 76, 60, 0.15));
  color: var(--ordo-error, #e74c3c);
}

.result-header .message {
  flex: 1;
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.result-header .duration {
  font-family: var(--ordo-font-mono);
  font-size: 11px;
  color: var(--ordo-text-tertiary);
  padding: 2px 6px;
  background: var(--ordo-bg-item);
  border-radius: 3px;
}

.show-in-flow-btn-lg {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  font-size: 11px;
  font-weight: 500;
  color: var(--ordo-text-secondary);
  background: var(--ordo-bg-item);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  cursor: pointer;
  transition: all 0.15s;
}

.show-in-flow-btn-lg:hover {
  color: var(--ordo-accent);
  border-color: var(--ordo-accent);
  background: var(--ordo-accent-bg);
}

.show-in-flow-btn-lg.active {
  color: var(--ordo-success, #4ec969);
  border-color: var(--ordo-success, #4ec969);
  background: var(--ordo-success-bg, rgba(78, 201, 105, 0.15));
}

.output-json {
  margin: 0;
  padding: 12px;
  font-family: var(--ordo-font-mono);
  font-size: 12px;
  line-height: 1.5;
  color: var(--ordo-text-primary);
  background: var(--ordo-bg-editor);
  border: 1px solid var(--ordo-border-light);
  border-radius: var(--ordo-radius-sm);
  overflow-x: auto;
}

.error-display {
  padding: 12px;
  background: var(--ordo-error-bg, rgba(231, 76, 60, 0.1));
  border: 1px solid var(--ordo-error, #e74c3c);
  border-radius: var(--ordo-radius-sm);
}

.error-text {
  margin: 0;
  font-family: var(--ordo-font-mono);
  font-size: 12px;
  line-height: 1.5;
  color: var(--ordo-error, #e74c3c);
  white-space: pre-wrap;
  word-break: break-word;
}

.no-output {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  gap: 8px;
  color: var(--ordo-text-tertiary);
}

.no-output p {
  margin: 0;
  font-size: 12px;
}

/* Trace Tab */
.trace-tab {
  padding: 12px;
}

.trace-display {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.trace-path {
  font-size: 12px;
  color: var(--ordo-text-secondary);
  padding: 8px;
  background: var(--ordo-bg-editor);
  border-radius: var(--ordo-radius-sm);
}

.trace-path strong {
  color: var(--ordo-text-primary);
}

.trace-steps {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.trace-step {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  background: var(--ordo-bg-editor);
  border-radius: var(--ordo-radius-sm);
  font-size: 12px;
}

.step-index {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  font-size: 10px;
  font-weight: 600;
  color: var(--ordo-accent);
  background: var(--ordo-accent-bg);
  border-radius: 50%;
  flex-shrink: 0;
}

.step-id {
  font-family: var(--ordo-font-mono);
  font-size: 10px;
  color: var(--ordo-text-tertiary);
}

.step-name {
  flex: 1;
  color: var(--ordo-text-primary);
  font-weight: 500;
}

.step-duration {
  font-family: var(--ordo-font-mono);
  font-size: 10px;
  color: var(--ordo-text-tertiary);
}

.no-trace {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--ordo-text-tertiary);
  font-size: 12px;
}

/* History Tab */
.history-tab {
  display: flex;
  flex-direction: column;
}

.history-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--ordo-border-light);
  flex-shrink: 0;
}

.history-list {
  flex: 1;
  overflow-y: auto;
}

.history-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--ordo-border-light);
  cursor: pointer;
  transition: background 0.15s;
}

.history-item:hover {
  background: var(--ordo-bg-item-hover);
}

.history-time {
  font-family: var(--ordo-font-mono);
  font-size: 11px;
  color: var(--ordo-text-tertiary);
}

.history-code {
  font-family: var(--ordo-font-mono);
  font-size: 11px;
  font-weight: 600;
  padding: 2px 6px;
  border-radius: 3px;
}

.history-code.success {
  background: var(--ordo-success-bg, rgba(78, 201, 105, 0.15));
  color: var(--ordo-success, #4ec969);
}

.history-code.error {
  background: var(--ordo-error-bg, rgba(231, 76, 60, 0.15));
  color: var(--ordo-error, #e74c3c);
}

.history-duration {
  font-family: var(--ordo-font-mono);
  font-size: 10px;
  color: var(--ordo-text-tertiary);
}

.show-in-flow-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 0;
  margin-left: auto;
  color: var(--ordo-text-tertiary);
  background: transparent;
  border: 1px solid transparent;
  border-radius: var(--ordo-radius-sm);
  cursor: pointer;
  transition: all 0.15s;
}

.show-in-flow-btn:hover {
  color: var(--ordo-accent);
  border-color: var(--ordo-accent);
  background: var(--ordo-accent-bg);
}

.no-history {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--ordo-text-tertiary);
  font-size: 12px;
}
</style>
