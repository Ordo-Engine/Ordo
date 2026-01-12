<script setup lang="ts">
/**
 * OrdoActionEditor - Action step editor (Refactored)
 * 动作步骤编辑器
 */
import { computed } from 'vue';
import type { ActionStep, VariableAssignment, Step } from '@ordo/editor-core';
import { Expr, generateId } from '@ordo/editor-core';
import OrdoExpressionInput from '../base/OrdoExpressionInput.vue';
import OrdoIcon from '../icons/OrdoIcon.vue';
import { useI18n } from '../../locale';
import type { FieldSuggestion } from '../base/OrdoExpressionInput.vue';

export interface Props {
  /** Action step data */
  modelValue: ActionStep;
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
  'update:modelValue': [value: ActionStep];
  change: [value: ActionStep];
}>();

const { t } = useI18n();

const stepOptions = computed(() => {
  return props.availableSteps
    .filter((s) => s.id !== props.modelValue.id)
    .map((s) => ({ value: s.id, label: `${s.name} (${s.type})` }));
});

// ... (Existing logic same) ...
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

function updateNextStep(event: Event) {
  const target = event.target as HTMLSelectElement;
  const newStep = { ...props.modelValue, nextStepId: target.value };
  emit('update:modelValue', newStep);
  emit('change', newStep);
}

// Variable assignment operations
function addAssignment() {
  const newAssignment: VariableAssignment = {
    name: `var_${generateId('').slice(0, 6)}`,
    value: Expr.string(''),
  };
  const newStep = {
    ...props.modelValue,
    assignments: [...(props.modelValue.assignments || []), newAssignment],
  };
  emit('update:modelValue', newStep);
  emit('change', newStep);
}

function updateAssignment(index: number, assignment: Partial<VariableAssignment>) {
  const assignments = [...(props.modelValue.assignments || [])];
  assignments[index] = { ...assignments[index], ...assignment };

  // For value update, handle expression parsing inside updateAssignment logic if needed,
  // but here we just pass expression directly

  const newStep = { ...props.modelValue, assignments };
  emit('update:modelValue', newStep);
  emit('change', newStep);
}

function removeAssignment(index: number) {
  const assignments = (props.modelValue.assignments || []).filter((_, i) => i !== index);
  const newStep = {
    ...props.modelValue,
    assignments: assignments.length > 0 ? assignments : undefined,
  };
  emit('update:modelValue', newStep);
  emit('change', newStep);
}

// Logging
function updateLogging(logging: Partial<NonNullable<ActionStep['logging']>>) {
  const currentLogging = props.modelValue.logging || { message: Expr.string(''), level: 'info' };
  const newLogging = { ...currentLogging, ...logging };

  // If level is cleared and no message, remove logging
  if (
    !newLogging.level &&
    (!newLogging.message || (newLogging.message.type === 'literal' && !newLogging.message.value))
  ) {
    const newStep = { ...props.modelValue, logging: undefined };
    emit('update:modelValue', newStep);
    emit('change', newStep);
    return;
  }

  const newStep = { ...props.modelValue, logging: newLogging as any };
  emit('update:modelValue', newStep);
  emit('change', newStep);
}

// Helper
function getExprValue(expr?: { type: string; value?: unknown; path?: string }): string {
  if (!expr) return '';
  if (expr.type === 'variable' && expr.path) return expr.path;
  if (expr.type === 'literal') {
    if (expr.value === null) return 'null';
    if (typeof expr.value === 'string') return expr.value;
    return String(expr.value);
  }
  return '';
}

function updateExprValue(val: string): any {
  // Simple parser simulation
  if (val.startsWith('$')) return Expr.variable(val);
  if (!isNaN(Number(val)) && val !== '') return Expr.number(Number(val));
  if (val === 'true') return Expr.boolean(true);
  if (val === 'false') return Expr.boolean(false);
  return Expr.string(val);
}
</script>

