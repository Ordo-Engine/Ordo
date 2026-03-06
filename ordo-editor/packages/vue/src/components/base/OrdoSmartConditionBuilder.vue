<script setup lang="ts">
/**
 * OrdoSmartConditionBuilder - Schema-driven condition builder that replaces
 * OrdoConditionBuilder when a schema is available.
 *
 * Left side:  OrdoSchemaFieldPicker (dropdown, not free-text)
 * Operator:   Dynamically filtered by selected field type
 * Right side: OrdoTypeAwareValueInput (adapts to field type)
 *
 * Falls back to OrdoConditionBuilder (expression mode) for advanced cases.
 * 基于 Schema 的智能条件构建器
 */
import { computed, ref } from 'vue';
import type {
  Condition,
  SimpleCondition,
  LogicalCondition,
  SchemaContext,
  OperatorInfo,
  SchemaFieldType,
} from '@ordo-engine/editor-core';
import { Condition as ConditionFactory, Expr } from '@ordo-engine/editor-core';
import OrdoSchemaFieldPicker from './OrdoSchemaFieldPicker.vue';
import OrdoTypeAwareValueInput from './OrdoTypeAwareValueInput.vue';
import OrdoConditionBuilder from './OrdoConditionBuilder.vue';
import type { FieldSuggestion } from './OrdoExpressionInput.vue';

