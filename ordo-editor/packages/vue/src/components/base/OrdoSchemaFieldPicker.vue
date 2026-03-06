<script setup lang="ts">
/**
 * OrdoSchemaFieldPicker - Schema-aware field selector with grouped fields,
 * fuzzy search, and keyboard navigation.
 * 基于 Schema 的字段选择器
 */
import { computed, ref, watch, nextTick, onMounted, onBeforeUnmount } from 'vue';
import type { SchemaContext, ResolvedField } from '@ordo-engine/editor-core';

export interface Props {
  modelValue: string;
  schemaContext: SchemaContext;
  placeholder?: string;
  disabled?: boolean;
  /** Filter to only show fields of specific types */
  filterTypes?: string[];
}

const props = withDefaults(defineProps<Props>(), {
  placeholder: 'Select field...',
  disabled: false,
  filterTypes: () => [],
});

const emit = defineEmits<{
  'update:modelValue': [value: string];
  change: [value: string];
}>();

const isOpen = ref(false);
const searchQuery = ref('');
const highlightIndex = ref(-1);
const containerRef = ref<HTMLElement | null>(null);
const listRef = ref<HTMLElement | null>(null);
const searchInputRef = ref<HTMLInputElement | null>(null);

// Get all fields from schema context, optionally filtered by type
const allFields = computed(() => {
  const fields = props.schemaContext.getAllFields();
  if (props.filterTypes.length === 0) return fields;
  return fields.filter((f) => props.filterTypes.includes(f.type));
});

// Group fields by top-level parent
interface FieldGroup {
  name: string;
  fields: ResolvedField[];
}

const groupedFields = computed((): FieldGroup[] => {
  const groups = new Map<string, ResolvedField[]>();
  const ungrouped: ResolvedField[] = [];

  for (const field of allFields.value) {
    const topParent = field.path.split('.')[0];
    // If field has a parent or is a top-level object, group by top-level
    if (field.parent) {
      const existing = groups.get(topParent) || [];
      existing.push(field);
      groups.set(topParent, existing);
    } else if (field.type === 'object') {
      // Top-level object acts as group header, don't add as selectable
      if (!groups.has(field.name)) {
        groups.set(field.name, []);
      }
    } else {
      ungrouped.push(field);
    }
  }

  const result: FieldGroup[] = [];

  if (ungrouped.length > 0) {
    result.push({ name: '', fields: ungrouped });
  }

  for (const [name, fields] of groups) {
    if (fields.length > 0) {
      result.push({ name, fields });
    }
  }

  return result;
});

// Filtered fields based on search query (fuzzy match)
const filteredGroups = computed((): FieldGroup[] => {
  const query = searchQuery.value.toLowerCase().trim();
  if (!query) return groupedFields.value;

  return groupedFields.value
    .map((group) => ({
      name: group.name,
      fields: group.fields.filter(
        (f) =>
          f.path.toLowerCase().includes(query) ||
          f.name.toLowerCase().includes(query) ||
          f.description?.toLowerCase().includes(query),
      ),
    }))
    .filter((group) => group.fields.length > 0);
});

// Flat list of visible fields for keyboard navigation
const flatVisibleFields = computed(() => {
  return filteredGroups.value.flatMap((g) => g.fields);
});

// Current selected field info
const selectedField = computed(() => {
  if (!props.modelValue) return null;
  return props.schemaContext.getField(props.modelValue);
});

// Display text for the trigger
const displayText = computed(() => {
  if (selectedField.value) {
    return `$.${selectedField.value.path}`;
  }
  return props.modelValue ? `$.${props.modelValue}` : '';
});

function selectField(field: ResolvedField) {
  emit('update:modelValue', field.path);
  emit('change', field.path);
  closeDropdown();
}

function openDropdown() {
  if (props.disabled) return;
  isOpen.value = true;
  searchQuery.value = '';
  highlightIndex.value = -1;
  nextTick(() => {
    searchInputRef.value?.focus();
  });
}

function closeDropdown() {
  isOpen.value = false;
  searchQuery.value = '';
  highlightIndex.value = -1;
}

function toggleDropdown() {
  if (isOpen.value) {
    closeDropdown();
  } else {
    openDropdown();
  }
}

function handleKeydown(event: KeyboardEvent) {
  if (!isOpen.value) {
    if (event.key === 'Enter' || event.key === ' ' || event.key === 'ArrowDown') {
      event.preventDefault();
      openDropdown();
    }
    return;
  }

  const fields = flatVisibleFields.value;

  switch (event.key) {
    case 'ArrowDown':
      event.preventDefault();
      highlightIndex.value = Math.min(highlightIndex.value + 1, fields.length - 1);
      scrollToHighlighted();
      break;
    case 'ArrowUp':
      event.preventDefault();
      highlightIndex.value = Math.max(highlightIndex.value - 1, 0);
      scrollToHighlighted();
      break;
    case 'Enter':
      event.preventDefault();
      if (highlightIndex.value >= 0 && highlightIndex.value < fields.length) {
        selectField(fields[highlightIndex.value]);
      }
      break;
    case 'Escape':
      event.preventDefault();
      closeDropdown();
      break;
  }
}

