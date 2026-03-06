<script setup lang="ts">
/**
 * OrdoTableCellEditor - Inline cell editor for decision table cells
 * 决策表单元格编辑器
 */
import { computed, ref, watch, nextTick, onMounted } from 'vue';
import type { CellValue, SchemaFieldType } from '@ordo-engine/editor-core';
import { useI18n } from '../../locale';

export interface Props {
  modelValue: CellValue;
  fieldType: SchemaFieldType;
  disabled?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  disabled: false,
});

const emit = defineEmits<{
  'update:modelValue': [value: CellValue];
  change: [value: CellValue];
  confirm: [];
  cancel: [];
}>();

const { t } = useI18n();

const cellType = ref<CellValue['type']>(props.modelValue.type);
const exactValue = ref('');
const rangeMin = ref('');
const rangeMax = ref('');
const rangeMinInclusive = ref(true);
const rangeMaxInclusive = ref(false);
const listValue = ref('');
const exprValue = ref('');
const inputRef = ref<HTMLInputElement | HTMLTextAreaElement | null>(null);

const cellTypeOptions = computed(() => [
  { value: 'exact' as const, label: t('table.cellExact') },
  { value: 'range' as const, label: t('table.cellRange'), hidden: !isNumericType.value },
  { value: 'in' as const, label: t('table.cellList') },
  { value: 'any' as const, label: t('table.cellAny') },
  { value: 'expression' as const, label: t('table.cellExpression') },
]);

const visibleCellTypes = computed(() => cellTypeOptions.value.filter((o) => !o.hidden));

const isNumericType = computed(() => props.fieldType === 'number');
const isBooleanType = computed(() => props.fieldType === 'boolean');

function syncFromModel(cell: CellValue) {
  cellType.value = cell.type;
  switch (cell.type) {
    case 'exact':
      exactValue.value = String(cell.value);
      break;
    case 'range':
      rangeMin.value = cell.min !== undefined ? String(cell.min) : '';
      rangeMax.value = cell.max !== undefined ? String(cell.max) : '';
      rangeMinInclusive.value = cell.minInclusive ?? true;
      rangeMaxInclusive.value = cell.maxInclusive ?? false;
      break;
    case 'in':
      listValue.value = cell.values.map((v) => String(v)).join(', ');
      break;
    case 'expression':
      exprValue.value = cell.expr;
      break;
  }
}

watch(() => props.modelValue, syncFromModel, { immediate: true });

function buildCellValue(): CellValue {
  switch (cellType.value) {
    case 'exact': {
      const raw = exactValue.value.trim();
      if (isBooleanType.value) {
        return { type: 'exact', value: raw.toLowerCase() === 'true' };
      }
      if (isNumericType.value && raw !== '' && !isNaN(Number(raw))) {
        return { type: 'exact', value: Number(raw) };
      }
      return { type: 'exact', value: raw };
    }
    case 'range': {
      const min = rangeMin.value.trim() !== '' ? Number(rangeMin.value) : undefined;
      const max = rangeMax.value.trim() !== '' ? Number(rangeMax.value) : undefined;
      return {
        type: 'range',
        min: min !== undefined && !isNaN(min) ? min : undefined,
        max: max !== undefined && !isNaN(max) ? max : undefined,
        minInclusive: rangeMinInclusive.value,
        maxInclusive: rangeMaxInclusive.value,
      };
    }
    case 'in': {
      const parts = listValue.value
        .split(',')
        .map((s) => s.trim())
        .filter((s) => s !== '');
      const values = parts.map((s) => {
        if (isNumericType.value && !isNaN(Number(s))) return Number(s);
        return s;
      });
      return { type: 'in', values };
    }
    case 'any':
      return { type: 'any' };
    case 'expression':
      return { type: 'expression', expr: exprValue.value };
    default:
      return { type: 'any' };
  }
}

function emitUpdate() {
  const val = buildCellValue();
  emit('update:modelValue', val);
  emit('change', val);
}

function onTypeChange(newType: CellValue['type']) {
  cellType.value = newType;
  if (newType === 'any') {
    emitUpdate();
  }
  nextTick(() => inputRef.value?.focus());
}

function onKeyDown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    emitUpdate();
    emit('confirm');
  } else if (e.key === 'Escape') {
    e.preventDefault();
    emit('cancel');
  } else if (e.key === 'Tab') {
    emitUpdate();
    emit('confirm');
  }
}

