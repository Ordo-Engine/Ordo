<script setup lang="ts">
/**
 * OrdoPerformancePanel - JIT Performance Comparison Panel
 * JIT 性能对比面板
 *
 * Compares execution performance between:
 * - Tree-walk interpreter (WASM)
 * - HTTP backend (VM)
 * - JIT-compiled execution
 */
import { ref, computed, watch } from 'vue';
import type { RuleSet, JITSchema, JITRulesetAnalysis } from '@ordo-engine/editor-core';

export interface PerformanceMetric {
  mode: 'wasm' | 'http' | 'jit';
  label: string;
  duration_us: number;
  success: boolean;
  error?: string;
}

export interface Props {
  /** RuleSet to benchmark */
  ruleset?: RuleSet | null;
  /** Input data for execution */
  input?: Record<string, any>;
  /** JIT Schema (optional) */
  schema?: JITSchema | null;
  /** HTTP endpoint for server modes */
  httpEndpoint?: string;
  /** JIT analysis result */
  jitAnalysis?: JITRulesetAnalysis | null;
}

const props = withDefaults(defineProps<Props>(), {
  ruleset: null,
  input: () => ({}),
  schema: null,
  httpEndpoint: 'http://localhost:8080',
  jitAnalysis: null,
});

const emit = defineEmits<{
  'run-benchmark': [];
  'result': [metrics: PerformanceMetric[]];
}>();

// State
const isRunning = ref(false);
const metrics = ref<PerformanceMetric[]>([]);
const iterations = ref(1);
const showDetails = ref(false);

// Computed: Best performing mode
const bestMode = computed(() => {
  if (metrics.value.length === 0) return null;
  const successful = metrics.value.filter((m) => m.success);
  if (successful.length === 0) return null;
  return successful.reduce((a, b) => (a.duration_us < b.duration_us ? a : b));
});

// Computed: Speedup factors relative to WASM
const speedupFactors = computed(() => {
  const wasmMetric = metrics.value.find((m) => m.mode === 'wasm' && m.success);
  if (!wasmMetric) return {};

  const factors: Record<string, number> = {};
  for (const metric of metrics.value) {
    if (metric.success && metric.duration_us > 0) {
      factors[metric.mode] = wasmMetric.duration_us / metric.duration_us;
    }
  }
  return factors;
});

// Computed: Maximum duration for bar scaling
const maxDuration = computed(() => {
  const durations = metrics.value.filter((m) => m.success).map((m) => m.duration_us);
  return durations.length > 0 ? Math.max(...durations) : 1;
});

// Run benchmark
async function runBenchmark() {
  if (!props.ruleset) return;

  isRunning.value = true;
  metrics.value = [];

  try {
    emit('run-benchmark');

    // Test each mode
    const modes: Array<{ mode: 'wasm' | 'http' | 'jit'; label: string }> = [
      { mode: 'wasm', label: 'WASM (Tree-walk)' },
      { mode: 'http', label: 'HTTP (VM)' },
    ];

    // Only add JIT mode if analysis shows it's compatible
    if (props.jitAnalysis?.overallCompatible) {
      modes.push({ mode: 'jit', label: 'HTTP (JIT)' });
    }

    for (const { mode, label } of modes) {
      const metric = await benchmarkMode(mode, label);
      metrics.value.push(metric);
    }

    emit('result', metrics.value);
  } finally {
    isRunning.value = false;
  }
}

async function benchmarkMode(
  mode: 'wasm' | 'http' | 'jit',
  label: string
): Promise<PerformanceMetric> {
  try {
    const { RuleExecutor } = await import('@ordo-engine/editor-core');
    const executor = new RuleExecutor();

    const startTime = performance.now();

    for (let i = 0; i < iterations.value; i++) {
      await executor.execute(props.ruleset!, props.input, {
        mode,
        httpEndpoint: props.httpEndpoint,
        includeTrace: false,
        jitSchema: mode === 'jit' ? (props.schema ?? undefined) : undefined,
      });
    }

    const endTime = performance.now();
    const totalMs = endTime - startTime;
    const avgUs = (totalMs / iterations.value) * 1000;

    return {
      mode,
      label,
      duration_us: Math.round(avgUs),
      success: true,
    };
  } catch (error) {
    return {
      mode,
      label,
      duration_us: 0,
      success: false,
      error: error instanceof Error ? error.message : 'Unknown error',
    };
  }
}

