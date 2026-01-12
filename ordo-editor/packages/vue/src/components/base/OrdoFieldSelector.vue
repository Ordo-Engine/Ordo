<script setup lang="ts">
/**
 * OrdoFieldSelector - Field path selector with nested support
 * 字段路径选择器，支持嵌套结构
 */
import { computed, ref } from 'vue';
import type { SchemaField } from '@ordo/editor-core';

export interface Props {
  /** Selected field path */
  modelValue: string;
  /** Available fields schema */
  schema?: SchemaField[];
  /** Placeholder text */
  placeholder?: string;
  /** Whether the selector is disabled */
  disabled?: boolean;
  /** Allow custom input */
  allowCustom?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  schema: () => [],
  placeholder: 'Select field...',
  disabled: false,
  allowCustom: true,
});

const emit = defineEmits<{
  'update:modelValue': [value: string];
  change: [value: string];
}>();

// State
const isOpen = ref(false);
const searchQuery = ref('');

// Flatten schema into paths
interface FlatField {
  path: string;
  label: string;
  type: string;
  description?: string;
  depth: number;
}

function flattenSchema(fields: SchemaField[], prefix = '', depth = 0): FlatField[] {
  const result: FlatField[] = [];

  for (const field of fields) {
    const path = prefix ? `${prefix}.${field.name}` : field.name;

    result.push({
      path,
      label: field.name,
      type: field.type,
      description: field.description,
      depth,
    });

    // Recurse into nested fields
    if (field.type === 'object' && field.fields) {
      result.push(...flattenSchema(field.fields, path, depth + 1));
    }
  }

  return result;
}

const flatFields = computed(() => flattenSchema(props.schema));

// Filter fields by search query
const filteredFields = computed(() => {
  const query = searchQuery.value.toLowerCase();
  if (!query) return flatFields.value;

  return flatFields.value.filter(
    (f) =>
      f.path.toLowerCase().includes(query) ||
      f.label.toLowerCase().includes(query) ||
      f.description?.toLowerCase().includes(query)
  );
});

// Selected field info
const selectedField = computed(() => {
  return flatFields.value.find((f) => f.path === props.modelValue);
});

// Handlers
function selectField(path: string) {
  emit('update:modelValue', path);
  emit('change', path);
  isOpen.value = false;
  searchQuery.value = '';
}

function handleCustomInput(event: Event) {
  const target = event.target as HTMLInputElement;
  emit('update:modelValue', target.value);
}

function handleCustomBlur() {
  emit('change', props.modelValue);
}

function toggleDropdown() {
  if (!props.disabled) {
    isOpen.value = !isOpen.value;
    if (isOpen.value) {
      searchQuery.value = '';
    }
  }
}

function handleClickOutside(event: Event) {
  const target = event.target as HTMLElement;
  if (!target.closest('.ordo-field-selector')) {
    isOpen.value = false;
  }
}

// Close on outside click
if (typeof document !== 'undefined') {
  document.addEventListener('click', handleClickOutside);
}
</script>

<template>
  <div class="ordo-field-selector" :class="{ disabled, open: isOpen }">
    <!-- Selected value / Custom input -->
    <div class="ordo-field-selector__trigger" @click="toggleDropdown">
      <template v-if="allowCustom">
        <input
          :value="modelValue"
          :placeholder="placeholder"
          :disabled="disabled"
          class="ordo-field-selector__input"
          @input="handleCustomInput"
          @blur="handleCustomBlur"
          @click.stop
        />
      </template>
      <template v-else>
        <span v-if="selectedField" class="ordo-field-selector__selected">
          <span class="ordo-field-selector__selected-path">$.{{ selectedField.path }}</span>
          <span class="ordo-field-selector__selected-type">{{ selectedField.type }}</span>
        </span>
        <span v-else class="ordo-field-selector__placeholder">{{ placeholder }}</span>
      </template>
      <span class="ordo-field-selector__arrow">▼</span>
    </div>

    <!-- Dropdown -->
    <Transition name="dropdown">
      <div v-if="isOpen" class="ordo-field-selector__dropdown">
        <!-- Search -->
        <div class="ordo-field-selector__search">
          <input
            v-model="searchQuery"
            type="text"
            placeholder="Search fields..."
            class="ordo-field-selector__search-input"
            @click.stop
          />
        </div>

        <!-- Field list -->
        <div class="ordo-field-selector__list">
          <template v-if="filteredFields.length > 0">
            <div
              v-for="field in filteredFields"
              :key="field.path"
              class="ordo-field-selector__item"
              :class="{ selected: field.path === modelValue }"
              :style="{ paddingLeft: `${12 + field.depth * 16}px` }"
              @click.stop="selectField(field.path)"
            >
              <span class="ordo-field-selector__item-path">$.{{ field.path }}</span>
              <span class="ordo-field-selector__item-type">{{ field.type }}</span>
              <span v-if="field.description" class="ordo-field-selector__item-desc">
                {{ field.description }}
              </span>
            </div>
          </template>
          <div v-else class="ordo-field-selector__empty">No fields found</div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.ordo-field-selector {
  position: relative;
  display: inline-block;
  min-width: 200px;
}