function toggleBoolean() {
  if (exactValue.value.toLowerCase() === 'true') {
    exactValue.value = 'false';
  } else {
    exactValue.value = 'true';
  }
  emitUpdate();
}

function toggleBound(which: 'min' | 'max') {
  if (which === 'min') rangeMinInclusive.value = !rangeMinInclusive.value;
  else rangeMaxInclusive.value = !rangeMaxInclusive.value;
  emitUpdate();
}

onMounted(() => {
  nextTick(() => inputRef.value?.focus());
});
</script>

<template>
  <div class="ordo-cell-editor" :class="{ disabled }" @keydown="onKeyDown">
    <!-- Cell type selector -->
    <div class="ordo-cell-editor__type-bar">
      <button
        v-for="opt in visibleCellTypes"
        :key="opt.value"
        type="button"
        class="ordo-cell-editor__type-btn"
        :class="{ active: cellType === opt.value }"
        :disabled="disabled"
        @click="onTypeChange(opt.value)"
      >
        {{ opt.label }}
      </button>
    </div>

    <!-- Exact value input -->
    <div v-if="cellType === 'exact'" class="ordo-cell-editor__body">
      <template v-if="isBooleanType">
        <button
          type="button"
          class="ordo-cell-editor__toggle"
          :class="{ active: exactValue.toLowerCase() === 'true' }"
          :disabled="disabled"
          @click="toggleBoolean"
        >
          <span class="ordo-cell-editor__toggle-track">
            <span class="ordo-cell-editor__toggle-thumb" />
          </span>
          <span class="ordo-cell-editor__toggle-label">
            {{ exactValue.toLowerCase() === 'true' ? 'TRUE' : 'FALSE' }}
          </span>
        </button>
      </template>
      <template v-else>
        <input
          ref="inputRef"
          v-model="exactValue"
          :type="isNumericType ? 'number' : 'text'"
          class="ordo-cell-editor__input"
          :disabled="disabled"
          placeholder="Value"
          @blur="emitUpdate"
        />
      </template>
    </div>

    <!-- Range input -->
    <div v-else-if="cellType === 'range'" class="ordo-cell-editor__body ordo-cell-editor__range">
      <button
        type="button"
        class="ordo-cell-editor__bound-btn"
        :title="rangeMinInclusive ? 'Inclusive [' : 'Exclusive ('"
        @click="toggleBound('min')"
      >
        {{ rangeMinInclusive ? '[' : '(' }}
      </button>
      <input
        ref="inputRef"
        v-model="rangeMin"
        type="number"
        class="ordo-cell-editor__input ordo-cell-editor__input--range"
        :disabled="disabled"
        placeholder="min"
        @blur="emitUpdate"
      />
      <span class="ordo-cell-editor__range-sep">,</span>
      <input
        v-model="rangeMax"
        type="number"
        class="ordo-cell-editor__input ordo-cell-editor__input--range"
        :disabled="disabled"
        placeholder="max"
        @blur="emitUpdate"
      />
      <button
        type="button"
        class="ordo-cell-editor__bound-btn"
        :title="rangeMaxInclusive ? 'Inclusive ]' : 'Exclusive )'"
        @click="toggleBound('max')"
      >
        {{ rangeMaxInclusive ? ']' : ')' }}
      </button>
    </div>

    <!-- List (in) input -->
    <div v-else-if="cellType === 'in'" class="ordo-cell-editor__body">
      <input
        ref="inputRef"
        v-model="listValue"
        type="text"
        class="ordo-cell-editor__input"
        :disabled="disabled"
        placeholder="val1, val2, val3"
        @blur="emitUpdate"
      />
    </div>

    <!-- Any (wildcard) -->
    <div v-else-if="cellType === 'any'" class="ordo-cell-editor__body ordo-cell-editor__any">
      <span class="ordo-cell-editor__wildcard">*</span>
    </div>

    <!-- Expression -->
    <div v-else-if="cellType === 'expression'" class="ordo-cell-editor__body">
      <textarea
        ref="inputRef"
        v-model="exprValue"
        class="ordo-cell-editor__textarea"
        :disabled="disabled"
        placeholder="$.field > 10 && $.status == 'active'"
        rows="2"
        @blur="emitUpdate"
      />
    </div>
  </div>
</template>