// Format duration
function formatDuration(us: number): string {
  if (us < 1000) return `${us}µs`;
  if (us < 1000000) return `${(us / 1000).toFixed(2)}ms`;
  return `${(us / 1000000).toFixed(2)}s`;
}

// Get bar width percentage
function getBarWidth(us: number): number {
  if (maxDuration.value === 0) return 0;
  return (us / maxDuration.value) * 100;
}

// Get mode color
function getModeColor(mode: string): string {
  switch (mode) {
    case 'wasm':
      return '#3b82f6'; // Blue
    case 'http':
      return '#22c55e'; // Green
    case 'jit':
      return '#f59e0b'; // Amber
    default:
      return '#6b7280'; // Gray
  }
}
</script>

<template>
  <div class="ordo-performance-panel">
    <!-- Header -->
    <div class="panel-header">
      <div class="header-title">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" class="header-icon">
          <path d="M13 2L3 14h8l-1 8 10-12h-8l1-8z" />
        </svg>
        <span>Performance Comparison</span>
      </div>
      <div class="header-controls">
        <label class="iterations-label">
          Iterations:
          <input
            v-model.number="iterations"
            type="number"
            min="1"
            max="100"
            class="iterations-input"
          />
        </label>
        <button
          class="run-btn"
          :disabled="!ruleset || isRunning"
          @click="runBenchmark"
        >
          <span v-if="isRunning" class="spinner"></span>
          <span v-else>Run Benchmark</span>
        </button>
      </div>
    </div>

    <!-- JIT Compatibility Notice -->
    <div v-if="jitAnalysis && !jitAnalysis.overallCompatible" class="jit-notice warning">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="12" cy="12" r="10"></circle>
        <line x1="12" y1="8" x2="12" y2="12"></line>
        <line x1="12" y1="16" x2="12.01" y2="16"></line>
      </svg>
      <span>
        JIT mode not available: {{ jitAnalysis.incompatibleCount }} of
        {{ jitAnalysis.totalExpressions }} expressions are not JIT-compatible
      </span>
    </div>

    <div v-else-if="jitAnalysis?.overallCompatible" class="jit-notice success">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
        <path d="M13 2L3 14h8l-1 8 10-12h-8l1-8z" />
      </svg>
      <span>
        All {{ jitAnalysis.totalExpressions }} expressions are JIT-compatible
        (Est. {{ jitAnalysis.estimatedSpeedup?.toFixed(0) || '20' }}x speedup)
      </span>
    </div>

    <!-- Results -->
    <div v-if="metrics.length > 0" class="results">
      <div
        v-for="metric in metrics"
        :key="metric.mode"
        class="result-item"
        :class="{ best: bestMode?.mode === metric.mode, error: !metric.success }"
      >
        <div class="result-header">
          <span class="result-label">{{ metric.label }}</span>
          <span v-if="metric.success" class="result-duration">
            {{ formatDuration(metric.duration_us) }}
          </span>
          <span v-else class="result-error">Failed</span>
          <span v-if="speedupFactors[metric.mode] && metric.mode !== 'wasm'" class="result-speedup">
            {{ speedupFactors[metric.mode].toFixed(1) }}x
          </span>
          <span v-if="bestMode?.mode === metric.mode" class="best-badge">Fastest</span>
        </div>

        <div v-if="metric.success" class="result-bar-container">
          <div
            class="result-bar"
            :style="{
              width: `${getBarWidth(metric.duration_us)}%`,
              backgroundColor: getModeColor(metric.mode),
            }"
          ></div>
        </div>

        <div v-if="!metric.success && metric.error" class="error-message">
          {{ metric.error }}
        </div>
      </div>
    </div>

    <!-- Empty State -->
    <div v-else class="empty-state">
      <p>Click "Run Benchmark" to compare execution modes</p>
    </div>

    <!-- Details Toggle -->
    <div v-if="metrics.length > 0" class="details-section">
      <button class="details-toggle" @click="showDetails = !showDetails">
        {{ showDetails ? 'Hide Details' : 'Show Details' }}
      </button>

      <div v-if="showDetails" class="details-content">
        <table class="details-table">
          <thead>
            <tr>
              <th>Mode</th>
              <th>Duration</th>
              <th>Speedup vs WASM</th>
              <th>Status</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="metric in metrics" :key="metric.mode">
              <td>{{ metric.label }}</td>
              <td>{{ metric.success ? formatDuration(metric.duration_us) : '-' }}</td>
              <td>{{ speedupFactors[metric.mode]?.toFixed(2) || '1.00' }}x</td>
              <td>
                <span :class="['status-badge', metric.success ? 'success' : 'error']">
                  {{ metric.success ? 'OK' : 'Error' }}
                </span>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>

