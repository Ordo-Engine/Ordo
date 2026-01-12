<script setup lang="ts">
/**
 * OrdoConditionBuilder - Visual condition builder
 * 可视化条件构建器
 */
import { computed } from 'vue';
import type { Condition, SimpleCondition, LogicalCondition } from '@ordo/editor-core';
import { Condition as ConditionFactory, Expr } from '@ordo/editor-core';
import OrdoExpressionInput from './OrdoExpressionInput.vue';
import type { FieldSuggestion } from './OrdoExpressionInput.vue';

export interface Props {
  /** Current condition */
  modelValue: Condition;
  /** Available field suggestions */
  suggestions?: FieldSuggestion[];
  /** Whether the builder is disabled */
  disabled?: boolean;
  /** Whether to allow nested conditions */
  allowNested?: boolean;
  /** Maximum nesting depth */
  maxDepth?: number;
}

const props = withDefaults(defineProps<Props>(), {
  suggestions: () => [],
  disabled: false,
  allowNested: true,
  maxDepth: 3,
});

const emit = defineEmits<{
  'update:modelValue': [value: Condition];
  change: [value: Condition];
}>();

// Available operators for simple conditions
const operators: { value: SimpleCondition['operator']; label: string }[] = [
  { value: 'eq', label: '==' },
  { value: 'ne', label: '!=' },
  { value: 'gt', label: '>' },
  { value: 'gte', label: '>=' },
  { value: 'lt', label: '<' },
  { value: 'lte', label: '<=' },
  { value: 'in', label: 'in' },
  { value: 'contains', label: 'contains' },
  { value: 'startsWith', label: 'starts with' },
  { value: 'endsWith', label: 'ends with' },
];

// Check if condition is simple
const isSimple = computed(() => props.modelValue.type === 'simple');
const isLogical = computed(() => props.modelValue.type === 'logical');
const isExpression = computed(() => props.modelValue.type === 'expression');

// Get simple condition parts
const simpleCondition = computed(() => {
  if (props.modelValue.type === 'simple') {
    return props.modelValue as SimpleCondition;
  }
  return null;
});

// Get logical condition parts
const logicalCondition = computed(() => {
  if (props.modelValue.type === 'logical') {
    return props.modelValue as LogicalCondition;
  }
  return null;
});

// Update functions
function updateConditionType(type: 'simple' | 'logical' | 'expression') {
  let newCondition: Condition;

  switch (type) {
    case 'simple':
      newCondition = ConditionFactory.simple(Expr.variable('$.field'), 'eq', Expr.string(''));
      break;
    case 'logical':
      newCondition = ConditionFactory.and(
        ConditionFactory.simple(Expr.variable('$.field'), 'eq', Expr.string(''))
      );
      break;
    case 'expression':
      newCondition = ConditionFactory.expression('');
      break;
    default:
      return;
  }

  emit('update:modelValue', newCondition);
  emit('change', newCondition);
}

function updateSimpleLeft(value: string) {
  if (!simpleCondition.value) return;

  const newCondition: SimpleCondition = {
    ...simpleCondition.value,
    left: Expr.variable(value),
  };
  emit('update:modelValue', newCondition);
  emit('change', newCondition);
}

function updateSimpleOperator(op: SimpleCondition['operator']) {
  if (!simpleCondition.value) return;

  const newCondition: SimpleCondition = {
    ...simpleCondition.value,
    operator: op,
  };
  emit('update:modelValue', newCondition);
  emit('change', newCondition);
}

function updateSimpleRight(value: string) {
  if (!simpleCondition.value) return;

  // Try to parse as literal
  let rightExpr = Expr.string(value);
  if (value.startsWith('$.') || value.startsWith('$')) {
    rightExpr = Expr.variable(value);
  } else if (value === 'true') {
    rightExpr = Expr.boolean(true);
  } else if (value === 'false') {
    rightExpr = Expr.boolean(false);
  } else if (value === 'null') {
    rightExpr = Expr.null();
  } else if (!isNaN(Number(value)) && value !== '') {
    rightExpr = Expr.number(Number(value));
  }

  const newCondition: SimpleCondition = {
    ...simpleCondition.value,
    right: rightExpr,
  };
  emit('update:modelValue', newCondition);
  emit('change', newCondition);
}

function updateLogicalOperator(op: 'and' | 'or') {
  if (!logicalCondition.value) return;

  const newCondition: LogicalCondition = {
    ...logicalCondition.value,
    operator: op,
  };
  emit('update:modelValue', newCondition);
  emit('change', newCondition);
}

function updateLogicalCondition(index: number, condition: Condition) {
  if (!logicalCondition.value) return;

  const newConditions = [...logicalCondition.value.conditions];
  newConditions[index] = condition;

  const newCondition: LogicalCondition = {
    ...logicalCondition.value,
    conditions: newConditions,
  };
  emit('update:modelValue', newCondition);
  emit('change', newCondition);
}

