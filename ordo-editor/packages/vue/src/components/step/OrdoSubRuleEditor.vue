<script setup lang="ts">
/**
 * OrdoSubRuleEditor - Sub-rule invocation editor
 */
import { computed } from 'vue';
import type {
  Step,
  SubRuleGraph,
  SubRuleStep,
  SubRuleBinding,
  SubRuleOutput,
  SubRuleAssetRef,
} from '@ordo-engine/editor-core';
import { Expr, generateId } from '@ordo-engine/editor-core';
import OrdoExpressionInput from '../base/OrdoExpressionInput.vue';
import OrdoIcon from '../icons/OrdoIcon.vue';
import { useI18n } from '../../locale';
import type { FieldSuggestion } from '../base/OrdoExpressionInput.vue';
import type { SubRuleAssetOption } from './subRuleAssets';
import { subRuleAssetOptionKey } from './subRuleAssets';

export interface Props {
  modelValue: SubRuleStep;
  availableSteps?: Step[];
  availableSubRules?: Record<string, SubRuleGraph>;
  managedSubRules?: SubRuleAssetOption[];
  suggestions?: FieldSuggestion[];
  disabled?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  availableSteps: () => [],
  availableSubRules: () => ({}),
  managedSubRules: () => [],
  suggestions: () => [],
  disabled: false,
});

const emit = defineEmits<{
  'update:modelValue': [value: SubRuleStep];
  change: [value: SubRuleStep];
  'open-sub-rule': [name: string];
}>();

const { t } = useI18n();

const subRuleOptions = computed(() =>
  Object.keys(props.availableSubRules).map((name) => ({ value: name, label: name }))
);

const selectedGraph = computed(() =>
  props.modelValue.refName ? props.availableSubRules[props.modelValue.refName] : undefined
);

const selectedAssetKey = computed(() => {
  if (!props.modelValue.refName) return '';
  const scope = props.modelValue.assetRef?.scope ?? 'project';
  const option = props.managedSubRules.find(
    (item) => item.name === props.modelValue.refName && item.scope === scope
  );
  return option ? subRuleAssetOptionKey(option) : '';
});

const inputFieldOptions = computed(() => selectedGraph.value?.inputSchema ?? []);
const outputFieldOptions = computed(() => selectedGraph.value?.outputSchema ?? []);

const stepOptions = computed(() => {
  return props.availableSteps
    .filter((s) => s.id !== props.modelValue.id)
    .map((s) => ({ value: s.id, label: `${s.name} (${getStepTypeLabel(s.type)})` }));
});

function getStepTypeLabel(type: Step['type']) {
  switch (type) {
    case 'decision':
      return t('step.typeDecision');
    case 'action':
      return t('step.typeAction');
    case 'terminal':
      return t('step.typeTerminal');
    case 'sub_rule':
      return t('step.typeSubRule');
    default:
      return type;
  }
}

function updateStep(patch: Partial<SubRuleStep>, changed = false) {
  const next = { ...props.modelValue, ...patch };
  emit('update:modelValue', next);
  if (changed) emit('change', next);
}

function updateName(event: Event) {
  updateStep({ name: (event.target as HTMLInputElement).value });
}

function updateDescription(event: Event) {
  updateStep({ description: (event.target as HTMLTextAreaElement).value || undefined });
}

function updateRefName(event: Event) {
  const refName = (event.target as HTMLInputElement).value;
  const existing = props.managedSubRules.find((item) => item.name === refName);
  updateStep(
    {
      refName,
      assetRef: {
        scope: existing?.scope ?? props.modelValue.assetRef?.scope ?? 'project',
        name: refName,
      },
    },
    true
  );
}

function updateManagedAsset(event: Event) {
  const key = (event.target as HTMLSelectElement).value;
  if (!key) return;

  const option = props.managedSubRules.find((item) => subRuleAssetOptionKey(item) === key);
  if (!option) return;

  updateStep(
    {
      refName: option.name,
      assetRef: {
        scope: option.scope,
        name: option.name,
      },
    },
    true
  );
}

function updateAssetScope(event: Event) {
  const scope = (event.target as HTMLSelectElement).value as SubRuleAssetRef['scope'];
  updateStep(
    {
      assetRef: {
        scope,
        name: props.modelValue.assetRef?.name || props.modelValue.refName,
      },
    },
    true
  );
}

function updateNextStep(event: Event) {
  updateStep({ nextStepId: (event.target as HTMLSelectElement).value }, true);
}

