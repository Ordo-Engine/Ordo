<script setup lang="ts">
/**
 * ExecutionAnnotation - Shows execution details as an annotation bubble on nodes
 * 执行注释气泡 - 在节点上显示执行详情
 */
import { computed } from 'vue';

export interface StepTraceInfo {
  /** Step ID */
  stepId: string;
  /** Step name */
  stepName: string;
  /** Duration in microseconds */
  durationUs: number;
  /** Branch taken (for decision nodes) */
  branchTaken?: string;
  /** Variables set (for action nodes) */
  variablesSet?: Record<string, any>;
  /** Result code (for terminal nodes) */
  resultCode?: string;
  /** Execution order (1-based) */
  order: number;
  /** Whether this step is the entry point */
  isEntry?: boolean;
  /** Whether this step is the terminal */
  isTerminal?: boolean;
}

export interface Props {
  /** Trace info for this node */
  trace: StepTraceInfo;
  /** Position offset from node */
  position?: 'top' | 'right' | 'bottom';
}

const props = withDefaults(defineProps<Props>(), {
  position: 'top',
});

const formattedDuration = computed(() => {
  const us = props.trace.durationUs;
  if (us < 1000) return `${us}µs`;
  if (us < 1000000) return `${(us / 1000).toFixed(1)}ms`;
  return `${(us / 1000000).toFixed(2)}s`;
});

const positionClass = computed(() => `annotation-${props.position}`);

const statusIcon = computed(() => {
  if (props.trace.isEntry) return '🚀';
  if (props.trace.isTerminal) return '🏁';
  if (props.trace.branchTaken) return '🔀';
  if (props.trace.variablesSet) return '⚙️';
  return '✓';
});
</script>

<template>
  <div class="execution-annotation" :class="positionClass">
    <div class="annotation-bubble">
      <!-- Order badge -->
      <span class="order-badge">{{ trace.order }}</span>

      <!-- Status icon -->
      <span class="status-icon">{{ statusIcon }}</span>

      <!-- Duration -->
      <span class="duration">{{ formattedDuration }}</span>

      <!-- Branch info for decision nodes -->
      <span v-if="trace.branchTaken" class="branch-info"> → {{ trace.branchTaken }} </span>

      <!-- Result code for terminal nodes -->
      <span
        v-if="trace.resultCode"
        class="result-code"
        :class="trace.resultCode.includes('ERROR') ? 'error' : 'success'"
      >
        {{ trace.resultCode }}
      </span>

      <!-- Variables tooltip -->
      <div
        v-if="trace.variablesSet && Object.keys(trace.variablesSet).length > 0"
        class="variables-tooltip"
      >
        <div v-for="(value, key) in trace.variablesSet" :key="key" class="variable-item">
          <span class="var-name">{{ key }}</span>
          <span class="var-value">{{ JSON.stringify(value) }}</span>
        </div>
      </div>
    </div>

    <!-- Connector line -->
    <div class="annotation-connector"></div>
  </div>
</template>

<style scoped>
.execution-annotation {
  position: absolute;
  z-index: 1000;
  pointer-events: auto;
}

.annotation-top {
  bottom: calc(100% + 8px);
  left: 50%;
  transform: translateX(-50%);
}

.annotation-right {
  left: calc(100% + 8px);
  top: 50%;
  transform: translateY(-50%);
}

.annotation-bottom {
  top: calc(100% + 8px);
  left: 50%;
  transform: translateX(-50%);
}

.annotation-bubble {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  background: var(--ordo-bg-elevated, #2a2d3e);
  border: 1px solid var(--ordo-accent, #6366f1);
  border-radius: 12px;
  font-size: 11px;
  color: var(--ordo-text-primary, #e4e4e7);
  white-space: nowrap;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
  animation: fadeIn 0.2s ease-out;
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(4px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.order-badge {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  background: var(--ordo-accent, #6366f1);
  color: white;
  font-size: 10px;
  font-weight: 600;
  border-radius: 50%;
}

.status-icon {
  font-size: 12px;
}

.duration {
  color: var(--ordo-text-secondary, #a1a1aa);
  font-family: var(--ordo-font-mono, monospace);
}

.branch-info {
  color: var(--ordo-accent-secondary, #8b5cf6);
  font-weight: 500;
}

.result-code {
  padding: 1px 6px;
  border-radius: 4px;
  font-weight: 600;
  font-size: 10px;
}

.result-code.success {
  background: var(--ordo-success-bg, rgba(78, 201, 105, 0.15));
  color: var(--ordo-success, #4ec969);
}

.result-code.error {
  background: var(--ordo-error-bg, rgba(231, 76, 60, 0.15));
  color: var(--ordo-error, #e74c3c);
}

.variables-tooltip {
  display: none;
  position: absolute;
  top: calc(100% + 4px);
  left: 50%;
  transform: translateX(-50%);
  padding: 8px 12px;
  background: var(--ordo-bg-elevated, #2a2d3e);
  border: 1px solid var(--ordo-border-color, #3f4257);
  border-radius: 6px;
  font-size: 10px;
  white-space: nowrap;
  z-index: 10;
}

.annotation-bubble:hover .variables-tooltip {
  display: block;
}

.variable-item {
  display: flex;
  gap: 8px;
  padding: 2px 0;
}

.var-name {
  color: var(--ordo-accent, #6366f1);
  font-weight: 500;
}

.var-value {
  color: var(--ordo-text-secondary, #a1a1aa);
  font-family: var(--ordo-font-mono, monospace);
}

.annotation-connector {
  position: absolute;
  background: var(--ordo-accent, #6366f1);
}

.annotation-top .annotation-connector {
  width: 2px;
  height: 8px;
  bottom: -8px;
  left: 50%;
  transform: translateX(-50%);
}

.annotation-right .annotation-connector {
  width: 8px;
  height: 2px;
  left: -8px;
  top: 50%;
  transform: translateY(-50%);
}

.annotation-bottom .annotation-connector {
  width: 2px;
  height: 8px;
  top: -8px;
  left: 50%;
  transform: translateX(-50%);
}
</style>