function addLogicalCondition() {
  if (!logicalCondition.value) return;

  const newCondition: LogicalCondition = {
    ...logicalCondition.value,
    conditions: [
      ...logicalCondition.value.conditions,
      ConditionFactory.simple(Expr.variable('$.field'), 'eq', Expr.string('')),
    ],
  };
  emit('update:modelValue', newCondition);
  emit('change', newCondition);
}

function removeLogicalCondition(index: number) {
  if (!logicalCondition.value) return;

  const newConditions = logicalCondition.value.conditions.filter((_, i) => i !== index);

  // If only one condition left, unwrap it
  if (newConditions.length === 1) {
    emit('update:modelValue', newConditions[0]);
    emit('change', newConditions[0]);
    return;
  }

  const newCondition: LogicalCondition = {
    ...logicalCondition.value,
    conditions: newConditions,
  };
  emit('update:modelValue', newCondition);
  emit('change', newCondition);
}

function updateExpression(value: string) {
  const newCondition = ConditionFactory.expression(value);
  emit('update:modelValue', newCondition);
  emit('change', newCondition);
}

// Helper to get expression value for display
function getExprValue(expr: { type: string; path?: string; value?: unknown }): string {
  if (expr.type === 'variable' && expr.path) {
    return expr.path;
  }
  if (expr.type === 'literal') {
    if (expr.value === null) return 'null';
    if (typeof expr.value === 'string') return expr.value;
    return String(expr.value);
  }
  return '';
}
</script>

<template>
  <div class="ordo-condition-builder" :class="{ disabled }">
    <!-- Type Switcher (only show if not nested too deep to avoid clutter) -->
    <div class="ordo-condition-builder__header">
      <div class="ordo-condition-builder__type-tabs">
        <button
          type="button"
          :class="{ active: isSimple }"
          :disabled="disabled"
          @click="updateConditionType('simple')"
        >
          Simple
        </button>
        <button
          v-if="allowNested"
          type="button"
          :class="{ active: isLogical }"
          :disabled="disabled"
          @click="updateConditionType('logical')"
        >
          Group
        </button>
        <button
          type="button"
          :class="{ active: isExpression }"
          :disabled="disabled"
          @click="updateConditionType('expression')"
        >
          Expr
        </button>
      </div>
    </div>

    <!-- Simple condition -->
    <div v-if="isSimple && simpleCondition" class="ordo-condition-builder__simple">
      <div class="ordo-condition-builder__row">
        <OrdoExpressionInput
          :model-value="getExprValue(simpleCondition.left)"
          :suggestions="suggestions"
          :disabled="disabled"
          placeholder="Field (e.g. $.age)"
          class="ordo-condition-builder__input-left"
          @update:model-value="updateSimpleLeft"
        />

        <div class="ordo-condition-builder__operator-wrapper">
          <select
            :value="simpleCondition.operator"
            :disabled="disabled"
            class="ordo-condition-builder__operator"
            @change="
              updateSimpleOperator(
                ($event.target as HTMLSelectElement).value as SimpleCondition['operator']
              )
            "
          >
            <option v-for="op in operators" :key="op.value" :value="op.value">
              {{ op.label }}
            </option>
          </select>
        </div>

        <OrdoExpressionInput
          :model-value="getExprValue(simpleCondition.right)"
          :suggestions="suggestions"
          :disabled="disabled"
          placeholder="Value"
          class="ordo-condition-builder__input-right"
          @update:model-value="updateSimpleRight"
        />
      </div>
    </div>

    <!-- Logical condition (AND/OR group) -->
    <div v-if="isLogical && logicalCondition" class="ordo-condition-builder__logical">
      <div class="ordo-condition-builder__logical-bar">
        <select
          :value="logicalCondition.operator"
          :disabled="disabled"
          class="ordo-condition-builder__logical-select"
          :class="logicalCondition.operator"
          @change="
            updateLogicalOperator(($event.target as HTMLSelectElement).value as 'and' | 'or')
          "
        >
          <option value="and">AND (All match)</option>
          <option value="or">OR (Any match)</option>
        </select>
        <div class="ordo-condition-builder__logical-line"></div>
      </div>

      <div class="ordo-condition-builder__logical-content">
        <div
          v-for="(cond, index) in logicalCondition.conditions"
          :key="index"
          class="ordo-condition-builder__logical-item"
        >
          <OrdoConditionBuilder
            :model-value="cond"
            :suggestions="suggestions"
            :disabled="disabled"
            :allow-nested="allowNested && maxDepth > 1"
            :max-depth="maxDepth - 1"
            @update:model-value="updateLogicalCondition(index, $event)"
          />
          <button
            v-if="logicalCondition.conditions.length > 1"
            type="button"
            class="ordo-condition-builder__remove-btn"
            :disabled="disabled"
            title="Remove condition"
            @click="removeLogicalCondition(index)"
          >
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

        <button
          type="button"
          class="ordo-condition-builder__add-btn"
          :disabled="disabled"
          @click="addLogicalCondition"
        >
          + Add Condition
        </button>
      </div>
    </div>

    <!-- Expression condition -->
    <div v-if="isExpression" class="ordo-condition-builder__expression">
      <OrdoExpressionInput
        :model-value="(modelValue as { expression: string }).expression || ''"
        :suggestions="suggestions"
        :disabled="disabled"
        :multiline="true"
        :min-rows="2"
        placeholder="Enter raw expression (e.g. $.age >= 18 && $.status == 'active')"
        @update:model-value="updateExpression"
      />
    </div>
  </div>