function addBinding() {
  const field = inputFieldOptions.value.find((item) => item.required)?.name ?? '';
  const binding: SubRuleBinding = {
    field,
    expr: Expr.string(''),
  };
  updateStep({ bindings: [...(props.modelValue.bindings ?? []), binding] }, true);
}

function updateBinding(index: number, patch: Partial<SubRuleBinding>) {
  const bindings = [...(props.modelValue.bindings ?? [])];
  bindings[index] = { ...bindings[index], ...patch };
  updateStep({ bindings }, true);
}

function removeBinding(index: number) {
  const bindings = (props.modelValue.bindings ?? []).filter((_, i) => i !== index);
  updateStep({ bindings: bindings.length ? bindings : undefined }, true);
}

function addOutput() {
  const childVar = outputFieldOptions.value.find((item) => item.required)?.name ?? '';
  const output: SubRuleOutput = {
    parentVar: childVar || `var_${generateId('').slice(0, 6)}`,
    childVar,
  };
  updateStep({ outputs: [...(props.modelValue.outputs ?? []), output] }, true);
}

function updateOutput(index: number, patch: Partial<SubRuleOutput>) {
  const outputs = [...(props.modelValue.outputs ?? [])];
  outputs[index] = { ...outputs[index], ...patch };
  updateStep({ outputs }, true);
}

function removeOutput(index: number) {
  const outputs = (props.modelValue.outputs ?? []).filter((_, i) => i !== index);
  updateStep({ outputs: outputs.length ? outputs : undefined }, true);
}

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

function updateExprValue(value: string): any {
  if (value.startsWith('$')) return Expr.variable(value);
  if (!isNaN(Number(value)) && value !== '') return Expr.number(Number(value));
  if (value === 'true') return Expr.boolean(true);
  if (value === 'false') return Expr.boolean(false);
  return Expr.string(value);
}
</script>