<style scoped>
.ordo-cell-editor {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 160px;
  padding: 6px;
  background: var(--ordo-bg-card);
  border: 1px solid var(--ordo-primary-400);
  border-radius: var(--ordo-radius-md);
  box-shadow: var(--ordo-shadow-lg);
}

.ordo-cell-editor.disabled {
  opacity: 0.6;
  pointer-events: none;
}

.ordo-cell-editor__type-bar {
  display: flex;
  gap: 2px;
  padding: 2px;
  background: var(--ordo-bg-tertiary);
  border-radius: var(--ordo-radius-sm);
}

.ordo-cell-editor__type-btn {
  flex: 1;
  padding: 2px 6px;
  border: none;
  background: transparent;
  color: var(--ordo-text-secondary);
  font-size: 10px;
  font-weight: 500;
  border-radius: var(--ordo-radius-sm);
  cursor: pointer;
  transition: all 0.15s;
  white-space: nowrap;
}

.ordo-cell-editor__type-btn:hover:not(:disabled) {
  color: var(--ordo-text-primary);
  background: var(--ordo-bg-secondary);
}

.ordo-cell-editor__type-btn.active {
  background: var(--ordo-bg-card);
  color: var(--ordo-primary-600);
  box-shadow: var(--ordo-shadow-sm);
}

.ordo-cell-editor__body {
  display: flex;
  align-items: center;
  gap: 4px;
}

.ordo-cell-editor__input {
  width: 100%;
  height: 28px;
  padding: 0 8px;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  font-size: var(--ordo-font-size-sm);
  font-family: var(--ordo-font-mono);
  background: var(--ordo-bg-input);
  color: var(--ordo-text-primary);
  transition: var(--ordo-transition-base);
}

.ordo-cell-editor__input:focus {
  outline: none;
  border-color: var(--ordo-primary-500);
  box-shadow: var(--ordo-focus-ring);
}

.ordo-cell-editor__input--range {
  width: 64px;
  text-align: center;
}

.ordo-cell-editor__textarea {
  width: 100%;
  padding: 4px 8px;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  font-size: var(--ordo-font-size-sm);
  font-family: var(--ordo-font-mono);
  background: var(--ordo-bg-input);
  color: var(--ordo-text-primary);
  resize: vertical;
  min-height: 40px;
  transition: var(--ordo-transition-base);
}

.ordo-cell-editor__textarea:focus {
  outline: none;
  border-color: var(--ordo-primary-500);
  box-shadow: var(--ordo-focus-ring);
}

.ordo-cell-editor__range {
  align-items: center;
}

.ordo-cell-editor__range-sep {
  font-family: var(--ordo-font-mono);
  color: var(--ordo-text-tertiary);
  font-size: 14px;
}

.ordo-cell-editor__bound-btn {
  width: 20px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  background: var(--ordo-bg-secondary);
  color: var(--ordo-primary-600);
  font-family: var(--ordo-font-mono);
  font-size: 14px;
  font-weight: 700;
  cursor: pointer;
  transition: all 0.15s;
}

.ordo-cell-editor__bound-btn:hover {
  background: var(--ordo-primary-50);
  border-color: var(--ordo-primary-400);
}

.ordo-cell-editor__any {
  justify-content: center;
  padding: 4px 0;
}

.ordo-cell-editor__wildcard {
  font-family: var(--ordo-font-mono);
  font-size: 18px;
  font-weight: 700;
  color: var(--ordo-text-tertiary);
}

.ordo-cell-editor__toggle {
  display: inline-flex;
  align-items: center;
  gap: var(--ordo-space-sm);
  padding: 0;
  border: none;
  background: none;
  cursor: pointer;
  user-select: none;
}

.ordo-cell-editor__toggle:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}

.ordo-cell-editor__toggle-track {
  position: relative;
  display: inline-block;
  width: 36px;
  height: 20px;
  border-radius: 99px;
  background: var(--ordo-gray-300);
  transition: var(--ordo-transition-base);
}

.ordo-cell-editor__toggle.active .ordo-cell-editor__toggle-track {
  background: var(--ordo-success);
}

.ordo-cell-editor__toggle-thumb {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: white;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.1);
  transition: transform 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

.ordo-cell-editor__toggle.active .ordo-cell-editor__toggle-thumb {
  transform: translateX(16px);
}

.ordo-cell-editor__toggle-label {
  font-family: var(--ordo-font-mono);
  font-size: 11px;
  font-weight: 600;
  color: var(--ordo-text-secondary);
}
</style>
