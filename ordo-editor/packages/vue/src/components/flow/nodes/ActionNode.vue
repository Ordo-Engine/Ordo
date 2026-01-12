<script setup lang="ts">
/**
 * Action Node - Flow editor action step node (Blueprint style)
 * 动作节点 - 流程编辑器动作步骤节点（蓝图风格）
 */
import { computed } from 'vue';
import { Handle, Position } from '@vue-flow/core';
import type { ActionStep, VariableAssignment } from '@ordo/editor-core';
import { exprToString } from '@ordo/editor-core';
import OrdoIcon from '../../icons/OrdoIcon.vue';
import { PIN_COLORS } from '../types';
import { useI18n } from '../../../locale';
import type { StepTraceInfo } from './ExecutionAnnotation.vue';
import ExecutionAnnotation from './ExecutionAnnotation.vue';

export interface Props {
  data: {
    step: ActionStep;
    isStart: boolean;
    label: string;
    executionAnnotation?: StepTraceInfo | null;
  };
  selected?: boolean;
}

const props = defineProps<Props>();

const { t } = useI18n();

/** Node display title (reactive to i18n changes if name is missing) */
const displayTitle = computed(() => {
  return props.data.label || props.data.step.name || t('step.action');
});

const assignments = computed<VariableAssignment[]>(() => props.data.step.assignments || []);
const hasLogging = computed(() => !!props.data.step.logging);
const hasExternalCall = computed(() => !!props.data.step.externalCall);

/** Format assignment value for display */
function formatValue(assignment: VariableAssignment): string {
  if (assignment.value !== undefined) {
    const val = assignment.value;
    if (typeof val === 'string') return `"${val}"`;
    if (typeof val === 'object') return JSON.stringify(val);
    return String(val);
  }
  if (assignment.expression) {
    const exprStr = exprToString(assignment.expression);
    return exprStr.length > 15 ? exprStr.slice(0, 15) + '...' : exprStr;
  }
  return '?';
}
</script>

<template>
  <div class="flow-node action-node" :class="{ selected, 'is-start': data.isStart }">
    <!-- Execution Annotation -->
    <ExecutionAnnotation
      v-if="data.executionAnnotation"
      :trace="data.executionAnnotation"
      position="top"
    />

    <!-- Node Header -->
    <div class="node-header">
      <!-- Exec Input Pin (Left) -->
      <Handle type="target" :position="Position.Left" class="pin pin-exec pin-input" id="input">
        <svg class="pin-shape" width="10" height="10" viewBox="0 0 10 10">
          <polygon points="0,0 10,5 0,10" :fill="PIN_COLORS.execInput" class="pin-fill" />
        </svg>
      </Handle>

      <span class="node-badge start" v-if="data.isStart">{{ t('step.start') }}</span>
      <OrdoIcon name="action" :size="14" class="node-icon" />
      <span class="node-title">{{ displayTitle }}</span>
      <span class="node-type-badge">{{ t('step.typeAction') }}</span>
    </div>

    <!-- Variable Assignments Section -->
    <div class="node-section vars-section" v-if="assignments.length > 0">
      <div
        v-for="assign in assignments"
        :key="assign.variable"
        class="var-row"
        :title="`${assign.variable} = ${formatValue(assign)}`"
      >
        <!-- Data Input (optional, for expression dependencies) -->
        <Handle
          type="target"
          :position="Position.Left"
          class="pin pin-data pin-input"
          :id="`data-in-${assign.variable}`"
        >
          <svg class="pin-shape" width="8" height="8" viewBox="0 0 8 8">
            <circle cx="4" cy="4" r="3.5" :fill="PIN_COLORS.dataPin" class="pin-fill" />
          </svg>
        </Handle>

        <span class="var-name">{{ assign.variable }}</span>
        <span class="var-op">=</span>
        <span class="var-value">{{ formatValue(assign) }}</span>

        <!-- Data Output -->
        <Handle
          type="source"
          :position="Position.Right"
          class="pin pin-data pin-output"
          :id="`data-out-${assign.variable}`"
        >
          <svg class="pin-shape" width="8" height="8" viewBox="0 0 8 8">
            <circle cx="4" cy="4" r="3.5" :fill="PIN_COLORS.dataPin" class="pin-fill" />
          </svg>
        </Handle>
      </div>
    </div>

    <!-- Info chips (logging, external call) -->
    <div class="node-section info-section" v-if="hasLogging || hasExternalCall">
      <div class="info-row">
        <span class="info-chip" v-if="hasLogging"> <OrdoIcon name="check" :size="10" /> log </span>
        <span class="info-chip external" v-if="hasExternalCall">
          <OrdoIcon name="action" :size="10" /> call
        </span>
      </div>
    </div>

    <!-- Exec Output Section -->
    <div class="node-section exec-section">
      <div class="exec-row">
        <span class="exec-label">{{ t('step.next') }}</span>
        <Handle
          type="source"
          :position="Position.Right"
          class="pin pin-exec pin-output pin-default"
          id="output"
        >
          <svg class="pin-shape" width="10" height="10" viewBox="0 0 10 10">
            <polygon points="0,0 10,5 0,10" :fill="PIN_COLORS.execDefault" class="pin-fill" />
          </svg>
        </Handle>
      </div>
    </div>
  </div>
