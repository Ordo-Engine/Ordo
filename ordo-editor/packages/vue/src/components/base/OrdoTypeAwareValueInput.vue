<script setup lang="ts">
/**
 * OrdoTypeAwareValueInput - Adaptive value input based on field type.
 * Renders different input controls depending on whether the target field
 * is a string, number, boolean, enum, or field reference.
 * 基于字段类型的自适应值输入
 */
import { computed, ref } from 'vue';
import type { Expr } from '@ordo-engine/editor-core';
import { Expr as ExprFactory } from '@ordo-engine/editor-core';
import type { SchemaContext, SchemaFieldType } from '@ordo-engine/editor-core';
import OrdoSchemaFieldPicker from './OrdoSchemaFieldPicker.vue';

export interface Props {
  /** Current expression value */
  modelValue: Expr;
  /** Target field type */
  fieldType: SchemaFieldType;
  /** Target field path (for hints) */
  fieldPath?: string;
  /** Schema context for field references */
  schemaContext?: SchemaContext;
  /** Enum values when the field has known possible values */
  enumValues?: string[];
  /** Whether to disable the input */
  disabled?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  fieldPath: '',
  schemaContext: undefined,
  enumValues: () => [],
  disabled: false,
});

const emit = defineEmits<{
  'update:modelValue': [value: Expr];
  change: [value: Expr];
}>();

type InputMode = 'literal' | 'field';
const inputMode = ref<InputMode>(props.modelValue.type === 'variable' ? 'field' : 'literal');

// Extract display value from expression
const displayValue = computed(() => {
  const expr = props.modelValue;
  if (expr.type === 'variable') return expr.path;
  if (expr.type === 'literal') {
    if (expr.value === null) return '';
    return String(expr.value);
  }
  return '';
});

const booleanValue = computed(() => {
  if (props.modelValue.type === 'literal') {
    return props.modelValue.value === true;
  }
  return false;
});

const numberValue = computed(() => {
  if (props.modelValue.type === 'literal' && typeof props.modelValue.value === 'number') {
    return props.modelValue.value;
  }
  return 0;
});

// Whether the field has enum values (from props or schema hints)
const hasEnum = computed(() => {
  if (props.enumValues.length > 0) return true;
  if (props.schemaContext && props.fieldPath) {
    const hints = props.schemaContext.getValueHintsForField(props.fieldPath);
    return hints.length > 0 && props.fieldType === 'string';
  }
  return false;
});

const effectiveEnumValues = computed(() => {
  if (props.enumValues.length > 0) return props.enumValues;
  if (props.schemaContext && props.fieldPath) {
    return props.schemaContext.getValueHintsForField(props.fieldPath).map((h) => String(h.value));
  }
  return [];
});

function emitExpr(expr: Expr) {
  emit('update:modelValue', expr);
  emit('change', expr);
}

function handleStringInput(event: Event) {
  const value = (event.target as HTMLInputElement).value;
  emitExpr(ExprFactory.string(value));
}

function handleNumberInput(event: Event) {
  const value = (event.target as HTMLInputElement).value;
  const num = parseFloat(value);
  if (!isNaN(num)) {
    emitExpr(ExprFactory.number(num));
  }
}

function handleBooleanToggle() {
  const current = props.modelValue.type === 'literal' && props.modelValue.value === true;
  emitExpr(ExprFactory.boolean(!current));
}

function handleEnumSelect(event: Event) {
  const value = (event.target as HTMLSelectElement).value;
  emitExpr(ExprFactory.string(value));
}

function handleFieldRefChange(path: string) {
  emitExpr(ExprFactory.variable(path.startsWith('$.') ? path : `$.${path}`));
}

function switchMode(mode: InputMode) {
  inputMode.value = mode;
  if (mode === 'literal') {
    // Reset to a sensible default for the field type
    switch (props.fieldType) {
      case 'string':
        emitExpr(ExprFactory.string(''));
        break;
      case 'number':
        emitExpr(ExprFactory.number(0));
        break;
      case 'boolean':
        emitExpr(ExprFactory.boolean(false));
        break;
      default:
        emitExpr(ExprFactory.string(''));
    }
  } else {
    emitExpr(ExprFactory.variable('$.'));
  }
}
</script>