export interface Props {
  modelValue: Condition;
  schemaContext: SchemaContext;
  /** Fallback suggestions for expression mode */
  suggestions?: FieldSuggestion[];
  disabled?: boolean;
  allowNested?: boolean;
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

type EditorMode = 'smart' | 'expression';

// Determine initial mode based on condition type
const editorMode = ref<EditorMode>(props.modelValue.type === 'expression' ? 'expression' : 'smart');

const isSimple = computed(() => props.modelValue.type === 'simple');
const isLogical = computed(() => props.modelValue.type === 'logical');

const simpleCondition = computed(() => {
  if (props.modelValue.type === 'simple') return props.modelValue as SimpleCondition;
  return null;
});

const logicalCondition = computed(() => {
  if (props.modelValue.type === 'logical') return props.modelValue as LogicalCondition;
  return null;
});

// Resolve the left-hand field path from the condition
const leftFieldPath = computed(() => {
  if (!simpleCondition.value) return '';
  const left = simpleCondition.value.left;
  if (left.type === 'variable') {
    return left.path.replace(/^\$\./, '');
  }
  return '';
});

// Resolve the field type from schema
const leftFieldType = computed((): SchemaFieldType => {
  if (!leftFieldPath.value) return 'any';
  const field = props.schemaContext.getField(leftFieldPath.value);
  return field?.type ?? 'any';
});

// Get valid operators for the selected field
const availableOperators = computed((): OperatorInfo[] => {
  if (leftFieldPath.value) {
    return props.schemaContext.getOperatorsForField(leftFieldPath.value);
  }
  return props.schemaContext.getOperatorsForType('any');
});

// Check if current operator is valid for the field type, auto-fix if not
const currentOperator = computed(() => {
  if (!simpleCondition.value) return 'eq';
  return simpleCondition.value.operator;
});

// --- Update handlers ---

function emitCondition(condition: Condition) {
  emit('update:modelValue', condition);
  emit('change', condition);
}

function updateLeftField(fieldPath: string) {
  const left = Expr.variable(`$.${fieldPath}`);
  const field = props.schemaContext.getField(fieldPath);
  const fieldType = field?.type ?? 'any';

  // Check if current operator is valid for new field type
  const validOps = props.schemaContext.getOperatorsForType(fieldType as SchemaFieldType);
  let operator = simpleCondition.value?.operator ?? 'eq';
  if (!validOps.find((op) => op.value === operator)) {
    operator = (validOps[0]?.value as SimpleCondition['operator']) ?? 'eq';
  }

  // Reset right side to a type-appropriate default
  let right = simpleCondition.value?.right ?? Expr.string('');
  if (right.type === 'literal') {
    switch (fieldType) {
      case 'number':
        if (typeof right.value !== 'number') right = Expr.number(0);
        break;
      case 'boolean':
        if (typeof right.value !== 'boolean') right = Expr.boolean(false);
        break;
      case 'string':
        if (typeof right.value !== 'string') right = Expr.string('');
        break;
    }
  }

  emitCondition(ConditionFactory.simple(left, operator as SimpleCondition['operator'], right));
}

function updateOperator(opValue: string) {
  if (!simpleCondition.value) return;
  emitCondition(
    ConditionFactory.simple(
      simpleCondition.value.left,
      opValue as SimpleCondition['operator'],
      simpleCondition.value.right
    )
  );
}

function updateRightValue(expr: import('@ordo-engine/editor-core').Expr) {
  if (!simpleCondition.value) return;
  emitCondition(
    ConditionFactory.simple(simpleCondition.value.left, simpleCondition.value.operator, expr)
  );
}

// --- Logical condition handlers ---

function updateLogicalOperator(op: 'and' | 'or') {
  if (!logicalCondition.value) return;
  emitCondition({ ...logicalCondition.value, operator: op });
}

function updateLogicalChild(index: number, condition: Condition) {
  if (!logicalCondition.value) return;
  const newConditions = [...logicalCondition.value.conditions];
  newConditions[index] = condition;
  emitCondition({ ...logicalCondition.value, conditions: newConditions });
}

function addLogicalChild() {
  if (!logicalCondition.value) return;
  emitCondition({
    ...logicalCondition.value,
    conditions: [
      ...logicalCondition.value.conditions,
      ConditionFactory.simple(Expr.variable('$.field'), 'eq', Expr.string('')),
    ],
  });
}

function removeLogicalChild(index: number) {
  if (!logicalCondition.value) return;
  const newConditions = logicalCondition.value.conditions.filter((_, i) => i !== index);
  if (newConditions.length === 1) {
    emitCondition(newConditions[0]);
    return;
  }
  emitCondition({ ...logicalCondition.value, conditions: newConditions });
}

// --- Mode switching ---

function switchToExpression() {
  editorMode.value = 'expression';
  // Convert current condition to expression string if it's simple
  if (simpleCondition.value) {
    const left =
      simpleCondition.value.left.type === 'variable' ? simpleCondition.value.left.path : '';
    const op = availableOperators.value.find((o) => o.value === simpleCondition.value!.operator);
    const right = getExprDisplay(simpleCondition.value.right);
    const exprStr = `${left} ${op?.symbol ?? '=='} ${right}`.trim();
    emitCondition(ConditionFactory.expression(exprStr));
  }
}

function switchToSmart() {
  editorMode.value = 'smart';
  emitCondition(ConditionFactory.simple(Expr.variable('$.field'), 'eq', Expr.string('')));
}

function switchConditionType(type: 'simple' | 'logical') {
  if (type === 'simple') {
    emitCondition(ConditionFactory.simple(Expr.variable('$.field'), 'eq', Expr.string('')));
  } else {
    emitCondition(
      ConditionFactory.and(ConditionFactory.simple(Expr.variable('$.field'), 'eq', Expr.string('')))
    );
  }
}

function getExprDisplay(expr: { type: string; path?: string; value?: unknown }): string {
  if (expr.type === 'variable' && expr.path) return expr.path;
  if (expr.type === 'literal') {
    if (expr.value === null) return 'null';
    if (typeof expr.value === 'string') return `"${expr.value}"`;
    return String(expr.value);
  }
  return '';
}

function handleExpressionFallback(condition: Condition) {
  emitCondition(condition);
}
</script>

<template>
  <div class="ordo-smart-condition" :class="{ disabled }">
    <!-- Mode header -->
    <div class="ordo-smart-condition__header">
      <div class="ordo-smart-condition__type-tabs">
        <template v-if="editorMode === 'smart'">
          <button
            type="button"
            :class="{ active: isSimple }"
            :disabled="disabled"
            @click="switchConditionType('simple')"
          >
            Simple
          </button>
          <button
            v-if="allowNested"
            type="button"
            :class="{ active: isLogical }"
            :disabled="disabled"
            @click="switchConditionType('logical')"
          >
            Group
          </button>
        </template>
        <button
          v-if="editorMode === 'expression'"
          type="button"
          class="active"
          :disabled="disabled"
        >
          Expr
        </button>
      </div>
      <button
        v-if="editorMode === 'smart'"
        type="button"
        class="ordo-smart-condition__switch-btn"
        :disabled="disabled"
        @click="switchToExpression"
      >
        Expression
      </button>
      <button
        v-else
        type="button"
        class="ordo-smart-condition__switch-btn"
        :disabled="disabled"
        @click="switchToSmart"
      >
        Smart
      </button>
    </div>