<template>
  <div class="ordo-editor-panel sub-rule">
    <div class="ordo-form-row">
      <div class="ordo-form-group grow">
        <label>{{ t('common.name') }}</label>
        <input
          :value="modelValue.name"
          :disabled="disabled"
          class="ordo-input-base"
          @input="updateName"
          @blur="emit('change', modelValue)"
        />
      </div>
    </div>

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

    <div class="ordo-form-row">
      <div class="ordo-form-group full">
        <label>{{ t('step.managedAsset') }}</label>
        <select
          :value="selectedAssetKey"
          :disabled="disabled || managedSubRules.length === 0"
          class="ordo-input-base"
          @change="updateManagedAsset"
        >
          <option value="">{{ t('step.customSubRuleName') }}</option>
          <option
            v-for="asset in managedSubRules"
            :key="subRuleAssetOptionKey(asset)"
            :value="subRuleAssetOptionKey(asset)"
          >
            {{
              `${asset.displayName || asset.name} · ${
                asset.scope === 'org' ? t('step.scopeOrg') : t('step.scopeProject')
              }`
            }}
          </option>
        </select>
      </div>
    </div>

    <div class="ordo-form-row">
      <div class="ordo-form-group grow">
        <label>{{ t('step.refName') }}</label>
        <div class="ordo-input-with-action">
          <input
            :value="modelValue.refName"
            :disabled="disabled"
            list="ordo-sub-rule-options"
            class="ordo-input-base"
            @input="updateRefName"
          />
          <button
            v-if="modelValue.refName"
            class="ordo-btn-open-subrule"
            :title="t('step.openSubRuleEditor')"
            @click="emit('open-sub-rule', modelValue.refName)"
          >
            <OrdoIcon name="arrow_right_up" :size="14" />
          </button>
        </div>
        <datalist id="ordo-sub-rule-options">
          <option v-for="opt in subRuleOptions" :key="opt.value" :value="opt.value">
            {{ opt.label }}
          </option>
        </datalist>
      </div>
      <div class="ordo-form-group">
        <label>{{ t('step.assetScope') }}</label>
        <select
          :value="modelValue.assetRef?.scope || 'project'"
          :disabled="disabled"
          class="ordo-input-base"
          @change="updateAssetScope"
        >
          <option value="project">{{ t('step.scopeProject') }}</option>
          <option value="org">{{ t('step.scopeOrg') }}</option>
        </select>
      </div>
    </div>

    <div class="ordo-section">
      <div class="ordo-section-header">
        <span class="title">{{ t('step.bindings') }}</span>
        <button class="ordo-btn-text" :disabled="disabled" @click="addBinding">
          <OrdoIcon name="add" :size="12" /> {{ t('step.addBinding') }}
        </button>
      </div>

      <div class="ordo-table-container">
        <table class="ordo-data-table" v-if="modelValue.bindings?.length">
          <thead>
            <tr>
              <th width="35%">{{ t('step.childField') }}</th>
              <th width="55%">{{ t('common.value') }}</th>
              <th width="10%"></th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(binding, index) in modelValue.bindings" :key="index">
              <td>
                <input
                  :value="binding.field"
                  :disabled="disabled"
                  class="ordo-input-clean"
                  :list="`sub-rule-inputs-${modelValue.id}`"
                  @input="
                    updateBinding(index, { field: ($event.target as HTMLInputElement).value })
                  "
                />
              </td>
              <td>
                <OrdoExpressionInput
                  :model-value="getExprValue(binding.expr)"
                  :suggestions="suggestions"
                  :disabled="disabled"
                  @update:model-value="updateBinding(index, { expr: updateExprValue($event) })"
                />
              </td>
              <td class="center">
                <button class="ordo-btn-icon danger" @click="removeBinding(index)">
                  <OrdoIcon name="delete" :size="14" />
                </button>
              </td>
            </tr>
          </tbody>
        </table>
        <div v-else class="ordo-empty-state">{{ t('step.noBindings') }}</div>
      </div>
      <datalist :id="`sub-rule-inputs-${modelValue.id}`">
        <option v-for="field in inputFieldOptions" :key="field.name" :value="field.name" />
      </datalist>
    </div>

    <div class="ordo-section">
      <div class="ordo-section-header">
        <span class="title">{{ t('step.outputs') }}</span>
        <button class="ordo-btn-text" :disabled="disabled" @click="addOutput">
          <OrdoIcon name="add" :size="12" /> {{ t('step.addOutput') }}
        </button>
      </div>

      <div class="ordo-table-container">
        <table class="ordo-data-table" v-if="modelValue.outputs?.length">
          <thead>
            <tr>
              <th width="45%">{{ t('step.parentVariable') }}</th>
              <th width="45%">{{ t('step.childVariable') }}</th>
              <th width="10%"></th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(output, index) in modelValue.outputs" :key="index">
              <td>
                <input
                  :value="output.parentVar"
                  :disabled="disabled"
                  class="ordo-input-clean"
                  @input="
                    updateOutput(index, { parentVar: ($event.target as HTMLInputElement).value })
                  "
                />
              </td>
              <td>
                <input
                  :value="output.childVar"
                  :disabled="disabled"
                  class="ordo-input-clean"
                  :list="`sub-rule-outputs-${modelValue.id}`"
                  @input="
                    updateOutput(index, { childVar: ($event.target as HTMLInputElement).value })
                  "
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
        <div v-else class="ordo-empty-state">{{ t('step.noSubRuleOutputs') }}</div>
      </div>
      <datalist :id="`sub-rule-outputs-${modelValue.id}`">
        <option v-for="field in outputFieldOptions" :key="field.name" :value="field.name" />
      </datalist>
    </div>

    <div class="ordo-form-row">
      <div class="ordo-form-group full">
        <label>{{ t('step.nextStep') }}</label>
        <select
          :value="modelValue.nextStepId"
          :disabled="disabled"
          class="ordo-input-base"
          @change="updateNextStep"
        >
          <option value="">{{ t('common.endFlow') }}</option>
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

.ordo-form-group label,
.ordo-section-header .title {
  font-size: 11px;
  font-weight: 600;
  color: var(--ordo-text-secondary);
  text-transform: uppercase;
}

.ordo-section {
  display: flex;
  flex-direction: column;
  gap: var(--ordo-space-sm);
}

.ordo-section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--ordo-border-light);
  padding-bottom: 4px;
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

.center {
  text-align: center;
}

.ordo-btn-icon.danger {
  color: var(--ordo-error);
}

.ordo-btn-icon.danger:hover {
  background: var(--ordo-error-bg);
}

.ordo-input-with-action {
  display: flex;
  gap: 4px;
  align-items: center;
}

.ordo-input-with-action .ordo-input-base {
  flex: 1;
  min-width: 0;
}

.ordo-btn-open-subrule {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  padding: 0;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  background: transparent;
  color: var(--ordo-accent);
  cursor: pointer;
  transition:
    background 0.15s,
    border-color 0.15s;
}

.ordo-btn-open-subrule:hover {
  background: var(--ordo-accent-bg);
  border-color: var(--ordo-accent);
}
</style>