<template>
  <div class="ordo-type-value-input" :class="{ disabled }">
    <!-- Mode toggle (literal vs field reference) -->
    <div v-if="schemaContext && fieldType !== 'boolean'" class="ordo-type-value-input__mode-toggle">
      <button
        type="button"
        :class="{ active: inputMode === 'literal' }"
        :disabled="disabled"
        title="Literal value"
        @click="switchMode('literal')"
      >
        Val
      </button>
      <button
        type="button"
        :class="{ active: inputMode === 'field' }"
        :disabled="disabled"
        title="Field reference"
        @click="switchMode('field')"
      >
        Ref
      </button>
    </div>

    <!-- Field reference mode -->
    <div v-if="inputMode === 'field' && schemaContext" class="ordo-type-value-input__field-ref">
      <OrdoSchemaFieldPicker
        :model-value="displayValue.replace(/^\$\./, '')"
        :schema-context="schemaContext"
        :disabled="disabled"
        placeholder="Select field..."
        @update:model-value="handleFieldRefChange"
      />
    </div>

    <!-- Boolean toggle -->
    <div v-else-if="fieldType === 'boolean'" class="ordo-type-value-input__boolean">
      <button
        type="button"
        class="ordo-type-value-input__bool-toggle"
        :class="{ on: booleanValue }"
        :disabled="disabled"
        @click="handleBooleanToggle"
      >
        <span class="ordo-type-value-input__bool-knob" />
      </button>
      <span class="ordo-type-value-input__bool-label">{{ booleanValue ? 'true' : 'false' }}</span>
    </div>

    <!-- Enum select -->
    <div v-else-if="hasEnum && inputMode === 'literal'" class="ordo-type-value-input__enum">
      <select
        :value="displayValue"
        :disabled="disabled"
        class="ordo-type-value-input__select"
        @change="handleEnumSelect"
      >
        <option value="">-- select --</option>
        <option v-for="val in effectiveEnumValues" :key="val" :value="val">
          {{ val }}
        </option>
      </select>
    </div>

    <!-- Number input -->
    <div
      v-else-if="fieldType === 'number' && inputMode === 'literal'"
      class="ordo-type-value-input__number"
    >
      <input
        type="number"
        :value="numberValue"
        :disabled="disabled"
        class="ordo-type-value-input__input"
        step="any"
        @input="handleNumberInput"
      />
    </div>

    <!-- String input (default) -->
    <div v-else class="ordo-type-value-input__string">
      <input
        type="text"
        :value="displayValue"
        :disabled="disabled"
        class="ordo-type-value-input__input"
        placeholder="Enter value..."
        @input="handleStringInput"
      />
    </div>
  </div>
</template>

<style scoped>
.ordo-type-value-input {
  display: flex;
  align-items: center;
  gap: 4px;
  flex: 1;
  min-width: 0;
}

.ordo-type-value-input.disabled {
  opacity: 0.6;
  pointer-events: none;
}

/* Mode toggle */
.ordo-type-value-input__mode-toggle {
  display: inline-flex;
  background: var(--ordo-bg-tertiary);
  padding: 1px;
  border-radius: var(--ordo-radius-sm);
  flex-shrink: 0;
}

.ordo-type-value-input__mode-toggle button {
  padding: 2px 6px;
  border: none;
  background: transparent;
  color: var(--ordo-text-tertiary);
  font-size: 9px;
  font-weight: 600;
  border-radius: 2px;
  cursor: pointer;
  transition: all 0.15s;
  text-transform: uppercase;
}

.ordo-type-value-input__mode-toggle button:hover:not(:disabled) {
  color: var(--ordo-text-secondary);
}

.ordo-type-value-input__mode-toggle button.active {
  background: var(--ordo-bg-card);
  color: var(--ordo-primary-600);
  box-shadow: var(--ordo-shadow-sm);
}

/* Field ref */
.ordo-type-value-input__field-ref {
  flex: 1;
  min-width: 0;
  display: flex;
}

/* Boolean */
.ordo-type-value-input__boolean {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ordo-type-value-input__bool-toggle {
  position: relative;
  width: 36px;
  height: 20px;
  border: none;
  border-radius: 10px;
  background: var(--ordo-gray-300);
  cursor: pointer;
  transition: background 0.2s;
  padding: 0;
}

.ordo-type-value-input__bool-toggle.on {
  background: var(--ordo-primary-500);
}

.ordo-type-value-input__bool-knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: white;
  transition: transform 0.2s;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
}

.ordo-type-value-input__bool-toggle.on .ordo-type-value-input__bool-knob {
  transform: translateX(16px);
}

.ordo-type-value-input__bool-label {
  font-family: var(--ordo-font-mono);
  font-size: 12px;
  font-weight: 600;
  color: var(--ordo-text-secondary);
}

/* Enum select */
.ordo-type-value-input__enum {
  flex: 1;
  min-width: 0;
}

.ordo-type-value-input__select {
  width: 100%;
  height: 32px;
  padding: 0 24px 0 8px;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  background: var(--ordo-bg-input);
  font-size: 12px;
  color: var(--ordo-text-primary);
  cursor: pointer;
  appearance: none;
}

.ordo-type-value-input__select:focus {
  outline: none;
  border-color: var(--ordo-primary-500);
  box-shadow: var(--ordo-focus-ring);
}

/* Number & String inputs */
.ordo-type-value-input__number,
.ordo-type-value-input__string {
  flex: 1;
  min-width: 0;
}

.ordo-type-value-input__input {
  width: 100%;
  height: 32px;
  padding: 0 8px;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  background: var(--ordo-bg-input);
  font-size: 12px;
  color: var(--ordo-text-primary);
}

.ordo-type-value-input__input[type='number'] {
  font-family: var(--ordo-font-mono);
}

.ordo-type-value-input__input:focus {
  outline: none;
  border-color: var(--ordo-primary-500);
  box-shadow: var(--ordo-focus-ring);
}

.ordo-type-value-input__input::placeholder {
  color: var(--ordo-text-tertiary);
}
</style>