</template>

<style scoped>
.ordo-condition-builder {
  display: flex;
  flex-direction: column;
  gap: var(--ordo-space-sm);
  width: 100%;
}

.ordo-condition-builder__header {
  display: flex;
  justify-content: flex-end;
}

.ordo-condition-builder__type-tabs {
  display: inline-flex;
  background: var(--ordo-bg-tertiary);
  padding: 2px;
  border-radius: var(--ordo-radius-md);
}

.ordo-condition-builder__type-tabs button {
  padding: 2px 8px;
  border: none;
  background: transparent;
  color: var(--ordo-text-secondary);
  font-size: 10px;
  font-weight: 500;
  border-radius: var(--ordo-radius-sm);
  cursor: pointer;
  transition: all 0.15s;
}

.ordo-condition-builder__type-tabs button:hover:not(:disabled) {
  color: var(--ordo-text-primary);
}

.ordo-condition-builder__type-tabs button.active {
  background: var(--ordo-bg-card);
  color: var(--ordo-primary-600);
  box-shadow: var(--ordo-shadow-sm);
}

.ordo-condition-builder__simple {
  width: 100%;
}

.ordo-condition-builder__row {
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  gap: var(--ordo-space-sm);
  align-items: center;
}

.ordo-condition-builder__operator-wrapper {
  position: relative;
}

.ordo-condition-builder__operator {
  appearance: none;
  padding: 0 24px 0 8px;
  height: 32px;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  background: var(--ordo-bg-input);
  font-family: var(--ordo-font-mono);
  font-size: 12px;
  color: var(--ordo-primary-600);
  font-weight: 600;
  cursor: pointer;
  text-align: center;
}

.ordo-condition-builder__operator:focus {
  outline: none;
  border-color: var(--ordo-primary-500);
  box-shadow: var(--ordo-focus-ring);
}

/* Logical Group Styles */
.ordo-condition-builder__logical {
  display: flex;
  flex-direction: column;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-lg);
  overflow: hidden;
  background: var(--ordo-bg-secondary);
}

.ordo-condition-builder__logical-bar {
  display: flex;
  align-items: center;
  padding: var(--ordo-space-xs) var(--ordo-space-sm);
  background: var(--ordo-gray-100);
  border-bottom: 1px solid var(--ordo-border-color);
}

[data-ordo-theme='dark'] .ordo-condition-builder__logical-bar {
  background: var(--ordo-gray-800);
}

.ordo-condition-builder__logical-select {
  appearance: none;
  border: none;
  background: transparent;
  font-size: 11px;
  font-weight: 700;
  cursor: pointer;
  padding-right: 12px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.ordo-condition-builder__logical-select.and {
  color: var(--ordo-primary-600);
}

.ordo-condition-builder__logical-select.or {
  color: var(--ordo-warning);
}

.ordo-condition-builder__logical-line {
  flex: 1;
}

.ordo-condition-builder__logical-content {
  display: flex;
  flex-direction: column;
  gap: var(--ordo-space-md);
  padding: var(--ordo-space-sm);
}

.ordo-condition-builder__logical-item {
  display: flex;
  align-items: flex-start;
  gap: var(--ordo-space-sm);
  position: relative;
}

.ordo-condition-builder__logical-item > :first-child {
  flex: 1;
}

.ordo-condition-builder__remove-btn {
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  color: var(--ordo-gray-400);
  border-radius: var(--ordo-radius-sm);
  cursor: pointer;
  transition: all 0.15s;
  margin-top: 4px; /* Align with inputs */
}

.ordo-condition-builder__remove-btn:hover:not(:disabled) {
  background: var(--ordo-error-bg);
  color: var(--ordo-error);
}

.ordo-condition-builder__add-btn {
  align-self: flex-start;
  padding: 4px 12px;
  border: 1px dashed var(--ordo-gray-300);
  border-radius: var(--ordo-radius-md);
  background: transparent;
  color: var(--ordo-gray-500);
  font-size: 11px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s;
}

.ordo-condition-builder__add-btn:hover:not(:disabled) {
  border-color: var(--ordo-primary-400);
  color: var(--ordo-primary-600);
  background: var(--ordo-primary-50);
}

.ordo-condition-builder.disabled {
  opacity: 0.6;
  pointer-events: none;
}
</style>