    <!-- Smart Simple Mode -->
    <div
      v-if="editorMode === 'smart' && isSimple && simpleCondition"
      class="ordo-smart-condition__simple"
    >
      <div class="ordo-smart-condition__row">
        <!-- Left: Field picker -->
        <OrdoSchemaFieldPicker
          :model-value="leftFieldPath"
          :schema-context="schemaContext"
          :disabled="disabled"
          placeholder="Select field..."
          class="ordo-smart-condition__field"
          @update:model-value="updateLeftField"
        />

        <!-- Operator -->
        <div class="ordo-smart-condition__operator-wrapper">
          <select
            :value="currentOperator"
            :disabled="disabled"
            class="ordo-smart-condition__operator"
            @change="updateOperator(($event.target as HTMLSelectElement).value)"
          >
            <option v-for="op in availableOperators" :key="op.value" :value="op.value">
              {{ op.symbol }}
            </option>
          </select>
        </div>

        <!-- Right: Type-aware value input -->
        <OrdoTypeAwareValueInput
          :model-value="simpleCondition.right"
          :field-type="leftFieldType"
          :field-path="leftFieldPath"
          :schema-context="schemaContext"
          :disabled="disabled"
          class="ordo-smart-condition__value"
          @update:model-value="updateRightValue"
        />
      </div>
    </div>

    <!-- Smart Logical Mode -->
    <div
      v-if="editorMode === 'smart' && isLogical && logicalCondition"
      class="ordo-smart-condition__logical"
    >
      <div class="ordo-smart-condition__logical-bar">
        <select
          :value="logicalCondition.operator"
          :disabled="disabled"
          class="ordo-smart-condition__logical-select"
          :class="logicalCondition.operator"
          @change="
            updateLogicalOperator(($event.target as HTMLSelectElement).value as 'and' | 'or')
          "
        >
          <option value="and">AND (All match)</option>
          <option value="or">OR (Any match)</option>
        </select>
        <div class="ordo-smart-condition__logical-line" />
      </div>

      <div class="ordo-smart-condition__logical-content">
        <div
          v-for="(cond, index) in logicalCondition.conditions"
          :key="index"
          class="ordo-smart-condition__logical-item"
        >
          <OrdoSmartConditionBuilder
            :model-value="cond"
            :schema-context="schemaContext"
            :suggestions="suggestions"
            :disabled="disabled"
            :allow-nested="allowNested && maxDepth > 1"
            :max-depth="maxDepth - 1"
            @update:model-value="updateLogicalChild(index, $event)"
          />
          <button
            v-if="logicalCondition.conditions.length > 1"
            type="button"
            class="ordo-smart-condition__remove-btn"
            :disabled="disabled"
            title="Remove condition"
            @click="removeLogicalChild(index)"
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>

        <button
          type="button"
          class="ordo-smart-condition__add-btn"
          :disabled="disabled"
          @click="addLogicalChild"
        >
          + Add Condition
        </button>
      </div>
    </div>

