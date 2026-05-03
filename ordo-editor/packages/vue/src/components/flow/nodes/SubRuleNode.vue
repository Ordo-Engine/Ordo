<script setup lang="ts">
/**
 * SubRule Node - Flow editor sub-rule invocation node
 */
import { computed } from 'vue';
import { Handle, Position } from '@vue-flow/core';
import type { SubRuleStep } from '@ordo-engine/editor-core';
import OrdoIcon from '../../icons/OrdoIcon.vue';
import { PIN_COLORS } from '../types';
import { useI18n } from '../../../locale';
import type { StepTraceInfo } from './ExecutionAnnotation.vue';
import ExecutionAnnotation from './ExecutionAnnotation.vue';

export interface Props {
  data: {
    step: SubRuleStep;
    isStart: boolean;
    label: string;
    executionAnnotation?: StepTraceInfo | null;
  };
  selected?: boolean;
}

const props = defineProps<Props>();
const { t } = useI18n();

const displayTitle = computed(() => props.data.label || props.data.step.name || t('step.subRule'));
const bindingCount = computed(() => props.data.step.bindings?.length ?? 0);
const outputCount = computed(() => props.data.step.outputs?.length ?? 0);
</script>

<template>
  <div class="flow-node sub-rule-node" :class="{ selected, 'is-start': data.isStart }">
    <ExecutionAnnotation
      v-if="data.executionAnnotation"
      :trace="data.executionAnnotation"
      position="top"
    />

    <div class="node-header">
      <Handle type="target" :position="Position.Left" class="pin pin-exec pin-input" id="input">
        <svg class="pin-shape" width="10" height="10" viewBox="0 0 10 10">
          <polygon points="0,0 10,5 0,10" :fill="PIN_COLORS.execInput" class="pin-fill" />
        </svg>
      </Handle>

      <span class="node-badge start" v-if="data.isStart">{{ t('step.start') }}</span>
      <OrdoIcon name="sub_rule" :size="14" class="node-icon" />
      <span class="node-title">{{ displayTitle }}</span>
      <span class="node-type-badge">{{ t('step.typeSubRule') }}</span>
    </div>

    <div class="node-section ref-section">
      <span class="ref-label">{{ t('step.refName') }}</span>
      <span class="ref-value">{{ data.step.refName || t('common.none') }}</span>
    </div>

    <div class="node-section io-section">
      <span class="io-chip">{{ bindingCount }} {{ t('step.bindings') }}</span>
      <span class="io-chip">{{ outputCount }} {{ t('step.outputs') }}</span>
    </div>

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
  min-width: 190px;
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

.flow-node.selected,
.flow-node.is-start {
  border-color: var(--ordo-node-sub-rule, #5b708a);
}

.flow-node.selected {
  box-shadow: 0 0 0 2px rgba(91, 112, 138, 0.3);
}

.sub-rule-node {
  border-top: 3px solid var(--ordo-node-sub-rule, #5b708a);
}

.node-header {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 12px 8px 20px;
  background: rgba(91, 112, 138, 0.12);
  border-bottom: 1px solid var(--ordo-border-light, #2d2d2d);
  position: relative;
}

.node-badge.start {
  font-size: 8px;
  font-weight: 700;
  color: #fff;
  background: var(--ordo-node-sub-rule, #5b708a);
  padding: 2px 4px;
  border-radius: 2px;
  text-transform: uppercase;
}

.node-icon {
  color: var(--ordo-node-sub-rule, #5b708a);
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

.node-type-badge,
.io-chip {
  font-size: 9px;
  color: var(--ordo-text-tertiary, #888);
  background: var(--ordo-bg-panel, #252525);
  padding: 2px 5px;
  border-radius: 2px;
  flex-shrink: 0;
}

.node-section {
  padding: 6px 12px;
  border-bottom: 1px solid var(--ordo-border-light, #2d2d2d);
}

.ref-section,
.io-section,
.exec-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ref-label,
.exec-label {
  font-size: 10px;
  color: var(--ordo-text-tertiary, #888);
  text-transform: uppercase;
}

.ref-value {
  font-family: var(--ordo-font-mono);
  font-size: 11px;
  color: var(--ordo-text-primary, #e0e0e0);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.io-section {
  justify-content: space-between;
}

.exec-section {
  border-bottom: none;
}

.exec-row {
  justify-content: space-between;
  position: relative;
}

.pin {
  width: 14px;
  height: 14px;
  border: none;
  background: transparent;
}

.pin-input {
  left: -12px;
  padding: 5px;
}

.pin-output {
  right: -19px;
}
</style>
