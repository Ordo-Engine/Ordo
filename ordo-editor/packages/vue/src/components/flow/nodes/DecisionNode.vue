<script setup lang="ts">
/**
 * Decision Node - Flow editor decision step node (Blueprint style)
 * 决策节点 - 流程编辑器决策步骤节点（蓝图风格）
 */
import { computed } from 'vue';
import { Handle, Position } from '@vue-flow/core';
import type { DecisionStep } from '@ordo/editor-core';
import { conditionToString } from '@ordo/editor-core';
import OrdoIcon from '../../icons/OrdoIcon.vue';
import { PIN_COLORS } from '../types';
import { useI18n } from '../../../locale';

import type { StepTraceInfo } from './ExecutionAnnotation.vue';
import ExecutionAnnotation from './ExecutionAnnotation.vue';

export interface Props {
  data: {
    step: DecisionStep;
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
  return props.data.label || props.data.step.name || t('step.decision');
});

const branches = computed(() => props.data.step.branches || []);
const branchCount = computed(() => branches.value.length);

/** Check if step has any assignments (data outputs) */
const assignments = computed(() => {
  // Decision steps typically don't have assignments, but we support it
  return [];
});

/** Get condition display text for a branch */
function getBranchLabel(branch: { name?: string; condition?: unknown }): string {
  if (branch.name) return branch.name;
  if (branch.condition) {
    const condStr = conditionToString(branch.condition as Parameters<typeof conditionToString>[0]);
    return condStr.length > 20 ? condStr.slice(0, 20) + '...' : condStr;
  }
  return t('step.branch');
}

/** Get full condition for tooltip */
function getBranchTooltip(branch: { condition?: unknown }): string {
  if (branch.condition) {
    return conditionToString(branch.condition as Parameters<typeof conditionToString>[0]);
  }
  return '';
}
</script>

<template>
  <div class="flow-node decision-node" :class="{ selected, 'is-start': data.isStart }">
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
      <OrdoIcon name="decision" :size="14" class="node-icon" />
      <span class="node-title">{{ displayTitle }}</span>
      <span class="node-type-badge">{{ t('step.typeDecision') }}</span>
    </div>

    <!-- Branch Outputs Section -->
    <div class="node-section branches-section" v-if="branchCount > 0">
      <div
        v-for="branch in branches"
        :key="branch.id"
        class="branch-row"
        :title="getBranchTooltip(branch)"
      >
        <span class="branch-label">{{ getBranchLabel(branch) }}</span>
        <Handle
          type="source"
          :position="Position.Right"
          class="pin pin-exec pin-output pin-branch"
          :id="branch.id"
        >
          <svg class="pin-shape" width="10" height="10" viewBox="0 0 10 10">
            <polygon points="0,0 10,5 0,10" :fill="PIN_COLORS.execBranch" class="pin-fill" />
          </svg>
        </Handle>
      </div>

      <!-- Default branch -->
      <div class="branch-row branch-default">
        <span class="branch-label default-label">{{ t('step.default') }}</span>
        <Handle
          type="source"
          :position="Position.Right"
          class="pin pin-exec pin-output pin-default"
          id="default"
        >
          <svg class="pin-shape" width="10" height="10" viewBox="0 0 10 10">
            <polygon points="0,0 10,5 0,10" :fill="PIN_COLORS.execDefault" class="pin-fill" />
          </svg>
        </Handle>
      </div>
    </div>

    <!-- Empty state if no branches -->
    <div class="node-section empty-section" v-else>
      <div class="branch-row branch-default">
        <span class="branch-label default-label">{{ t('step.default') }}</span>
        <Handle
          type="source"
          :position="Position.Right"
          class="pin pin-exec pin-output pin-default"
          id="default"
        >
          <svg class="pin-shape" width="10" height="10" viewBox="0 0 10 10">
            <polygon points="0,0 10,5 0,10" :fill="PIN_COLORS.execDefault" class="pin-fill" />
          </svg>
        </Handle>
      </div>
    </div>

    <!-- Data Outputs Section (if any assignments) -->
    <div class="node-section data-section" v-if="assignments.length > 0">
      <div v-for="assign in assignments" :key="assign" class="data-row">
        <Handle
          type="source"
          :position="Position.Right"
          class="pin pin-data pin-output"
          :id="`data-${assign}`"
        >
          <svg class="pin-shape" width="8" height="8" viewBox="0 0 8 8">
            <circle cx="4" cy="4" r="3.5" :fill="PIN_COLORS.dataPin" class="pin-fill" />
          </svg>
        </Handle>
        <span class="data-label">{{ assign }}</span>
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
  max-width: 260px;
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
  border-color: var(--ordo-node-decision, #b76e00);
  box-shadow: 0 0 0 2px rgba(183, 110, 0, 0.3);
}

.flow-node.is-start {
  border-color: var(--ordo-node-decision, #b76e00);
}

/* Decision specific - top border accent */
.decision-node {
  border-top: 3px solid var(--ordo-node-decision, #b76e00);
}

/* Header */
.node-header {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 12px 8px 20px;
  background: rgba(183, 110, 0, 0.1);
  border-bottom: 1px solid var(--ordo-border-light, #2d2d2d);
  position: relative;
}

.node-badge.start {
  font-size: 8px;
  font-weight: 700;
  color: #fff;
  background: var(--ordo-node-decision, #b76e00);
  padding: 2px 4px;
  border-radius: 2px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.node-icon {
  color: var(--ordo-node-decision, #b76e00);
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

/* Branch rows */
.branch-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 12px 4px 12px;
  position: relative;
  min-height: 24px;
}

.branch-label {
  font-size: 11px;
  color: var(--ordo-text-secondary, #b0b0b0);
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  padding-right: 16px;
}

.default-label {
  color: var(--ordo-text-tertiary, #888);
  font-style: italic;
}

/* Data rows */
.data-section {
  background: rgba(74, 158, 255, 0.05);
}

.data-row {
  display: flex;
  align-items: center;
  padding: 4px 12px;
  position: relative;
  min-height: 24px;
}

.data-label {
  font-size: 10px;
  color: var(--ordo-text-tertiary, #888);
  font-family: var(--ordo-font-mono, monospace);
  padding-left: 16px;
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

/* Data pin positioning */
.data-row .pin-output {
  position: absolute;
  left: -4px;
  right: auto;
}
</style>
