<script setup lang="ts">
/**
 * OrdoExecutionModal - Rule execution debugging modal
 * 规则执行调试模态窗口
 */
import { ref, computed, watch } from 'vue';
import type { RuleSet } from '@ordo-engine/editor-core';
import {
  RuleExecutor,
  type ExecutionResult,
  type ExecutionOptions,
} from '@ordo-engine/editor-core';
import { useI18n } from '../../locale';

export interface Props {
  /** Whether the modal is open */
  open?: boolean;
  /** RuleSet to execute */
  ruleset: RuleSet;
}

const props = withDefaults(defineProps<Props>(), {
  open: false,
});

const emit = defineEmits<{
  'update:open': [value: boolean];
}>();

const { t } = useI18n();

// Execution state
const executor = new RuleExecutor();
const executing = ref(false);
const result = ref<ExecutionResult | null>(null);
const error = ref<string | null>(null);

// Input
const inputJson = ref('{\n  \n}');

// Execution options
const executionMode = ref<'wasm' | 'http'>('wasm');
const httpEndpoint = ref('http://localhost:8080');
const includeTrace = ref(true);

// Computed
const isOpen = computed({
  get: () => props.open,
  set: (value) => emit('update:open', value),
});

const resultClass = computed(() => {
  if (!result.value) return '';
  // You can customize this based on result code
  return result.value.code.includes('ERROR') || result.value.code.includes('FAIL')
    ? 'result-error'
    : 'result-success';
});

// Methods
function close() {
  isOpen.value = false;
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
    error.value = `${t('execution.parseError')}: ${
      e instanceof Error ? e.message : 'Unknown error'
    }`;
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
  } catch (e) {
    error.value = `${t('execution.executionError')}: ${
      e instanceof Error ? e.message : 'Unknown error'
    }`;
  } finally {
    executing.value = false;
  }
}

function formatOutput(output: any): string {
  return JSON.stringify(output, null, 2);
}

// Watch for modal open to reset state
watch(
  () => props.open,
  (newVal) => {
    if (newVal) {
      // Reset when opening
      result.value = null;
      error.value = null;
    }
  }
);
</script>

<template>
  <Teleport to="body">
    <Transition name="modal">
      <div v-if="isOpen" class="execution-modal-overlay" @click.self="close">
        <div class="execution-modal">
          <div class="modal-header">
            <h2>{{ t('execution.title') }}</h2>
            <button class="close-btn" @click="close" :title="t('common.close')">
              <svg
                width="16"
                height="16"
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

          <div class="modal-body">
            <!-- Input Editor -->
            <div class="section input-section">
              <label class="section-label">{{ t('execution.input') }}</label>
              <textarea
                v-model="inputJson"
                class="input-editor"
                :placeholder="t('execution.inputPlaceholder')"
                spellcheck="false"
              />
            </div>

            <!-- Execution Mode -->
            <div class="section mode-section">
              <label class="section-label">{{ t('execution.mode') }}</label>
              <div class="mode-controls">
                <select v-model="executionMode" class="mode-select">
                  <option value="wasm">{{ t('execution.modeWasm') }}</option>
                  <option value="http">{{ t('execution.modeHttp') }}</option>
                </select>

                <input
                  v-if="executionMode === 'http'"
                  v-model="httpEndpoint"
                  class="http-endpoint-input"
                  :placeholder="t('execution.httpEndpoint')"
                />
              </div>
            </div>

            <!-- Actions -->
            <div class="section actions-section">
              <button
                class="execute-btn"
                :class="{ executing }"
                :disabled="executing"
                @click="execute"
              >
                <svg
                  v-if="!executing"
                  width="14"
                  height="14"
                  viewBox="0 0 24 24"
                  fill="currentColor"
                >
                  <path d="M8 5v14l11-7z" />
                </svg>
                <svg v-else class="spinner" width="14" height="14" viewBox="0 0 24 24">
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

              <label class="trace-checkbox">
                <input type="checkbox" v-model="includeTrace" />
                <span>{{ t('execution.includeTrace') }}</span>
              </label>
            </div>

            <!-- Result Display -->
            <div v-if="result" class="section result-section">
              <h3 class="section-title">{{ t('execution.result') }}</h3>
              <div class="result-card" :class="resultClass">
                <div class="result-header">
                  <span class="code-badge">{{ result.code }}</span>
                  <span class="duration-badge">{{ result.duration_us }}µs</span>
                </div>
                <p v-if="result.message" class="result-message">{{ result.message }}</p>
                <div class="result-output">
                  <div class="output-label">{{ t('execution.output') }}:</div>
                  <pre class="output-content">{{ formatOutput(result.output) }}</pre>
                </div>
              </div>

              <!-- Trace Visualization -->
              <div v-if="result.trace" class="trace-section">
                <h4 class="trace-title">{{ t('execution.trace') }}</h4>
                <div class="trace-path">
                  <strong>{{ t('execution.path') }}:</strong> {{ result.trace.path }}
                </div>
                <div class="trace-steps">
                  <div
                    v-for="(step, index) in result.trace.steps"
                    :key="step.id"
                    class="trace-step"
                  >
                    <span class="step-index">{{ index + 1 }}</span>
                    <div class="step-info">
                      <div class="step-id-name">
                        <span class="step-id">{{ step.id }}</span>
                        <span class="step-name">{{ step.name }}</span>
                      </div>
                      <span class="step-duration">{{ step.duration_us }}µs</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>

            <!-- Error Display -->
            <div v-else-if="error" class="section error-section">
              <h3 class="section-title">{{ t('execution.error') }}</h3>
              <pre class="error-message">{{ error }}</pre>
            </div>

            <!-- No Result Placeholder -->
            <div v-else class="section no-result-section">
              <svg
                width="48"
                height="48"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
                opacity="0.3"
              >
                <circle cx="12" cy="12" r="10" />
                <path d="M8 12h8" />
                <path d="M12 8v8" />
              </svg>
              <p>{{ t('execution.noResult') }}</p>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
