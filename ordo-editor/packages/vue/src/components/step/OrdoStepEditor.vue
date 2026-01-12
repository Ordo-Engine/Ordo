<script setup lang="ts">
/**
 * OrdoStepEditor - Unified step editor that renders based on step type
 * 统一步骤编辑器，根据步骤类型渲染
 */
import { computed } from 'vue';
import type { Step, DecisionStep, ActionStep, TerminalStep } from '@ordo/editor-core';
import OrdoDecisionEditor from './OrdoDecisionEditor.vue';
import OrdoActionEditor from './OrdoActionEditor.vue';
import OrdoTerminalEditor from './OrdoTerminalEditor.vue';
import type { FieldSuggestion } from '../base/OrdoExpressionInput.vue';

export interface Props {
  /** Step data */
  modelValue: Step;
  /** Available steps to link to */
  availableSteps?: Step[];
  /** Field suggestions for expressions */
  suggestions?: FieldSuggestion[];
  /** Whether the editor is disabled */
  disabled?: boolean;
  /** Whether to show delete button */
  showDelete?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  availableSteps: () => [],
  suggestions: () => [],
  disabled: false,
  showDelete: true,
});

const emit = defineEmits<{
  'update:modelValue': [value: Step];
  change: [value: Step];
  delete: [stepId: string];
}>();

// Type guards
const isDecision = computed(() => props.modelValue.type === 'decision');
const isAction = computed(() => props.modelValue.type === 'action');
const isTerminal = computed(() => props.modelValue.type === 'terminal');

// Handle updates
function handleUpdate(step: Step) {
  emit('update:modelValue', step);
}

function handleChange(step: Step) {
  emit('change', step);
}

function handleDelete() {
  emit('delete', props.modelValue.id);
}
</script>

<template>
  <div class="ordo-step-editor">
    <!-- Delete button -->
    <button
      v-if="showDelete && !disabled"
      type="button"
      class="ordo-step-editor__delete"
      title="Delete step"
      @click="handleDelete"
    >
      🗑
    </button>

    <!-- Decision step -->
    <OrdoDecisionEditor
      v-if="isDecision"
      :model-value="modelValue as DecisionStep"
      :available-steps="availableSteps"
      :suggestions="suggestions"
      :disabled="disabled"
      @update:model-value="handleUpdate"
      @change="handleChange"
    />

    <!-- Action step -->
    <OrdoActionEditor
      v-if="isAction"
      :model-value="modelValue as ActionStep"
      :available-steps="availableSteps"
      :suggestions="suggestions"
      :disabled="disabled"
      @update:model-value="handleUpdate"
      @change="handleChange"
    />

    <!-- Terminal step -->
    <OrdoTerminalEditor
      v-if="isTerminal"
      :model-value="modelValue as TerminalStep"
      :suggestions="suggestions"
      :disabled="disabled"
      @update:model-value="handleUpdate"
      @change="handleChange"
    />
  </div>
</template>

<style scoped>
.ordo-step-editor {
  position: relative;
}

.ordo-step-editor__delete {
  position: absolute;
  top: var(--ordo-space-sm, 8px);
  right: var(--ordo-space-sm, 8px);
  padding: var(--ordo-space-xs, 4px);
  border: none;
  background: transparent;
  font-size: var(--ordo-font-size-md, 16px);
  cursor: pointer;
  opacity: 0.4;
  transition: opacity 0.15s;
  z-index: 10;
}

.ordo-step-editor__delete:hover {
  opacity: 1;
}
</style>
