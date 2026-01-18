<script setup lang="ts">
/**
 * OrdoTerminalEditor - Terminal step editor (Refactored)
 * 终结步骤编辑器
 */
import type { TerminalStep, OutputField } from '@ordo-engine/editor-core';
import { Expr, generateId } from '@ordo-engine/editor-core';
import OrdoExpressionInput from '../base/OrdoExpressionInput.vue';
import OrdoIcon from '../icons/OrdoIcon.vue';
import { useI18n } from '../../locale';
import type { FieldSuggestion } from '../base/OrdoExpressionInput.vue';

export interface Props {
  /** Terminal step data */
  modelValue: TerminalStep;
  /** Field suggestions for expressions */
  suggestions?: FieldSuggestion[];
  /** Whether the editor is disabled */
  disabled?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  suggestions: () => [],
  disabled: false,
});

const emit = defineEmits<{
  'update:modelValue': [value: TerminalStep];
  change: [value: TerminalStep];
}>();

const { t } = useI18n();

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

function updateCode(event: Event) {
  const target = event.target as HTMLInputElement;
  emit('update:modelValue', { ...props.modelValue, code: target.value });
}

function handleCodeBlur() {
  emit('change', props.modelValue);
}

function updateMessage(value: string) {
  const newStep = { ...props.modelValue, message: value ? Expr.string(value) : undefined };
  emit('update:modelValue', newStep);
  emit('change', newStep);
}

// Output fields
function addOutput() {
  const newOutput: OutputField = {
    name: `field_${generateId('').slice(0, 6)}`,
    value: Expr.string(''),
  };
  const newStep = { ...props.modelValue, output: [...(props.modelValue.output || []), newOutput] };
  emit('update:modelValue', newStep);
  emit('change', newStep);
}

function updateOutput(index: number, output: Partial<OutputField>) {
  const outputs = [...(props.modelValue.output || [])];
  outputs[index] = { ...outputs[index], ...output };
  const newStep = { ...props.modelValue, output: outputs };
  emit('update:modelValue', newStep);
  emit('change', newStep);
}

function removeOutput(index: number) {
  const outputs = (props.modelValue.output || []).filter((_, i) => i !== index);
  const newStep = { ...props.modelValue, output: outputs.length > 0 ? outputs : undefined };
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
  if (val.startsWith('$')) return Expr.variable(val);
  if (!isNaN(Number(val)) && val !== '') return Expr.number(Number(val));
  if (val === 'true') return Expr.boolean(true);
  if (val === 'false') return Expr.boolean(false);
  return Expr.string(val);
}
</script>

<template>
  <div class="ordo-editor-panel terminal">
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

    <!-- Result Code & Message -->
    <div class="ordo-form-row">
      <div class="ordo-form-group" style="width: 150px">
        <label>{{ t('step.resultCode') }}</label>
        <input
          :value="modelValue.code"
          :disabled="disabled"
          placeholder="CODE"
          class="ordo-input-base code-font"
          @input="updateCode"
          @blur="handleCodeBlur"
        />
      </div>
      <div class="ordo-form-group grow">
        <label>{{ t('step.resultMessage') }}</label>
        <OrdoExpressionInput
          :model-value="getExprValue(modelValue.message)"
          :suggestions="suggestions"
          :disabled="disabled"
          placeholder="Message expression..."
          @update:model-value="updateMessage"
        />
      </div>
    </div>

    <!-- Output Fields -->
    <div class="ordo-section">
      <div class="ordo-section-header">
        <span class="title">{{ t('step.outputFields') }}</span>
        <button class="ordo-btn-text" :disabled="disabled" @click="addOutput">
          <OrdoIcon name="add" :size="12" /> {{ t('common.add') }}
        </button>
      </div>

      <div class="ordo-table-container">
        <table class="ordo-data-table" v-if="modelValue.output?.length">
          <thead>
            <tr>
              <th width="30%">Field</th>
              <th width="60%">Value</th>
              <th width="10%"></th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(output, index) in modelValue.output" :key="index">
              <td>
                <input
                  :value="output.name"
                  :disabled="disabled"
                  class="ordo-input-clean"
                  @input="updateOutput(index, { name: ($event.target as HTMLInputElement).value })"
                />
              </td>
              <td>
                <OrdoExpressionInput
                  :model-value="getExprValue(output.value)"
                  :suggestions="suggestions"
                  :disabled="disabled"
                  @update:model-value="updateOutput(index, { value: updateExprValue($event) })"
                />
              </td>
              <td class="center">
                <button class="ordo-btn-icon danger" @click="removeOutput(index)">
                  <OrdoIcon name="delete" :size="14" />
                </button>
              </td>
            </tr>
          </tbody>
        </table>
        <div v-else class="ordo-empty-state">No output fields.</div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* Reuse styles */
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

/* Table */
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
  color: var(--ordo-keyword);
}

.ordo-input-clean:focus {
  outline: none;
  background: var(--ordo-bg-input);
}

.code-font {
  font-family: var(--ordo-font-mono);
  font-weight: 600;
}

.ordo-empty-state {
  padding: 12px;
  text-align: center;
  color: var(--ordo-text-tertiary);
  font-style: italic;
  background: var(--ordo-bg-item);
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