<style scoped>
.ordo-performance-panel {
  background: var(--ordo-bg-panel);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  overflow: hidden;
}

.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  background: var(--ordo-bg-item);
  border-bottom: 1px solid var(--ordo-border-color);
}

.header-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-weight: 600;
  font-size: 13px;
}

.header-icon {
  color: #f59e0b;
}

.header-controls {
  display: flex;
  align-items: center;
  gap: 12px;
}

.iterations-label {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.iterations-input {
  width: 60px;
  padding: 4px 8px;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  background: var(--ordo-bg-input);
  color: var(--ordo-text-primary);
  font-size: 12px;
}

.run-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 14px;
  background: #f59e0b;
  color: #000;
  border: none;
  border-radius: var(--ordo-radius-sm);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s;
}

.run-btn:hover:not(:disabled) {
  background: #d97706;
}

.run-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.spinner {
  width: 12px;
  height: 12px;
  border: 2px solid rgba(0, 0, 0, 0.2);
  border-top-color: #000;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.jit-notice {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 16px;
  font-size: 12px;
  border-bottom: 1px solid var(--ordo-border-color);
}

.jit-notice.warning {
  background: rgba(245, 158, 11, 0.1);
  color: #d97706;
}

.jit-notice.success {
  background: rgba(34, 197, 94, 0.1);
  color: #16a34a;
}

.results {
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.result-item {
  padding: 12px;
  background: var(--ordo-bg-item);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
}

.result-item.best {
  border-color: #f59e0b;
  background: rgba(245, 158, 11, 0.05);
}

.result-item.error {
  border-color: var(--ordo-error);
  opacity: 0.7;
}

.result-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 8px;
}

.result-label {
  font-weight: 500;
  font-size: 13px;
}

.result-duration {
  font-family: var(--ordo-font-mono);
  font-size: 14px;
  font-weight: 600;
}

.result-error {
  color: var(--ordo-error);
  font-size: 12px;
}

.result-speedup {
  margin-left: auto;
  font-size: 12px;
  padding: 2px 8px;
  background: var(--ordo-bg-tertiary);
  border-radius: 10px;
  color: var(--ordo-text-secondary);
}

.best-badge {
  font-size: 10px;
  padding: 2px 8px;
  background: #f59e0b;
  color: #000;
  border-radius: 10px;
  font-weight: 600;
}

.result-bar-container {
  height: 8px;
  background: var(--ordo-bg-tertiary);
  border-radius: 4px;
  overflow: hidden;
}

.result-bar {
  height: 100%;
  border-radius: 4px;
  transition: width 0.3s ease;
}

.error-message {
  margin-top: 8px;
  font-size: 11px;
  color: var(--ordo-error);
}

.empty-state {
  padding: 32px;
  text-align: center;
  color: var(--ordo-text-tertiary);
  font-size: 13px;
}

.details-section {
  padding: 12px 16px;
  border-top: 1px solid var(--ordo-border-color);
}

.details-toggle {
  background: none;
  border: none;
  color: var(--ordo-primary-500);
  font-size: 12px;
  cursor: pointer;
  padding: 0;
}

.details-toggle:hover {
  text-decoration: underline;
}

.details-content {
  margin-top: 12px;
}

.details-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
}

.details-table th,
.details-table td {
  padding: 8px 12px;
  text-align: left;
  border-bottom: 1px solid var(--ordo-border-light);
}

.details-table th {
  font-weight: 600;
  color: var(--ordo-text-secondary);
  background: var(--ordo-bg-tertiary);
}

.status-badge {
  padding: 2px 6px;
  border-radius: var(--ordo-radius-xs);
  font-size: 10px;
  font-weight: 600;
}

.status-badge.success {
  background: rgba(34, 197, 94, 0.2);
  color: #16a34a;
}

.status-badge.error {
  background: rgba(239, 68, 68, 0.2);
  color: #ef4444;
}
</style>