.ordo-field-selector__trigger {
  display: flex;
  align-items: center;
  gap: var(--ordo-space-sm, 8px);
  padding: var(--ordo-space-sm, 8px) var(--ordo-space-md, 12px);
  border: 1px solid var(--ordo-border-color, #e5e7eb);
  border-radius: var(--ordo-radius-sm, 4px);
  background: var(--ordo-bg-input, #ffffff);
  cursor: pointer;
  transition: border-color 0.2s;
}

.ordo-field-selector__trigger:hover:not(.disabled) {
  border-color: var(--ordo-accent, #3b82f6);
}

.ordo-field-selector.open .ordo-field-selector__trigger {
  border-color: var(--ordo-accent, #3b82f6);
  box-shadow: 0 0 0 2px var(--ordo-accent-alpha, rgba(59, 130, 246, 0.2));
}

.ordo-field-selector__input {
  flex: 1;
  border: none;
  background: transparent;
  font-family: var(--ordo-font-mono, monospace);
  font-size: var(--ordo-font-size-sm, 14px);
  color: var(--ordo-text-primary, #1a1a1a);
  outline: none;
}

.ordo-field-selector__selected {
  flex: 1;
  display: flex;
  align-items: center;
  gap: var(--ordo-space-sm, 8px);
}

.ordo-field-selector__selected-path {
  font-family: var(--ordo-font-mono, monospace);
  font-size: var(--ordo-font-size-sm, 14px);
  color: var(--ordo-text-primary, #1a1a1a);
}

.ordo-field-selector__selected-type {
  font-size: var(--ordo-font-size-xs, 12px);
  padding: 2px 6px;
  background: var(--ordo-bg-secondary, #f3f4f6);
  color: var(--ordo-text-secondary, #6b7280);
  border-radius: var(--ordo-radius-xs, 2px);
}

.ordo-field-selector__placeholder {
  flex: 1;
  color: var(--ordo-text-tertiary, #9ca3af);
  font-size: var(--ordo-font-size-sm, 14px);
}

.ordo-field-selector__arrow {
  font-size: var(--ordo-font-size-xs, 10px);
  color: var(--ordo-text-tertiary, #9ca3af);
  transition: transform 0.2s;
}

.ordo-field-selector.open .ordo-field-selector__arrow {
  transform: rotate(180deg);
}

.ordo-field-selector__dropdown {
  position: absolute;
  top: 100%;
  left: 0;
  right: 0;
  margin-top: 4px;
  background: var(--ordo-bg-popup, #ffffff);
  border: 1px solid var(--ordo-border-color, #e5e7eb);
  border-radius: var(--ordo-radius-sm, 4px);
  box-shadow: var(--ordo-shadow-md, 0 4px 6px -1px rgba(0, 0, 0, 0.1));
  z-index: 100;
  overflow: hidden;
}

.ordo-field-selector__search {
  padding: var(--ordo-space-sm, 8px);
  border-bottom: 1px solid var(--ordo-border-color, #e5e7eb);
}

.ordo-field-selector__search-input {
  width: 100%;
  padding: var(--ordo-space-xs, 4px) var(--ordo-space-sm, 8px);
  border: 1px solid var(--ordo-border-color, #e5e7eb);
  border-radius: var(--ordo-radius-sm, 4px);
  font-size: var(--ordo-font-size-sm, 14px);
}

.ordo-field-selector__list {
  max-height: 250px;
  overflow-y: auto;
}

.ordo-field-selector__item {
  display: flex;
  align-items: center;
  gap: var(--ordo-space-sm, 8px);
  padding: var(--ordo-space-sm, 8px) var(--ordo-space-md, 12px);
  cursor: pointer;
  transition: background-color 0.15s;
}

.ordo-field-selector__item:hover {
  background: var(--ordo-bg-hover, #f3f4f6);
}

.ordo-field-selector__item.selected {
  background: var(--ordo-accent-bg, #eff6ff);
}

.ordo-field-selector__item-path {
  font-family: var(--ordo-font-mono, monospace);
  font-size: var(--ordo-font-size-sm, 14px);
  color: var(--ordo-text-primary, #1a1a1a);
}

.ordo-field-selector__item-type {
  font-size: var(--ordo-font-size-xs, 12px);
  padding: 2px 6px;
  background: var(--ordo-bg-secondary, #f3f4f6);
  color: var(--ordo-text-secondary, #6b7280);
  border-radius: var(--ordo-radius-xs, 2px);
}

.ordo-field-selector__item-desc {
  flex: 1;
  text-align: right;
  font-size: var(--ordo-font-size-xs, 12px);
  color: var(--ordo-text-tertiary, #9ca3af);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ordo-field-selector__empty {
  padding: var(--ordo-space-md, 12px);
  text-align: center;
  color: var(--ordo-text-tertiary, #9ca3af);
  font-size: var(--ordo-font-size-sm, 14px);
}

.ordo-field-selector.disabled {
  opacity: 0.6;
  pointer-events: none;
}

/* Transitions */
.dropdown-enter-active,
.dropdown-leave-active {
  transition:
    opacity 0.15s,
    transform 0.15s;
}

.dropdown-enter-from,
.dropdown-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}
</style>