/* Modal Transition */
.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.2s ease;
}

.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}

.modal-enter-active .execution-modal,
.modal-leave-active .execution-modal {
  transition: transform 0.2s ease;
}

.modal-enter-from .execution-modal,
.modal-leave-to .execution-modal {
  transform: scale(0.95);
}

/* Modal Overlay */
.execution-modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
  backdrop-filter: blur(2px);
}

/* Modal Container */
.execution-modal {
  width: 90%;
  max-width: 900px;
  max-height: 90vh;
  background: var(--ordo-bg-panel);
  border-radius: var(--ordo-radius-lg, 12px);
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* Modal Header */
.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 20px;
  border-bottom: 1px solid var(--ordo-border-color);
  background: var(--ordo-bg-secondary, var(--ordo-bg-panel));
}

.modal-header h2 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.close-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
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

/* Modal Body */
.modal-body {
  flex: 1;
  overflow-y: auto;
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

/* Section */
.section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.section-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--ordo-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.section-title {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

/* Input Editor */
.input-editor {
  width: 100%;
  min-height: 120px;
  padding: 12px;
  font-family: var(--ordo-font-mono, 'Consolas', 'Monaco', monospace);
  font-size: 13px;
  line-height: 1.5;
  color: var(--ordo-text-primary);
  background: var(--ordo-bg-editor);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  resize: vertical;
}

.input-editor:focus {
  outline: none;
  border-color: var(--ordo-accent);
}

/* Mode Controls */
.mode-controls {
  display: flex;
  gap: 12px;
  align-items: center;
}

.mode-select,
.http-endpoint-input {
  padding: 8px 12px;
  font-size: 13px;
  color: var(--ordo-text-primary);
  background: var(--ordo-bg-editor);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
}

.mode-select {
  min-width: 180px;
}

.http-endpoint-input {
  flex: 1;
}

.mode-select:focus,
.http-endpoint-input:focus {
  outline: none;
  border-color: var(--ordo-accent);
}

/* Actions */
.actions-section {
  flex-direction: row;
  align-items: center;
}

.execute-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 20px;
  font-size: 14px;
  font-weight: 600;
  color: #fff;
  background: var(--ordo-accent);
  border: none;
  border-radius: var(--ordo-radius-sm);
  cursor: pointer;
  transition: all 0.15s;
}

.execute-btn:hover:not(:disabled) {
  opacity: 0.9;
  transform: translateY(-1px);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
}

.execute-btn:active:not(:disabled) {
  transform: translateY(0);
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

.trace-checkbox {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  color: var(--ordo-text-secondary);
  cursor: pointer;
  user-select: none;
}

.trace-checkbox input {
  cursor: pointer;
}

/* Result Card */
.result-card {
  padding: 16px;
  background: var(--ordo-bg-editor);
  border: 1px solid var(--ordo-border-color);
  border-left: 4px solid var(--ordo-success, #4ec969);
  border-radius: var(--ordo-radius-sm);
}

.result-card.result-error {
  border-left-color: var(--ordo-error, #e74c3c);
}

.result-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}

.code-badge {
  font-family: var(--ordo-font-mono);
  font-size: 13px;
  font-weight: 700;
  padding: 4px 10px;
  background: var(--ordo-success-bg, rgba(78, 201, 105, 0.15));
  color: var(--ordo-success, #4ec969);
  border-radius: 4px;
}

.result-error .code-badge {
  background: var(--ordo-error-bg, rgba(231, 76, 60, 0.15));
  color: var(--ordo-error, #e74c3c);
}

.duration-badge {
  font-family: var(--ordo-font-mono);
  font-size: 11px;
  color: var(--ordo-text-tertiary);
  padding: 2px 8px;
  background: var(--ordo-bg-item);
  border-radius: 3px;
}

.result-message {
  margin: 0 0 12px 0;
  font-size: 13px;
  color: var(--ordo-text-secondary);
}

.result-output {
  margin-top: 12px;
}

.output-label {
  font-size: 11px;
  font-weight: 600;
  color: var(--ordo-text-tertiary);
  text-transform: uppercase;
  margin-bottom: 6px;
}

.output-content {
  margin: 0;
  padding: 12px;
  font-family: var(--ordo-font-mono);
  font-size: 12px;
  line-height: 1.5;
  color: var(--ordo-text-primary);
  background: var(--ordo-bg-panel);
  border: 1px solid var(--ordo-border-light);
  border-radius: 4px;
  overflow-x: auto;
}

/* Trace Section */
.trace-section {
  margin-top: 16px;
  padding: 16px;
  background: var(--ordo-bg-editor);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
}

.trace-title {
  margin: 0 0 12px 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.trace-path {
  font-size: 12px;
  color: var(--ordo-text-secondary);
  margin-bottom: 12px;
  padding: 8px;
  background: var(--ordo-bg-panel);
  border-radius: 4px;
}

.trace-path strong {
  color: var(--ordo-text-primary);
}

.trace-steps {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.trace-step {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px;
  background: var(--ordo-bg-panel);
  border-radius: 4px;
}

.step-index {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  font-size: 11px;
  font-weight: 600;
  color: var(--ordo-accent);
  background: var(--ordo-accent-bg);
  border-radius: 50%;
  flex-shrink: 0;
}

.step-info {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.step-id-name {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
}

.step-id {
  font-family: var(--ordo-font-mono);
  font-size: 11px;
  color: var(--ordo-text-tertiary);
}

.step-name {
  font-size: 13px;
  color: var(--ordo-text-primary);
  font-weight: 500;
}

.step-duration {
  font-family: var(--ordo-font-mono);
  font-size: 11px;
  color: var(--ordo-text-tertiary);
  padding: 2px 6px;
  background: var(--ordo-bg-editor);
  border-radius: 3px;
}

/* Error Section */
.error-section {
  padding: 16px;
  background: var(--ordo-error-bg, rgba(231, 76, 60, 0.1));
  border: 1px solid var(--ordo-error, #e74c3c);
  border-radius: var(--ordo-radius-sm);
}

.error-message {
  margin: 0;
  padding: 12px;
  font-family: var(--ordo-font-mono);
  font-size: 12px;
  line-height: 1.5;
  color: var(--ordo-error, #e74c3c);
  background: var(--ordo-bg-panel);
  border-radius: 4px;
  white-space: pre-wrap;
  word-break: break-word;
}

/* No Result Placeholder */
.no-result-section {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 48px 24px;
  color: var(--ordo-text-tertiary);
  text-align: center;
}

.no-result-section p {
  margin: 0;
  font-size: 13px;
  max-width: 300px;
}
</style>
