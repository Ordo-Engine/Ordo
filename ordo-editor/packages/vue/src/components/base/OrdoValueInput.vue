<script setup lang="ts">
/**
 * OrdoValueInput - Value input with type auto-detection
 * 值输入组件，自动检测类型
 */
import { computed, ref, watch } from 'vue';

export interface Props {
  /** Current value */
  modelValue: string | number | boolean | null;
  /** Value type hint */
  type?: 'string' | 'number' | 'boolean' | 'auto';
  /** Placeholder text */
  placeholder?: string;
  /** Whether the input is disabled */
  disabled?: boolean;
  /** Whether the input is readonly */
  readonly?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  type: 'auto',
  placeholder: '',
  disabled: false,
  readonly: false,
});

const emit = defineEmits<{
  'update:modelValue': [value: string | number | boolean | null];
  change: [value: string | number | boolean | null];
}>();

// Internal string representation
const internalValue = ref('');

// Detect type from value
const detectedType = computed(() => {
  if (props.type !== 'auto') return props.type;

  const val = props.modelValue;
  if (val === null) return 'string';
  if (typeof val === 'boolean') return 'boolean';
  if (typeof val === 'number') return 'number';
  return 'string';
});

// Sync internal value with model value
watch(
  () => props.modelValue,
  (newVal) => {
    if (newVal === null) {
      internalValue.value = '';
    } else if (typeof newVal === 'boolean') {
      internalValue.value = newVal ? 'true' : 'false';
    } else {
      internalValue.value = String(newVal);
    }
  },
  { immediate: true }
);

// Handle input change
function handleInput(event: Event) {
  const target = event.target as HTMLInputElement;
  internalValue.value = target.value;
}

function handleChange() {
  const rawValue = internalValue.value.trim();

  let parsedValue: string | number | boolean | null;

  if (rawValue === '' || rawValue === 'null') {
    parsedValue = null;
  } else if (detectedType.value === 'boolean') {
    parsedValue = rawValue.toLowerCase() === 'true';
  } else if (detectedType.value === 'number') {
    const num = Number(rawValue);
    parsedValue = isNaN(num) ? rawValue : num;
  } else {
    // Try to auto-detect
    if (rawValue.toLowerCase() === 'true') parsedValue = true;
    else if (rawValue.toLowerCase() === 'false') parsedValue = false;
    else if (!isNaN(Number(rawValue)) && rawValue !== '') parsedValue = Number(rawValue);
    else parsedValue = rawValue;
  }

  emit('update:modelValue', parsedValue);
  emit('change', parsedValue);
}

// Toggle boolean value
function toggleBoolean() {
  if (detectedType.value === 'boolean' && !props.disabled && !props.readonly) {
    const newVal = props.modelValue !== true;
    emit('update:modelValue', newVal);
    emit('change', newVal);
  }
}
</script>

<template>
  <div class="ordo-value-input" :class="{ disabled, readonly }">
    <!-- Boolean toggle -->
    <template v-if="detectedType === 'boolean'">
      <button
        type="button"
        class="ordo-value-input__toggle"
        :class="{ active: modelValue === true }"
        :disabled="disabled"
        @click="toggleBoolean"
      >
        <span class="ordo-value-input__toggle-track">
          <span class="ordo-value-input__toggle-thumb" />
        </span>
        <span class="ordo-value-input__toggle-label">
          {{ modelValue === true ? 'TRUE' : 'FALSE' }}
        </span>
      </button>
    </template>

    <!-- Number/String input -->
    <template v-else>
      <div class="ordo-value-input__wrapper">
        <input
          :type="detectedType === 'number' ? 'number' : 'text'"
          :value="internalValue"
          :placeholder="placeholder"
          :disabled="disabled"
          :readonly="readonly"
          class="ordo-value-input__field"
          @input="handleInput"
          @blur="handleChange"
          @keyup.enter="handleChange"
        />
        <span v-if="type === 'auto'" class="ordo-value-input__type-badge">
          {{ typeof modelValue === 'object' && modelValue === null ? 'null' : typeof modelValue }}
        </span>
      </div>
    </template>
  </div>
</template>

<style scoped>
.ordo-value-input {
  display: inline-flex;
  align-items: center;
  max-width: 100%;
}

.ordo-value-input__wrapper {
  position: relative;
  width: 100%;
  display: flex;
  align-items: center;
}

.ordo-value-input__field {
  width: 100%;
  height: 32px;
  padding: 0 var(--ordo-space-md) 0 var(--ordo-space-sm);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  font-size: var(--ordo-font-size-sm);
  font-family: var(--ordo-font-mono);
  background: var(--ordo-bg-input);
  color: var(--ordo-text-primary);
  transition: var(--ordo-transition-base);
}

.ordo-value-input__field:focus {
  outline: none;
  border-color: var(--ordo-primary-500);
  box-shadow: var(--ordo-focus-ring);
}

.ordo-value-input__field:hover:not(:disabled):not(:focus) {
  border-color: var(--ordo-border-hover);
}

.ordo-value-input__field:disabled {
  background: var(--ordo-bg-disabled);
  color: var(--ordo-text-tertiary);
  cursor: not-allowed;
}

.ordo-value-input__type-badge {
  position: absolute;
  right: 6px;
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  color: var(--ordo-text-tertiary);
  padding: 1px 4px;
  background: var(--ordo-bg-secondary);
  border-radius: var(--ordo-radius-sm);
  pointer-events: none;
  user-select: none;
}

.ordo-value-input__toggle {
  display: inline-flex;
  align-items: center;
  gap: var(--ordo-space-sm);
  padding: 0;
  border: none;
  background: none;
  cursor: pointer;
  font-size: var(--ordo-font-size-sm);
  user-select: none;
}

.ordo-value-input__toggle:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}

.ordo-value-input__toggle-track {
  position: relative;
  display: inline-block;
  width: 36px;
  height: 20px;
  border-radius: 99px;
  background: var(--ordo-gray-300);
  transition: var(--ordo-transition-base);
}

.ordo-value-input__toggle.active .ordo-value-input__toggle-track {
  background: var(--ordo-success);
}

.ordo-value-input__toggle-thumb {
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

.ordo-value-input__toggle.active .ordo-value-input__toggle-thumb {
  transform: translateX(16px);
}

.ordo-value-input__toggle-label {
  font-family: var(--ordo-font-mono);
  font-size: 11px;
  font-weight: 600;
  color: var(--ordo-text-secondary);
  min-width: 36px;
}
</style>
