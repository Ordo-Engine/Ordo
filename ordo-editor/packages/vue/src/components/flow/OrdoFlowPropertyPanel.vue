<script setup lang="ts">
/**
 * OrdoFlowPropertyPanel - Property panel for selected node
 * 选中节点的属性面板
 */
import { computed } from 'vue';
import type { Step } from '@ordo/editor-core';
import OrdoStepEditor from '../step/OrdoStepEditor.vue';
import OrdoIcon from '../icons/OrdoIcon.vue';
import { useI18n } from '../../locale';
import type { FlowNode } from './utils/converter';
import type { FieldSuggestion } from '../base/OrdoExpressionInput.vue';

export interface Props {
  node: FlowNode;
  availableSteps: Step[];
  suggestions?: FieldSuggestion[];
  disabled?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  suggestions: () => [],
  disabled: false,
});

const emit = defineEmits<{
  update: [step: Step];
  'set-start': [nodeId: string];
  delete: [];
  close: [];
}>();

const { t } = useI18n();

const step = computed(() => props.node.data.step);
const isStart = computed(() => props.node.data.isStart);

const nodeTypeLabel = computed(() => {
  switch (step.value.type) {
    case 'decision':
      return t('step.decision');
    case 'action':
      return t('step.action');
    case 'terminal':
      return t('step.terminal');
    default:
      return t('common.unknown');
  }
});

const nodeTypeColor = computed(() => {
  switch (step.value.type) {
    case 'decision':
      return 'var(--ordo-node-decision)';
    case 'action':
      return 'var(--ordo-node-action)';
    case 'terminal':
      return 'var(--ordo-node-terminal)';
    default:
      return 'var(--ordo-text-tertiary)';
  }
});

function handleStepUpdate(updatedStep: Step) {
  emit('update', updatedStep);
}

function handleStepChange(updatedStep: Step) {
  emit('update', updatedStep);
}
</script>

<template>
  <div class="property-panel">
    <!-- Header -->
    <div class="panel-header">
      <div class="header-title">
        <OrdoIcon :name="step.type" :size="16" :style="{ color: nodeTypeColor }" />
        <span class="type-label">{{ nodeTypeLabel }}</span>
        <span v-if="isStart" class="start-badge">{{ t('step.start') }}</span>
      </div>
      <button class="close-btn" @click="emit('close')" :title="t('common.close')">
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

    <!-- Actions Bar -->
    <div class="panel-actions">
      <button v-if="!isStart" class="action-btn" @click="emit('set-start', node.id)">
        <OrdoIcon name="start" :size="14" />
        {{ t('step.setAsStart') }}
      </button>
      <button class="action-btn danger" @click="emit('delete')">
        <OrdoIcon name="delete" :size="14" />
        {{ t('common.delete') }}
      </button>
    </div>

    <!-- Step Editor -->
    <div class="panel-content">
      <OrdoStepEditor
        :model-value="step"
        :available-steps="availableSteps"
        :suggestions="suggestions"
        :disabled="disabled"
        :show-delete="false"
        @update:model-value="handleStepUpdate"
        @change="handleStepChange"
      />
    </div>
  </div>
</template>

<style scoped>
.property-panel {
  position: absolute;
  top: 0;
  right: 0;
  width: 320px;
  height: 100%;
  background: var(--ordo-bg-panel);
  border-left: 1px solid var(--ordo-border-color);
  display: flex;
  flex-direction: column;
  z-index: 100;
  box-shadow: -4px 0 12px rgba(0, 0, 0, 0.08);
}

.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid var(--ordo-border-color);
  background: var(--ordo-bg-item);
}

.header-title {
  display: flex;
  align-items: center;
  gap: 8px;
}

.type-label {
  font-size: 13px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.start-badge {
  font-size: 9px;
  font-weight: 700;
  color: #fff;
  background: var(--ordo-accent);
  padding: 2px 6px;
  border-radius: 3px;
}

.close-btn {
  background: transparent;
  border: none;
  cursor: pointer;
  color: var(--ordo-text-tertiary);
  padding: 4px;
  border-radius: var(--ordo-radius-sm);
}

.close-btn:hover {
  background: var(--ordo-bg-item-hover);
  color: var(--ordo-text-primary);
}

.panel-actions {
  display: flex;
  gap: 8px;
  padding: 8px 16px;
  border-bottom: 1px solid var(--ordo-border-light);
}

.action-btn {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 6px 10px;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  background: var(--ordo-bg-item);
  color: var(--ordo-text-secondary);
  font-size: 11px;
  cursor: pointer;
  transition: all 0.15s;
}

.action-btn:hover {
  background: var(--ordo-bg-item-hover);
  color: var(--ordo-text-primary);
}

.action-btn.danger:hover {
  background: var(--ordo-error-bg, rgba(229, 20, 0, 0.1));
  color: var(--ordo-error);
  border-color: var(--ordo-error);
}

.panel-content {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}
</style>