    <!-- Expression fallback -->
    <div v-if="editorMode === 'expression'" class="ordo-smart-condition__expression">
      <OrdoConditionBuilder
        :model-value="modelValue"
        :suggestions="suggestions"
        :disabled="disabled"
        :allow-nested="false"
        @update:model-value="handleExpressionFallback"
      />
    </div>
  </div>
</template>

<style scoped>
.ordo-smart-condition {
  display: flex;
  flex-direction: column;
  gap: var(--ordo-space-sm);
  width: 100%;
}

.ordo-smart-condition.disabled {
  opacity: 0.6;
  pointer-events: none;
}

/* Header */
.ordo-smart-condition__header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.ordo-smart-condition__type-tabs {
  display: inline-flex;
  background: var(--ordo-bg-tertiary);
  padding: 2px;
  border-radius: var(--ordo-radius-md);
}

.ordo-smart-condition__type-tabs button {
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

.ordo-smart-condition__type-tabs button:hover:not(:disabled) {
  color: var(--ordo-text-primary);
}

.ordo-smart-condition__type-tabs button.active {
  background: var(--ordo-bg-card);
  color: var(--ordo-primary-600);
  box-shadow: var(--ordo-shadow-sm);
}

.ordo-smart-condition__switch-btn {
  padding: 2px 8px;
  border: 1px solid var(--ordo-border-light);
  background: transparent;
  color: var(--ordo-text-tertiary);
  font-size: 9px;
  font-weight: 500;
  border-radius: var(--ordo-radius-sm);
  cursor: pointer;
  transition: all 0.15s;
  text-transform: uppercase;
  letter-spacing: 0.3px;
}

.ordo-smart-condition__switch-btn:hover:not(:disabled) {
  border-color: var(--ordo-primary-400);
  color: var(--ordo-primary-600);
  background: var(--ordo-primary-50);
}

/* Simple row */
.ordo-smart-condition__row {
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  gap: var(--ordo-space-sm);
  align-items: center;
}

.ordo-smart-condition__field {
  min-width: 0;
}

.ordo-smart-condition__operator-wrapper {
  position: relative;
}

.ordo-smart-condition__operator {
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

.ordo-smart-condition__operator:focus {
  outline: none;
  border-color: var(--ordo-primary-500);
  box-shadow: var(--ordo-focus-ring);
}

.ordo-smart-condition__value {
  min-width: 0;
}

/* Logical Group */
.ordo-smart-condition__logical {
  display: flex;
  flex-direction: column;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-lg);
  overflow: hidden;
  background: var(--ordo-bg-secondary);
}

.ordo-smart-condition__logical-bar {
  display: flex;
  align-items: center;
  padding: var(--ordo-space-xs) var(--ordo-space-sm);
  background: var(--ordo-gray-100);
  border-bottom: 1px solid var(--ordo-border-color);
}

[data-ordo-theme='dark'] .ordo-smart-condition__logical-bar {
  background: var(--ordo-gray-800);
}

.ordo-smart-condition__logical-select {
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

.ordo-smart-condition__logical-select.and {
  color: var(--ordo-primary-600);
}

.ordo-smart-condition__logical-select.or {
  color: var(--ordo-warning);
}

.ordo-smart-condition__logical-line {
  flex: 1;
}

.ordo-smart-condition__logical-content {
  display: flex;
  flex-direction: column;
  gap: var(--ordo-space-md);
  padding: var(--ordo-space-sm);
}

.ordo-smart-condition__logical-item {
  display: flex;
  align-items: flex-start;
  gap: var(--ordo-space-sm);
  position: relative;
}

.ordo-smart-condition__logical-item > :first-child {
  flex: 1;
}

.ordo-smart-condition__remove-btn {
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
  margin-top: 4px;
}

.ordo-smart-condition__remove-btn:hover:not(:disabled) {
  background: var(--ordo-error-bg);
  color: var(--ordo-error);
}

.ordo-smart-condition__add-btn {
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

.ordo-smart-condition__add-btn:hover:not(:disabled) {
  border-color: var(--ordo-primary-400);
  color: var(--ordo-primary-600);
  background: var(--ordo-primary-50);
}

/* Expression fallback */
.ordo-smart-condition__expression {
  width: 100%;
}
</style>
