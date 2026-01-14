<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import OrdoASTTree from './OrdoASTTree.vue';
import type { ASTNode, BytecodeInfo, EvalStep, DebugEvalResponse } from './types';

const props = defineProps<{
  endpoint?: string;
  expression?: string;
  context?: Record<string, unknown>;
  response?: DebugEvalResponse;
}>();

const emit = defineEmits<{
  evaluated: [response: DebugEvalResponse];
  error: [message: string];
}>();

// State
const inputExpression = ref(props.expression ?? '');
const inputContext = ref(JSON.stringify(props.context ?? {}, null, 2));
const isLoading = ref(false);
const result = ref<DebugEvalResponse | null>(props.response ?? null);
const activeTab = ref<'ast' | 'bytecode' | 'steps'>('ast');

// Computed
const hasResult = computed(() => result.value !== null);
const ast = computed(() => result.value?.ast);
const bytecode = computed(() => result.value?.bytecode);
const evalSteps = computed(() => result.value?.eval_steps ?? []);

// Methods
async function evaluate() {
  if (!props.endpoint || !inputExpression.value) return;

  isLoading.value = true;

  try {
    let context = {};
    try {
      context = JSON.parse(inputContext.value || '{}');
    } catch {
      emit('error', 'Invalid JSON context');
      return;
    }

    const response = await fetch(`${props.endpoint}/api/v1/debug/eval`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        expression: inputExpression.value,
        context,
        trace_level: 'full',
      }),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(error.message || `HTTP ${response.status}`);
    }

    result.value = await response.json();
    emit('evaluated', result.value!);
  } catch (e) {
    emit('error', e instanceof Error ? e.message : 'Evaluation failed');
  } finally {
    isLoading.value = false;
  }
}

function formatDuration(ns: number): string {
  if (ns < 1000) return `${ns}ns`;
  if (ns < 1000000) return `${(ns / 1000).toFixed(2)}µs`;
  return `${(ns / 1000000).toFixed(2)}ms`;
}

function formatValue(value: unknown): string {
  if (value === null) return 'null';
  if (value === undefined) return 'undefined';
  if (typeof value === 'string') return `"${value}"`;
  return JSON.stringify(value);
}

// Watch for external changes
watch(
  () => props.expression,
  (val) => {
    if (val) inputExpression.value = val;
  }
);

watch(
  () => props.response,
  (val) => {
    if (val) result.value = val;
  }
);
</script>

<template>
  <div class="expr-visualizer">
    <!-- Input Section -->
    <div class="input-section">
      <div class="input-group">
        <label>Expression</label>
        <input
          v-model="inputExpression"
          type="text"
          placeholder="age > 18 && status == 'active'"
          @keyup.enter="evaluate"
        />
      </div>

      <div class="input-group">
        <label>Context (JSON)</label>
        <textarea v-model="inputContext" placeholder='{"age": 25, "status": "active"}' rows="3" />
      </div>

      <button class="eval-btn" :disabled="isLoading || !endpoint" @click="evaluate">
        {{ isLoading ? 'Evaluating...' : 'Evaluate' }}
      </button>
    </div>

    <!-- Result Section -->
    <div v-if="hasResult" class="result-section">
      <!-- Result Header -->
      <div class="result-header">
        <div class="result-value">
          <span class="label">Result:</span>
          <span class="value">{{ formatValue(result?.result) }}</span>
        </div>
        <div class="timing">
          <span>Parse: {{ formatDuration(result?.parse_duration_ns ?? 0) }}</span>
          <span v-if="result?.compile_duration_ns"
            >Compile: {{ formatDuration(result.compile_duration_ns) }}</span
          >
          <span>Eval: {{ formatDuration(result?.eval_duration_ns ?? 0) }}</span>
        </div>
      </div>

      <!-- Tabs -->
      <div class="tabs">
        <button class="tab" :class="{ active: activeTab === 'ast' }" @click="activeTab = 'ast'">
          AST
        </button>
        <button
          class="tab"
          :class="{ active: activeTab === 'bytecode' }"
          @click="activeTab = 'bytecode'"
        >
          Bytecode
        </button>
        <button class="tab" :class="{ active: activeTab === 'steps' }" @click="activeTab = 'steps'">
          Steps ({{ evalSteps.length }})
        </button>
      </div>

      <!-- Tab Content -->
      <div class="tab-content">
        <!-- AST Tab -->
        <div v-if="activeTab === 'ast' && ast" class="ast-panel">
          <OrdoASTTree :node="ast" />
        </div>

        <!-- Bytecode Tab -->
        <div v-if="activeTab === 'bytecode' && bytecode" class="bytecode-panel">
          <div class="bytecode-stats">
            <span>{{ bytecode.instruction_count }} instructions</span>
            <span>{{ bytecode.constant_count }} constants</span>
            <span>{{ bytecode.field_count }} fields</span>
            <span>{{ bytecode.function_count }} functions</span>
          </div>
          <div class="bytecode-list">
            <div v-for="(inst, idx) in bytecode.instructions" :key="idx" class="bytecode-item">
              {{ inst }}
            </div>
          </div>
        </div>

        <!-- Steps Tab -->
        <div v-if="activeTab === 'steps'" class="steps-panel">
          <div v-for="step in evalSteps" :key="step.step" class="step-item">
            <span class="step-num">{{ step.step }}</span>
            <span class="step-desc">{{ step.description }}</span>
            <span class="step-result">→ {{ formatValue(step.result) }}</span>
          </div>
          <div v-if="evalSteps.length === 0" class="empty-state">No evaluation steps recorded</div>
        </div>
      </div>
    </div>

    <!-- Empty State -->
    <div v-else class="empty-state">
      Enter an expression and click Evaluate to see the visualization
    </div>
  </div>