function scrollToHighlighted() {
  nextTick(() => {
    const highlighted = listRef.value?.querySelector('.highlighted');
    if (highlighted) {
      highlighted.scrollIntoView({ block: 'nearest' });
    }
  });
}

// Reset highlight when search changes
watch(searchQuery, () => {
  highlightIndex.value = flatVisibleFields.value.length > 0 ? 0 : -1;
});

// Get flat index of a field across groups
function getFlatIndex(field: ResolvedField): number {
  return flatVisibleFields.value.indexOf(field);
}

// Type badge color
function getTypeBadgeClass(type: string): string {
  switch (type) {
    case 'string':
      return 'type-string';
    case 'number':
      return 'type-number';
    case 'boolean':
      return 'type-boolean';
    case 'array':
      return 'type-array';
    case 'object':
      return 'type-object';
    default:
      return 'type-any';
  }
}

// Click outside to close
function handleClickOutside(event: MouseEvent) {
  if (containerRef.value && !containerRef.value.contains(event.target as Node)) {
    closeDropdown();
  }
}

onMounted(() => {
  document.addEventListener('mousedown', handleClickOutside);
});

onBeforeUnmount(() => {
  document.removeEventListener('mousedown', handleClickOutside);
});
</script>

<template>
  <div
    ref="containerRef"
    class="ordo-schema-field-picker"
    :class="{ disabled, open: isOpen }"
    @keydown="handleKeydown"
  >
    <!-- Trigger -->
    <button
      type="button"
      class="ordo-schema-field-picker__trigger"
      :disabled="disabled"
      @click="toggleDropdown"
    >
      <span v-if="displayText" class="ordo-schema-field-picker__value">
        {{ displayText }}
      </span>
      <span v-else class="ordo-schema-field-picker__placeholder">
        {{ placeholder }}
      </span>
      <span
        v-if="selectedField"
        class="ordo-schema-field-picker__type-badge"
        :class="getTypeBadgeClass(selectedField.type)"
      >
        {{ selectedField.type }}
      </span>
      <span class="ordo-schema-field-picker__arrow">
        <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.5">
          <path d="M2 4 L5 7 L8 4" />
        </svg>
      </span>
    </button>

    <!-- Dropdown -->
    <Transition name="picker-dropdown">
      <div v-if="isOpen" class="ordo-schema-field-picker__dropdown">
        <!-- Search -->
        <div class="ordo-schema-field-picker__search">
          <input
            ref="searchInputRef"
            v-model="searchQuery"
            type="text"
            placeholder="Search fields..."
            class="ordo-schema-field-picker__search-input"
            @click.stop
          />
        </div>

        <!-- Field list -->
        <div ref="listRef" class="ordo-schema-field-picker__list">
          <template v-if="filteredGroups.length > 0">
            <div
              v-for="group in filteredGroups"
              :key="group.name || '__ungrouped'"
              class="ordo-schema-field-picker__group"
            >
              <div v-if="group.name" class="ordo-schema-field-picker__group-header">
                {{ group.name }}
              </div>
              <div
                v-for="field in group.fields"
                :key="field.fullPath"
                class="ordo-schema-field-picker__item"
                :class="{
                  selected: field.path === modelValue,
                  highlighted: getFlatIndex(field) === highlightIndex,
                }"
                @click.stop="selectField(field)"
                @mouseenter="highlightIndex = getFlatIndex(field)"
              >
                <span class="ordo-schema-field-picker__item-path">$.{{ field.path }}</span>
                <span
                  class="ordo-schema-field-picker__item-type"
                  :class="getTypeBadgeClass(field.type)"
                >
                  {{ field.type }}
                </span>
                <span
                  v-if="field.description"
                  class="ordo-schema-field-picker__item-desc"
                  :title="field.description"
                >
                  {{ field.description }}
                </span>
              </div>
            </div>
          </template>
          <div v-else class="ordo-schema-field-picker__empty">
            No fields found
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.ordo-schema-field-picker {
  position: relative;
  display: inline-flex;
  min-width: 0;
  flex: 1;
}

.ordo-schema-field-picker__trigger {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  padding: 0 8px;
  height: 32px;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  background: var(--ordo-bg-input);
  cursor: pointer;
  transition: all 0.15s;
  font-size: 12px;
  text-align: left;
}

.ordo-schema-field-picker__trigger:hover:not(:disabled) {
  border-color: var(--ordo-primary-400);
}

.ordo-schema-field-picker.open .ordo-schema-field-picker__trigger {
  border-color: var(--ordo-primary-500);
  box-shadow: var(--ordo-focus-ring);
}

