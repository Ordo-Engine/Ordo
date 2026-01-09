<script setup lang="ts">
/**
 * OrdoDecisionEditor - Decision step editor (Refactored)
 * 决策步骤编辑器
 */
import { computed } from 'vue';
import type { DecisionStep, Branch, Condition, Step } from '@ordo/editor-core';
import { Condition as ConditionFactory, Expr, generateId } from '@ordo/editor-core';
import OrdoConditionBuilder from '../base/OrdoConditionBuilder.vue';
import OrdoIcon from '../icons/OrdoIcon.vue';
import { useI18n } from '../../locale';
import type { FieldSuggestion } from '../base/OrdoExpressionInput.vue';

export interface Props {
  /** Decision step data */
  modelValue: DecisionStep;
  /** Available steps to link to */
  availableSteps?: Step[];
  /** Field suggestions for expressions */
  suggestions?: FieldSuggestion[];
  /** Whether the editor is disabled */
  disabled?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  availableSteps: () => [],
  suggestions: () => [],
  disabled: false,
});

const emit = defineEmits<{
  'update:modelValue': [value: DecisionStep];
  'change': [value: DecisionStep];
}>();

const { t } = useI18n();

// Computed step options for dropdowns
const stepOptions = computed(() => {
  return props.availableSteps
    .filter((s) => s.id !== props.modelValue.id)
    .map((s) => ({
      value: s.id,
      label: `${s.name} (${s.type})`,
    }));
});

// ... (Keep existing update logic) ...
function updateName(event: Event) {
  const target = event.target as HTMLInputElement;
  emit('update:modelValue', { ...props.modelValue, name: target.value });
}

function handleNameBlur() {
  emit('change', props.modelValue);
}

function updateDescription(event: Event) {
  const target = event.target as HTMLTextAreaElement;
  emit('update:modelValue', { ...props.modelValue, description: target.value || undefined });
}

function updateDefaultNext(event: Event) {
  const target = event.target as HTMLSelectElement;
  const newStep = { ...props.modelValue, defaultNextStepId: target.value };
  emit('update:modelValue', newStep);
  emit('change', newStep);
}

// Branch operations
function addBranch() {
  const newBranch: Branch = {
    id: generateId('branch'),
    label: `Branch ${props.modelValue.branches.length + 1}`,
    condition: ConditionFactory.simple(Expr.variable('$.field'), 'eq', Expr.string('')),
    nextStepId: props.modelValue.defaultNextStepId,
  };
  const newStep = { ...props.modelValue, branches: [...props.modelValue.branches, newBranch] };
  emit('update:modelValue', newStep);
  emit('change', newStep);
}

function updateBranch(index: number, branch: Partial<Branch>) {
  const newBranches = [...props.modelValue.branches];
  newBranches[index] = { ...newBranches[index], ...branch };
  const newStep = { ...props.modelValue, branches: newBranches };
  emit('update:modelValue', newStep);
  if (branch.nextStepId !== undefined || branch.condition !== undefined) {
    emit('change', newStep);
  }
}

function removeBranch(index: number) {
  const newBranches = props.modelValue.branches.filter((_, i) => i !== index);
  const newStep = { ...props.modelValue, branches: newBranches };
  emit('update:modelValue', newStep);
  emit('change', newStep);
}

function moveBranch(index: number, direction: 'up' | 'down') {
  if (direction === 'up' && index === 0) return;
  if (direction === 'down' && index >= props.modelValue.branches.length - 1) return;
  
  const newBranches = [...props.modelValue.branches];
  const targetIndex = direction === 'up' ? index - 1 : index + 1;
  [newBranches[targetIndex], newBranches[index]] = [newBranches[index], newBranches[targetIndex]];
  
  const newStep = { ...props.modelValue, branches: newBranches };
  emit('update:modelValue', newStep);
  emit('change', newStep);
}
</script>

