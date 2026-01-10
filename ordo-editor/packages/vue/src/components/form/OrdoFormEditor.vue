<script setup lang="ts">
/**
 * OrdoFormEditor - Form-based ruleset editor (IDE Style)
 * 表单模式规则集编辑器
 */
import { computed, ref, provide, watch, type Ref } from 'vue';
import type { RuleSet, SchemaField } from '@ordo/editor-core';
import { validateRuleSet, type ValidationResult } from '@ordo/editor-core';
import OrdoStepList from './OrdoStepList.vue';
import type { FieldSuggestion } from '../base/OrdoExpressionInput.vue';
import { useI18n, type Lang, LOCALE_KEY } from '../../locale';

export interface Props {
  /** RuleSet data */
  modelValue: RuleSet;
  /** Whether the editor is disabled */
  disabled?: boolean;
  /** Whether to auto-validate on change */
  autoValidate?: boolean;
  /** Show validation panel */
  showValidation?: boolean;
  /** Locale */
  locale?: Lang;
}

const props = withDefaults(defineProps<Props>(), {
  disabled: false,
  autoValidate: true,
  showValidation: true,
  locale: 'en',
});

const emit = defineEmits<{
  'update:modelValue': [value: RuleSet];
  'change': [value: RuleSet];
  'validate': [result: ValidationResult];
}>();

// Provide locale using the shared key
const currentLocale = ref<Lang>(props.locale);
watch(() => props.locale, (val) => {
  currentLocale.value = val;
});
provide(LOCALE_KEY, currentLocale);

// Use i18n inside this component as well
const { t } = useI18n();

const validationResult = ref<ValidationResult | null>(null);

// Convert input schema to field suggestions
const suggestions = computed<FieldSuggestion[]>(() => {
  if (!props.modelValue.config.inputSchema) return [];

  function flattenSchema(fields: SchemaField[], prefix = ''): FieldSuggestion[] {
    const result: FieldSuggestion[] = [];
    for (const field of fields) {
      const path = prefix ? `${prefix}.${field.name}` : field.name;
      result.push({
        path,
        label: field.name,
        type: field.type,
        description: field.description,
      });
      if (field.type === 'object' && field.fields) {
        result.push(...flattenSchema(field.fields, path));
      }
    }
    return result;
  }

  return flattenSchema(props.modelValue.config.inputSchema);
});

// Update fields
function updateName(event: Event) {
  const target = event.target as HTMLInputElement;
  const newRuleset: RuleSet = { ...props.modelValue, config: { ...props.modelValue.config, name: target.value } };
  emit('update:modelValue', newRuleset);
}

function updateVersion(event: Event) {
  const target = event.target as HTMLInputElement;
  const newRuleset: RuleSet = { ...props.modelValue, config: { ...props.modelValue.config, version: target.value } };
  emit('update:modelValue', newRuleset);
}

function handleRulesetUpdate(ruleset: RuleSet) {
  emit('update:modelValue', ruleset);
}

function handleRulesetChange(ruleset: RuleSet) {
  emit('update:modelValue', ruleset);
  emit('change', ruleset);
  if (props.autoValidate) validate(ruleset);
}

function validate(ruleset: RuleSet = props.modelValue) {
  validationResult.value = validateRuleSet(ruleset);
  emit('validate', validationResult.value);
  return validationResult.value;
}

defineExpose({ validate });
</script>

<template>
  <div class="ordo-editor-container" :class="{ disabled }">
    <!-- Top Bar (Mini Config) -->
    <div class="ordo-editor-header">
      <div class="ordo-config-row">
        <div class="ordo-field-group">
          <label>{{ t('common.name') }}</label>
          <input
            :value="modelValue.config.name"
            :disabled="disabled"
            :placeholder="t('common.name')"
            class="ordo-input-base"
            @input="updateName"
          />
        </div>
        <div class="ordo-field-group small">
          <label>{{ t('common.version') }}</label>
          <input
            :value="modelValue.config.version || ''"
            :disabled="disabled"
            placeholder="1.0.0"
            class="ordo-input-base"
            @input="updateVersion"
          />
        </div>
      </div>
      
      <!-- Validation Indicator -->
      <div v-if="showValidation && validationResult" class="ordo-validation-status">
        <span 
          class="status-dot"
          :class="{ valid: validationResult.valid, invalid: !validationResult.valid }"
        ></span>
        <span class="status-text">
          {{ validationResult.valid ? t('validation.valid') : `${validationResult.errors.length} ${t('validation.invalid')}` }}
        </span>
      </div>
    </div>

    <!-- Main Content Area -->
    <div class="ordo-editor-body">
      <OrdoStepList
        :model-value="modelValue"
        :suggestions="suggestions"
        :disabled="disabled"
        @update:model-value="handleRulesetUpdate"
        @change="handleRulesetChange"
      />
    </div>

    <!-- Validation Footer (Collapsible/Fixed) -->
    <div v-if="showValidation && validationResult && !validationResult.valid" class="ordo-validation-footer">
      <div v-for="(error, index) in validationResult.errors" :key="index" class="validation-item error">
        <span class="icon">×</span> {{ error.message }}
      </div>
    </div>
  </div>
</template>

<style scoped>
.ordo-editor-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background: var(--ordo-bg-editor);
  color: var(--ordo-text-primary);
  font-family: var(--ordo-font-sans);
  overflow: hidden;
}

.ordo-editor-header {
  padding: 8px 12px;
  border-bottom: 1px solid var(--ordo-border-color);
  background: var(--ordo-bg-panel);
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-shrink: 0;
}

.ordo-config-row {
  display: flex;
  gap: 12px;
  align-items: center;
}

.ordo-field-group {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ordo-field-group label {
  font-size: 11px;
  font-weight: 600;
  color: var(--ordo-text-tertiary);
  text-transform: uppercase;
  white-space: nowrap;
}

.ordo-field-group.small input {
  width: 60px;
}

.ordo-validation-status {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  padding: 2px 6px;
  background: var(--ordo-bg-app);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
}

.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--ordo-gray-400);
}

.status-dot.valid { background: var(--ordo-success); }
.status-dot.invalid { background: var(--ordo-error); }

.ordo-editor-body {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}

.ordo-validation-footer {
  border-top: 1px solid var(--ordo-border-color);
  padding: 4px 8px;
  background: var(--ordo-bg-panel);
  max-height: 100px;
  overflow-y: auto;
  font-size: 11px;
}

.validation-item.error {
  color: var(--ordo-error);
  display: flex;
  align-items: center;
  gap: 4px;
}
</style>