.ordo-schema-field-picker__value {
  flex: 1;
  font-family: var(--ordo-font-mono);
  font-size: 12px;
  color: var(--ordo-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ordo-schema-field-picker__placeholder {
  flex: 1;
  color: var(--ordo-text-tertiary);
  font-size: 12px;
}

.ordo-schema-field-picker__type-badge,
.ordo-schema-field-picker__item-type {
  font-size: 9px;
  font-weight: 600;
  padding: 1px 5px;
  border-radius: 3px;
  text-transform: uppercase;
  letter-spacing: 0.3px;
  flex-shrink: 0;
}

.type-string {
  background: var(--ordo-green-100, #dcfce7);
  color: var(--ordo-green-700, #15803d);
}
.type-number {
  background: var(--ordo-blue-100, #dbeafe);
  color: var(--ordo-blue-700, #1d4ed8);
}
.type-boolean {
  background: var(--ordo-purple-100, #f3e8ff);
  color: var(--ordo-purple-700, #7e22ce);
}
.type-array {
  background: var(--ordo-orange-100, #ffedd5);
  color: var(--ordo-orange-700, #c2410c);
}
.type-object {
  background: var(--ordo-gray-100, #f3f4f6);
  color: var(--ordo-gray-600, #4b5563);
}
.type-any {
  background: var(--ordo-gray-100, #f3f4f6);
  color: var(--ordo-gray-500, #6b7280);
}

[data-ordo-theme='dark'] .type-string {
  background: rgba(34, 197, 94, 0.15);
  color: #86efac;
}
[data-ordo-theme='dark'] .type-number {
  background: rgba(59, 130, 246, 0.15);
  color: #93bbfd;
}
[data-ordo-theme='dark'] .type-boolean {
  background: rgba(168, 85, 247, 0.15);
  color: #c4b5fd;
}
[data-ordo-theme='dark'] .type-array {
  background: rgba(249, 115, 22, 0.15);
  color: #fdba74;
}
[data-ordo-theme='dark'] .type-object {
  background: rgba(156, 163, 175, 0.15);
  color: #d1d5db;
}

.ordo-schema-field-picker__arrow {
  color: var(--ordo-text-tertiary);
  transition: transform 0.15s;
  flex-shrink: 0;
  display: flex;
}

.ordo-schema-field-picker.open .ordo-schema-field-picker__arrow {
  transform: rotate(180deg);
}

/* Dropdown */
.ordo-schema-field-picker__dropdown {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  right: 0;
  min-width: 280px;
  background: var(--ordo-bg-popup, var(--ordo-bg-card));
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  box-shadow: var(--ordo-shadow-lg, 0 10px 15px -3px rgba(0, 0, 0, 0.1));
  z-index: 200;
  overflow: hidden;
}

.ordo-schema-field-picker__search {
  padding: 6px;
  border-bottom: 1px solid var(--ordo-border-light);
}

.ordo-schema-field-picker__search-input {
  width: 100%;
  padding: 5px 8px;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  font-size: 12px;
  background: var(--ordo-bg-input);
  color: var(--ordo-text-primary);
  outline: none;
}

.ordo-schema-field-picker__search-input:focus {
  border-color: var(--ordo-primary-500);
  box-shadow: var(--ordo-focus-ring);
}

.ordo-schema-field-picker__list {
  max-height: 240px;
  overflow-y: auto;
  padding: 4px 0;
}

.ordo-schema-field-picker__group-header {
  padding: 4px 10px 2px;
  font-size: 10px;
  font-weight: 700;
  color: var(--ordo-text-tertiary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin-top: 4px;
}

.ordo-schema-field-picker__group-header:first-child {
  margin-top: 0;
}

.ordo-schema-field-picker__item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 10px;
  cursor: pointer;
  transition: background-color 0.1s;
}

.ordo-schema-field-picker__item:hover,
.ordo-schema-field-picker__item.highlighted {
  background: var(--ordo-bg-hover, var(--ordo-gray-100));
}

[data-ordo-theme='dark'] .ordo-schema-field-picker__item:hover,
[data-ordo-theme='dark'] .ordo-schema-field-picker__item.highlighted {
  background: var(--ordo-gray-700);
}

.ordo-schema-field-picker__item.selected {
  background: var(--ordo-primary-50, var(--ordo-accent-bg));
}

[data-ordo-theme='dark'] .ordo-schema-field-picker__item.selected {
  background: rgba(59, 130, 246, 0.15);
}

.ordo-schema-field-picker__item-path {
  font-family: var(--ordo-font-mono);
  font-size: 12px;
  color: var(--ordo-text-primary);
  flex-shrink: 0;
}

.ordo-schema-field-picker__item-desc {
  flex: 1;
  text-align: right;
  font-size: 11px;
  color: var(--ordo-text-tertiary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ordo-schema-field-picker__empty {
  padding: 16px;
  text-align: center;
  color: var(--ordo-text-tertiary);
  font-size: 12px;
}

.ordo-schema-field-picker.disabled {
  opacity: 0.6;
  pointer-events: none;
}

/* Dropdown transition */
.picker-dropdown-enter-active,
.picker-dropdown-leave-active {
  transition: opacity 0.15s, transform 0.15s;
}

.picker-dropdown-enter-from,
.picker-dropdown-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>
