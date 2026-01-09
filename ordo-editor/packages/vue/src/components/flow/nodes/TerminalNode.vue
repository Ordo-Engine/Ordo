<script setup lang="ts">
/**
 * Terminal Node - Flow editor terminal step node (Blueprint style)
 * 终结节点 - 流程编辑器终结步骤节点（蓝图风格）
 */
import { computed } from 'vue';
import { Handle, Position } from '@vue-flow/core';
import type { TerminalStep, OutputField } from '@ordo/editor-core';
import { exprToString } from '@ordo/editor-core';
import OrdoIcon from '../../icons/OrdoIcon.vue';
import { PIN_COLORS } from '../types';
import { useI18n } from '../../../locale';
import type { StepTraceInfo } from './ExecutionAnnotation.vue';
import ExecutionAnnotation from './ExecutionAnnotation.vue';

export interface Props {
  data: {
    step: TerminalStep;
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
  return props.data.label || props.data.step.name || t('step.terminal');
});

const outputs = computed<OutputField[]>(() => props.data.step.output || []);
const resultCode = computed(() => props.data.step.code || 'RESULT');

/** Format output value for display */
function formatOutputValue(output: OutputField): string {
  if (output.value !== undefined) {
    const val = output.value;
    if (typeof val === 'string') return `"${val}"`;
    if (typeof val === 'boolean') return val ? 'true' : 'false';
    if (typeof val === 'object') return JSON.stringify(val);
    return String(val);
  }
  if (output.expression) {
    const exprStr = exprToString(output.expression);
    return exprStr.length > 15 ? exprStr.slice(0, 15) + '...' : exprStr;
  }
  return '?';
}
</script>

<template>
  <div 
    class="flow-node terminal-node" 
    :class="{ selected, 'is-start': data.isStart }"
  >
    <!-- Execution Annotation -->
    <ExecutionAnnotation 
      v-if="data.executionAnnotation" 
      :trace="data.executionAnnotation"
      position="top"
    />
    
    <!-- Node Header -->
    <div class="node-header">
      <!-- Exec Input Pin (Left) -->
      <Handle 
        type="target" 
        :position="Position.Left" 
        class="pin pin-exec pin-input"
        id="input"
      >
        <svg class="pin-shape" width="10" height="10" viewBox="0 0 10 10">
          <polygon points="0,0 10,5 0,10" :fill="PIN_COLORS.execInput" class="pin-fill" />
        </svg>
      </Handle>
      
      <span class="node-badge start" v-if="data.isStart">{{ t('step.start') }}</span>
      <OrdoIcon name="terminal" :size="14" class="node-icon" />
      <span class="node-title">{{ displayTitle }}</span>
      <span class="node-type-badge">{{ t('step.typeTerminal') }}</span>
    </div>
    
    <!-- Result Code Section -->
    <div class="node-section result-section">
      <div class="result-code">{{ resultCode }}</div>
    </div>
    
    <!-- Output Fields Section -->
    <div class="node-section outputs-section" v-if="outputs.length > 0">
      <div 
        v-for="output in outputs" 
        :key="output.field"
        class="output-row"
        :title="`${output.field} = ${formatOutputValue(output)}`"
      >
        <!-- Data Input (for expression dependencies) -->
        <Handle 
          type="target" 
          :position="Position.Left" 
          class="pin pin-data pin-input"
          :id="`data-in-${output.field}`"
        >
          <svg class="pin-shape" width="8" height="8" viewBox="0 0 8 8">
            <circle cx="4" cy="4" r="3.5" :fill="PIN_COLORS.dataPin" class="pin-fill" />
          </svg>
        </Handle>
        
        <span class="output-name">{{ output.field }}</span>
        <span class="output-op">=</span>
        <span class="output-value">{{ formatOutputValue(output) }}</span>
      </div>
    </div>

    <!-- No output exec handle for terminal nodes -->
  </div>
</template>

<style scoped>
.flow-node {
  background: var(--ordo-bg-item, #1e1e1e);
  border: 1px solid var(--ordo-border-color, #3c3c3c);
  border-radius: 4px;
  min-width: 160px;
  max-width: 240px;
  font-family: var(--ordo-font-sans);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
  transition: box-shadow 0.15s, border-color 0.15s;
  position: relative;
}

.flow-node:hover {
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
  border-color: var(--ordo-text-tertiary, #6c6c6c);
}

.flow-node.selected {
  border-color: var(--ordo-node-terminal, #388a34);
  box-shadow: 0 0 0 2px rgba(56, 138, 52, 0.3);
}

.flow-node.is-start {
  border-color: var(--ordo-node-terminal, #388a34);
}

/* Terminal specific - top border accent */
.terminal-node {
  border-top: 3px solid var(--ordo-node-terminal, #388a34);
}

/* Header */
.node-header {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 12px 8px 20px;
  background: rgba(56, 138, 52, 0.1);
  border-bottom: 1px solid var(--ordo-border-light, #2d2d2d);
  position: relative;
}

.node-badge.start {
  font-size: 8px;
  font-weight: 700;
  color: #fff;
  background: var(--ordo-node-terminal, #388a34);
  padding: 2px 4px;
  border-radius: 2px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.node-icon {
  color: var(--ordo-node-terminal, #388a34);
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

/* Result code section */
.result-section {
  padding: 8px 12px;
  display: flex;
  justify-content: center;
}

.result-code {
  font-family: var(--ordo-font-mono);
  font-size: 12px;
  font-weight: 700;
  color: var(--ordo-node-terminal, #388a34);
  background: rgba(56, 138, 52, 0.15);
  padding: 6px 16px;
  border-radius: 3px;
  text-align: center;
  letter-spacing: 0.5px;
}

/* Output fields section */
.outputs-section {
  background: rgba(74, 158, 255, 0.05);
}

.output-row {
  display: flex;
  align-items: center;
  padding: 4px 12px 4px 20px;
  position: relative;
  min-height: 24px;
  gap: 4px;
}

.output-name {
  font-size: 11px;
  font-weight: 500;
  color: var(--ordo-text-secondary, #b0b0b0);
  font-family: var(--ordo-font-mono, monospace);
}

.output-op {
  font-size: 11px;
  color: var(--ordo-text-tertiary, #888);
}

.output-value {
  font-size: 10px;
  color: var(--ordo-text-tertiary, #888);
  font-family: var(--ordo-font-mono, monospace);
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
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
  transition: filter 0.15s ease, fill 0.15s ease;
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

/* Data pins in output rows */
.output-row .pin-input {
  position: absolute;
  left: -4px;
  top: 50%;
  transform: translateY(-50%);
}
</style>