<template>
  <div class="ordo-editor-panel decision">
    <!-- Header / Name -->
    <div class="ordo-form-row">
      <div class="ordo-form-group grow">
        <label>{{ t('common.name') }}</label>
        <input
          :value="modelValue.name"
          :disabled="disabled"
          class="ordo-input-base"
          @input="updateName"
          @blur="handleNameBlur"
        />
      </div>
    </div>

    <!-- Description -->
    <div class="ordo-form-row">
      <div class="ordo-form-group full">
        <label>{{ t('common.description') }} <span class="subtle">({{ t('common.optional') }})</span></label>
        <textarea
          :value="modelValue.description || ''"
          :disabled="disabled"
          rows="2"
          class="ordo-input-base"
          @input="updateDescription"
        />
      </div>
    </div>

    <!-- Branches List -->
    <div class="ordo-section">
      <div class="ordo-section-header">
        <span class="title">{{ t('step.branches') }}</span>
        <button class="ordo-btn-text" :disabled="disabled" @click="addBranch">
          <OrdoIcon name="add" :size="12" /> {{ t('step.addBranch') }}
        </button>
      </div>

      <div class="ordo-branches-list">
        <div
          v-for="(branch, index) in modelValue.branches"
          :key="branch.id"
          class="ordo-branch-item"
        >
          <!-- Branch Header Line -->
          <div class="ordo-branch-header">
            <span class="ordo-branch-index">#{{ index + 1 }}</span>
            <input
              :value="branch.label"
              :disabled="disabled"
              placeholder="Branch Label"
              class="ordo-input-transparent"
              @input="updateBranch(index, { label: ($event.target as HTMLInputElement).value })"
            />
            
            <div class="ordo-branch-actions">
              <button class="ordo-btn-icon" :disabled="index === 0" @click="moveBranch(index, 'up')">
                <OrdoIcon name="arrow-up" :size="12" />
              </button>
              <button class="ordo-btn-icon" :disabled="index >= modelValue.branches.length - 1" @click="moveBranch(index, 'down')">
                <OrdoIcon name="arrow-down" :size="12" />
              </button>
              <button class="ordo-btn-icon danger" @click="removeBranch(index)">
                <OrdoIcon name="delete" :size="12" />
              </button>
            </div>
          </div>

          <!-- Branch Condition -->
          <div class="ordo-branch-body">
            <div class="ordo-branch-row">
              <span class="label">If</span>
              <div class="content">
                <OrdoConditionBuilder
                  :model-value="branch.condition"
                  :suggestions="suggestions"
                  :disabled="disabled"
                  @update:model-value="updateBranch(index, { condition: $event })"
                />
              </div>
            </div>
            
            <div class="ordo-branch-row">
              <span class="label">Then</span>
              <div class="content">
                <select
                  :value="branch.nextStepId"
                  :disabled="disabled"
                  class="ordo-input-base"
                  @change="updateBranch(index, { nextStepId: ($event.target as HTMLSelectElement).value })"
                >
                  <option value="">-- {{ t('step.nextStep') }} --</option>
                  <option v-for="opt in stepOptions" :key="opt.value" :value="opt.value">
                    {{ opt.label }}
                  </option>
                </select>
              </div>
            </div>
          </div>
        </div>
        
        <div v-if="modelValue.branches.length === 0" class="ordo-empty-state">
          No branches defined.
        </div>
      </div>
    </div>

    <!-- Default Next -->
    <div class="ordo-form-row">
      <div class="ordo-form-group full">
        <label>{{ t('step.defaultNext') }}</label>
        <select
          :value="modelValue.defaultNextStepId"
          :disabled="disabled"
          class="ordo-input-base"
          @change="updateDefaultNext"
        >
          <option value="">-- {{ t('step.nextStep') }} --</option>
          <option v-for="opt in stepOptions" :key="opt.value" :value="opt.value">
            {{ opt.label }}
          </option>
        </select>
      </div>
    </div>
  </div>
</template>

<style scoped>
.ordo-editor-panel {
  display: flex;
  flex-direction: column;
  gap: var(--ordo-space-md);
  font-size: var(--ordo-font-size-sm);
  color: var(--ordo-text-primary);
}

.ordo-form-row {
  display: flex;
  gap: var(--ordo-space-md);
}

.ordo-form-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.ordo-form-group.grow { flex: 1; }
.ordo-form-group.full { width: 100%; }

.ordo-form-group label {
  font-size: 11px;
  font-weight: 500;
  color: var(--ordo-text-secondary);
  text-transform: uppercase;
}

.subtle { color: var(--ordo-text-tertiary); font-weight: normal; text-transform: none; }

.ordo-section {
  display: flex;
  flex-direction: column;
  gap: var(--ordo-space-sm);
  margin-top: var(--ordo-space-sm);
}

.ordo-section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--ordo-border-light);
  padding-bottom: 4px;
}

.ordo-section-header .title {
  font-size: 11px;
  font-weight: 600;
  color: var(--ordo-text-secondary);
  text-transform: uppercase;
}

.ordo-btn-text {
  background: none;
  border: none;
  color: var(--ordo-accent);
  font-size: 11px;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 6px;
  border-radius: var(--ordo-radius-sm);
}

.ordo-btn-text:hover { background: var(--ordo-accent-bg); }

/* Branches */
.ordo-branches-list {
  display: flex;
  flex-direction: column;
  gap: 1px; /* Divider effect */
  background: var(--ordo-border-light); /* Gap color */
  border: 1px solid var(--ordo-border-light);
  border-radius: var(--ordo-radius-md);
  overflow: hidden;
}

.ordo-branch-item {
  background: var(--ordo-bg-item);
  display: flex;
  flex-direction: column;
}

.ordo-branch-header {
  display: flex;
  align-items: center;
  padding: 6px 8px;
  background: var(--ordo-bg-panel);
  border-bottom: 1px solid var(--ordo-border-light);
  gap: 8px;
}

.ordo-branch-index {
  font-family: var(--ordo-font-mono);
  font-size: 10px;
  color: var(--ordo-text-tertiary);
  width: 24px;
}

.ordo-input-transparent {
  flex: 1;
  background: transparent;
  border: none;
  font-weight: 500;
  font-size: 12px;
  color: var(--ordo-text-primary);
}

.ordo-input-transparent:focus {
  outline: none;
  background: var(--ordo-bg-input);
}

.ordo-branch-body {
  padding: 8px 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.ordo-branch-row {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.ordo-branch-row .label {
  width: 32px;
  font-size: 11px;
  color: var(--ordo-text-tertiary);
  margin-top: 6px;
  text-align: right;
  font-family: var(--ordo-font-mono);
}

.ordo-branch-row .content {
  flex: 1;
}

.ordo-empty-state {
  padding: 16px;
  text-align: center;
  color: var(--ordo-text-tertiary);
  font-style: italic;
  background: var(--ordo-bg-item);
}

.ordo-btn-icon.danger { color: var(--ordo-error); }
.ordo-btn-icon.danger:hover { background: var(--ordo-error-bg); }
</style>