<template>
  <div class="ordo-editor-panel action">
    <!-- Header -->
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
        <label>{{ t('common.description') }}</label>
        <textarea
          :value="modelValue.description || ''"
          :disabled="disabled"
          rows="2"
          class="ordo-input-base"
          @input="updateDescription"
        />
      </div>
    </div>

    <!-- Assignments -->
    <div class="ordo-section">
      <div class="ordo-section-header">
        <span class="title">{{ t('step.assignments') }}</span>
        <button class="ordo-btn-text" :disabled="disabled" @click="addAssignment">
          <OrdoIcon name="add" :size="12" /> {{ t('step.addAssignment') }}
        </button>
      </div>

      <div class="ordo-table-container">
        <table class="ordo-data-table" v-if="modelValue.assignments?.length">
          <thead>
            <tr>
              <th width="30%">Name</th>
              <th width="60%">Value</th>
              <th width="10%"></th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(assign, index) in modelValue.assignments" :key="index">
              <td>
                <input
                  :value="assign.name"
                  :disabled="disabled"
                  class="ordo-input-clean"
                  @input="
                    updateAssignment(index, { name: ($event.target as HTMLInputElement).value })
                  "
                />
              </td>
              <td>
                <OrdoExpressionInput
                  :model-value="getExprValue(assign.value)"
                  :suggestions="suggestions"
                  :disabled="disabled"
                  @update:model-value="updateAssignment(index, { value: updateExprValue($event) })"
                />
              </td>
              <td class="center">
                <button class="ordo-btn-icon danger" @click="removeAssignment(index)">
                  <OrdoIcon name="delete" :size="14" />
                </button>
              </td>
            </tr>
          </tbody>
        </table>
        <div v-else class="ordo-empty-state">No variable assignments.</div>
      </div>
    </div>

    <!-- Logging -->
    <div class="ordo-section">
      <div class="ordo-section-header">
        <span class="title">{{ t('step.logging') }}</span>
      </div>
      <div class="ordo-logging-row">
        <select
          :value="modelValue.logging?.level || ''"
          :disabled="disabled"
          class="ordo-input-base level-select"
          @change="updateLogging({ level: ($event.target as HTMLSelectElement).value as any })"
        >
          <option value="">None</option>
          <option value="debug">Debug</option>
          <option value="info">Info</option>
          <option value="warn">Warn</option>
          <option value="error">Error</option>
        </select>
        <OrdoExpressionInput
          :model-value="getExprValue(modelValue.logging?.message)"
          :suggestions="suggestions"
          :disabled="disabled"
          placeholder="Log message..."
          @update:model-value="updateLogging({ message: updateExprValue($event) })"
        />
      </div>
    </div>

    <!-- Next Step -->
    <div class="ordo-form-row">
      <div class="ordo-form-group full">
        <label>{{ t('step.nextStep') }}</label>
        <select
          :value="modelValue.nextStepId"
          :disabled="disabled"
          class="ordo-input-base"
          @change="updateNextStep"
        >
          <option value="">-- End Flow --</option>
          <option v-for="opt in stepOptions" :key="opt.value" :value="opt.value">
            {{ opt.label }}
          </option>
        </select>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* Reuse styles from Decision Editor or global theme */
.ordo-editor-panel {
  display: flex;
  flex-direction: column;
  gap: var(--ordo-space-md);
  font-size: var(--ordo-font-size-sm);
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

.ordo-form-group.grow {
  flex: 1;
}
.ordo-form-group.full {
  width: 100%;
}

.ordo-form-group label {
  font-size: 11px;
  font-weight: 500;
  color: var(--ordo-text-secondary);
  text-transform: uppercase;
}

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

.ordo-btn-text:hover {
  background: var(--ordo-accent-bg);
}

/* Table Styles */
.ordo-table-container {
  border: 1px solid var(--ordo-border-light);
  border-radius: var(--ordo-radius-md);
  overflow: hidden;
}

.ordo-data-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
}

.ordo-data-table th {
  background: var(--ordo-bg-panel);
  text-align: left;
  padding: 6px 8px;
  font-weight: 600;
  color: var(--ordo-text-secondary);
  border-bottom: 1px solid var(--ordo-border-light);
}

.ordo-data-table td {
  padding: 4px 8px;
  border-bottom: 1px solid var(--ordo-border-light);
  background: var(--ordo-bg-item);
}

.ordo-data-table tr:last-child td {
  border-bottom: none;
}

.ordo-input-clean {
  width: 100%;
  border: none;
  background: transparent;
  font-family: var(--ordo-font-mono);
  color: var(--ordo-variable);
}

.ordo-input-clean:focus {
  outline: none;
  background: var(--ordo-bg-input);
}

.ordo-empty-state {
  padding: 12px;
  text-align: center;
  color: var(--ordo-text-tertiary);
  font-style: italic;
  background: var(--ordo-bg-item);
}

.ordo-logging-row {
  display: flex;
  gap: 8px;
}

.level-select {
  width: 80px;
}

.center {
  text-align: center;
}
.ordo-btn-icon.danger {
  color: var(--ordo-error);
}
.ordo-btn-icon.danger:hover {
  background: var(--ordo-error-bg);
}
</style>