</template>

<style scoped>
.flow-node {
  background: var(--ordo-bg-item, #1e1e1e);
  border: 1px solid var(--ordo-border-color, #3c3c3c);
  border-radius: 4px;
  min-width: 180px;
  max-width: 280px;
  font-family: var(--ordo-font-sans);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
  transition:
    box-shadow 0.15s,
    border-color 0.15s;
  position: relative;
}

.flow-node:hover {
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
  border-color: var(--ordo-text-tertiary, #6c6c6c);
}

.flow-node.selected {
  border-color: var(--ordo-node-action, #0066b8);
  box-shadow: 0 0 0 2px rgba(0, 102, 184, 0.3);
}

.flow-node.is-start {
  border-color: var(--ordo-node-action, #0066b8);
}

/* Action specific - top border accent */
.action-node {
  border-top: 3px solid var(--ordo-node-action, #0066b8);
}

/* Header */
.node-header {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 12px 8px 20px;
  background: rgba(0, 102, 184, 0.1);
  border-bottom: 1px solid var(--ordo-border-light, #2d2d2d);
  position: relative;
}

.node-badge.start {
  font-size: 8px;
  font-weight: 700;
  color: #fff;
  background: var(--ordo-node-action, #0066b8);
  padding: 2px 4px;
  border-radius: 2px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.node-icon {
  color: var(--ordo-node-action, #0066b8);
  flex-shrink: 0;
}

.node-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--ordo-text-primary, #e0e0e0);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 1;
}

.node-type-badge {
  font-size: 9px;
  color: var(--ordo-text-tertiary, #888);
  background: var(--ordo-bg-panel, #252525);
  padding: 2px 5px;
  border-radius: 2px;
  flex-shrink: 0;
}

/* Sections */
.node-section {
  padding: 6px 0;
  border-bottom: 1px solid var(--ordo-border-light, #2d2d2d);
}

.node-section:last-child {
  border-bottom: none;
}

/* Variable assignment rows */
.vars-section {
  background: rgba(74, 158, 255, 0.05);
}

.var-row {
  display: flex;
  align-items: center;
  padding: 4px 20px;
  position: relative;
  min-height: 24px;
  gap: 4px;
}

.var-name {
  font-size: 11px;
  font-weight: 500;
  color: var(--ordo-text-secondary, #b0b0b0);
  font-family: var(--ordo-font-mono, monospace);
}

.var-op {
  font-size: 11px;
  color: var(--ordo-text-tertiary, #888);
}

.var-value {
  font-size: 10px;
  color: var(--ordo-text-tertiary, #888);
  font-family: var(--ordo-font-mono, monospace);
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* Info section */
.info-section {
  padding: 4px 12px;
}

.info-row {
  display: flex;
  align-items: center;
  gap: 6px;
}

.info-chip {
  font-size: 10px;
  color: var(--ordo-text-tertiary, #888);
  background: var(--ordo-bg-panel, #252525);
  padding: 2px 6px;
  border-radius: 2px;
  display: flex;
  align-items: center;
  gap: 3px;
}

.info-chip.external {
  color: var(--ordo-node-action, #0066b8);
}

/* Exec output section */
.exec-section {
  padding: 4px 0;
}

.exec-row {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  padding: 4px 12px;
  position: relative;
  min-height: 24px;
}

.exec-label {
  font-size: 10px;
  color: var(--ordo-text-tertiary, #888);
  padding-right: 16px;
}

/* Pin styles - Blueprint style triangles and circles */
.pin {
  /* Reset Vue Flow handle defaults */
  width: auto !important;
  height: auto !important;
  min-width: 0 !important;
  min-height: 0 !important;
  background: transparent !important;
  border: none !important;
  border-radius: 0 !important;

  display: flex;
  align-items: center;
  justify-content: center;
  cursor: crosshair;
}

.pin-shape {
  display: block;
  pointer-events: none;
}

.pin-fill {
  transition:
    filter 0.15s ease,
    fill 0.15s ease;
}

/* Hover effects - glow only, no size change */
.pin:hover .pin-fill {
  filter: drop-shadow(0 0 4px currentColor) brightness(1.2);
}

/* Input pin positioning (in header) */
.node-header .pin-input {
  position: absolute;
  left: -5px;
  top: 50%;
  transform: translateY(-50%);
}

/* Output pin positioning (in rows) */
.pin-output {
  position: absolute;
  right: -5px;
  top: 50%;
  transform: translateY(-50%);
}

/* Data pins in var rows */
.var-row .pin-input {
  position: absolute;
  left: -4px;
  top: 50%;
  transform: translateY(-50%);
}

.var-row .pin-output {
  position: absolute;
  right: -4px;
  top: 50%;
  transform: translateY(-50%);
}
</style>