</template>

<style scoped>
.expr-visualizer {
  font-family: 'JetBrains Mono', 'Fira Code', monospace;
  font-size: 12px;
  background: #1e1e2e;
  border-radius: 8px;
  padding: 16px;
  color: #cdd6f4;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.input-section {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.input-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.input-group label {
  font-size: 11px;
  color: #a6adc8;
  font-weight: 500;
}

.input-group input,
.input-group textarea {
  padding: 8px 12px;
  background: #313244;
  border: 1px solid #45475a;
  border-radius: 6px;
  color: #cdd6f4;
  font-family: inherit;
  font-size: 12px;
  resize: vertical;
}

.input-group input:focus,
.input-group textarea:focus {
  outline: none;
  border-color: #89b4fa;
}

.eval-btn {
  padding: 10px 20px;
  background: #89b4fa;
  border: none;
  border-radius: 6px;
  color: #1e1e2e;
  font-family: inherit;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.eval-btn:hover:not(:disabled) {
  background: #b4befe;
}

.eval-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.result-section {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.result-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px;
  background: #313244;
  border-radius: 6px;
}

.result-value {
  display: flex;
  gap: 8px;
  align-items: center;
}

.result-value .label {
  color: #a6adc8;
}

.result-value .value {
  font-weight: 600;
  color: #a6e3a1;
}

.timing {
  display: flex;
  gap: 12px;
  font-size: 10px;
  color: #6c7086;
}

.tabs {
  display: flex;
  gap: 4px;
  border-bottom: 1px solid #45475a;
  padding-bottom: 8px;
}

.tab {
  padding: 6px 12px;
  background: transparent;
  border: none;
  border-radius: 4px;
  color: #a6adc8;
  font-family: inherit;
  font-size: 11px;
  cursor: pointer;
  transition: all 0.15s;
}

.tab:hover {
  background: #313244;
}

.tab.active {
  background: #89b4fa;
  color: #1e1e2e;
  font-weight: 600;
}

.tab-content {
  min-height: 200px;
  max-height: 400px;
  overflow-y: auto;
}

.ast-panel {
  padding: 8px;
}

.bytecode-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.bytecode-stats {
  display: flex;
  gap: 16px;
  font-size: 10px;
  color: #a6adc8;
  padding: 8px 12px;
  background: #313244;
  border-radius: 4px;
}

.bytecode-list {
  display: flex;
  flex-direction: column;
}

.bytecode-item {
  padding: 4px 12px;
  font-size: 11px;
  border-left: 2px solid transparent;
}

.bytecode-item:hover {
  background: #313244;
  border-left-color: #89b4fa;
}

.steps-panel {
  display: flex;
  flex-direction: column;
}

.step-item {
  display: grid;
  grid-template-columns: 32px 1fr auto;
  gap: 12px;
  padding: 8px 12px;
  border-bottom: 1px solid #313244;
}

.step-num {
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #45475a;
  border-radius: 50%;
  font-size: 10px;
  font-weight: 600;
}

.step-desc {
  color: #cdd6f4;
}

.step-result {
  color: #a6e3a1;
  font-weight: 500;
}

.empty-state {
  padding: 32px;
  text-align: center;
  color: #6c7086;
}

/* Scrollbar */
.tab-content::-webkit-scrollbar {
  width: 6px;
}

.tab-content::-webkit-scrollbar-track {
  background: #1e1e2e;
}

.tab-content::-webkit-scrollbar-thumb {
  background: #45475a;
  border-radius: 3px;
}
</style>
