<script setup lang="ts">
/**
 * OrdoSchemaEditor - Visual JIT Schema Editor
 * 可视化 JIT Schema 编辑器
 *
 * Allows users to:
 * - Import .proto files
 * - Manually define schema fields
 * - View and edit byte offsets
 * - Preview generated Rust TypedContext code
 */
import { ref, computed, watch } from 'vue';
import type {
  JITSchema,
  JITSchemaField,
  JITFieldType,
  JITPrimitiveType,
} from '@ordo-engine/editor-core';
import {
  parseProtoFile,
  validateProtoContent,
  generateSampleProto,
  createEmptyJITSchema,
  calculateJITSchemaLayout,
} from '@ordo-engine/editor-core';

export interface Props {
  /** Current schema (v-model) */
  modelValue?: JITSchema | null;
  /** Whether the editor is read-only */
  readonly?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: null,
  readonly: false,
});

const emit = defineEmits<{
  'update:modelValue': [value: JITSchema];
  change: [value: JITSchema];
}>();

// Primitive types available for selection
const primitiveTypes: JITPrimitiveType[] = [
  'bool',
  'int32',
  'int64',
  'uint32',
  'uint64',
  'float32',
  'float64',
  'string',
  'bytes',
];

// Internal state
const schema = ref<JITSchema>(props.modelValue ?? createEmptyJITSchema('NewSchema'));
const activeTab = ref<'fields' | 'import' | 'preview'>('fields');
const protoContent = ref('');
const protoErrors = ref<string[]>([]);
const editingFieldIndex = ref<number | null>(null);

// Watch for external changes
watch(
  () => props.modelValue,
  (newVal) => {
    if (newVal) {
      schema.value = { ...newVal };
    }
  }
);

// Computed: parsed schemas from proto content
const parsedSchemas = computed(() => {
  if (!protoContent.value.trim()) return [];
  try {
    return parseProtoFile(protoContent.value);
  } catch {
    return [];
  }
});

// Emit changes
function emitChange() {
  emit('update:modelValue', schema.value);
  emit('change', schema.value);
}

// Add a new field
function addField() {
  const newField: JITSchemaField = {
    name: `field_${schema.value.fields.length + 1}`,
    type: 'float64',
    offset: 0,
    size: 8,
    required: true,
  };
  schema.value.fields.push(newField);
  recalculateLayout();
  editingFieldIndex.value = schema.value.fields.length - 1;
}

// Remove a field
function removeField(index: number) {
  schema.value.fields.splice(index, 1);
  recalculateLayout();
  emitChange();
}

// Update a field
function updateField(index: number, updates: Partial<JITSchemaField>) {
  schema.value.fields[index] = { ...schema.value.fields[index], ...updates };
  recalculateLayout();
  emitChange();
}

// Recalculate layout with proper offsets
function recalculateLayout() {
  const layoutResult = calculateJITSchemaLayout(schema.value);
  schema.value.fields = layoutResult.fields;
  schema.value.totalSize = layoutResult.totalSize;
}

// Import proto content
function importProto() {
  const validation = validateProtoContent(protoContent.value);
  if (!validation.valid) {
    protoErrors.value = validation.errors;
    return;
  }
  protoErrors.value = [];

  const schemas = parseProtoFile(protoContent.value);
  if (schemas.length > 0) {
    // Use the first schema
    schema.value = schemas[0];
    emitChange();
    activeTab.value = 'fields';
  }
}

// Load a specific schema from parsed list
function loadSchema(selectedSchema: JITSchema) {
  schema.value = { ...selectedSchema };
  emitChange();
  activeTab.value = 'fields';
}

// Load sample proto
function loadSample() {
  protoContent.value = generateSampleProto();
}

// Generate Rust TypedContext code
const generatedRustCode = computed(() => {
  const s = schema.value;
  if (!s || s.fields.length === 0) return '// Define fields to see generated code';

  let code = `/// Generated JIT Schema for ${s.name}\n`;
  code += `#[repr(C)]\n`;
  code += `pub struct ${s.name} {\n`;

  for (const field of s.fields) {
    const rustType = jitTypeToRust(field.type);
    code += `    /// Offset: ${field.offset}, Size: ${field.size}\n`;
    code += `    pub ${field.name}: ${rustType},\n`;
  }

  code += `}\n\n`;
  code += `impl ${s.name} {\n`;
  code += `    pub const SIZE: usize = ${s.totalSize};\n`;
  code += `}\n\n`;

  // Add field offset constants
  code += `// Field offsets for JIT compilation\n`;
  code += `pub mod ${s.name.toLowerCase()}_offsets {\n`;
  for (const field of s.fields) {
    code += `    pub const ${field.name.toUpperCase()}: usize = ${field.offset};\n`;
  }
  code += `}\n`;

  return code;
});

// Convert JIT type to Rust type
function jitTypeToRust(type: JITFieldType): string {
  if (typeof type === 'string') {
    switch (type) {
      case 'bool':
        return 'bool';
      case 'int32':
        return 'i32';
      case 'int64':
        return 'i64';
      case 'uint32':
        return 'u32';
      case 'uint64':
        return 'u64';
      case 'float32':
        return 'f32';
      case 'float64':
        return 'f64';
      case 'string':
        return 'String';
      case 'bytes':
        return 'Vec<u8>';
      default:
        return 'Unknown';
    }
  }
  if ('message' in type) {
    return type.message;
  }
  if ('repeated' in type) {
    return `Vec<${jitTypeToRust(type.repeated)}>`;
  }
  if ('optional' in type) {
    return `Option<${jitTypeToRust(type.optional)}>`;
  }
  if ('enum' in type) {
    return type.enum;
  }
  return 'Unknown';
}

// Get display type for field
function getDisplayType(type: JITFieldType): string {
  if (typeof type === 'string') return type;
  if ('message' in type) return `msg:${type.message}`;
  if ('repeated' in type) return `[]${getDisplayType(type.repeated)}`;
  if ('optional' in type) return `?${getDisplayType(type.optional)}`;
  if ('enum' in type) return `enum:${type.enum}`;
  return 'unknown';
}
</script>

<template>
  <div class="ordo-schema-editor">
    <!-- Header -->
    <div class="schema-header">
      <input
        v-model="schema.name"
        class="schema-name-input"
        placeholder="Schema Name"
        :disabled="readonly"
        @change="emitChange"
      />
      <div class="schema-meta">
        <span class="meta-item">
          <span class="meta-label">Fields:</span>
          <span class="meta-value">{{ schema.fields.length }}</span>
        </span>
        <span class="meta-item">
          <span class="meta-label">Size:</span>
          <span class="meta-value">{{ schema.totalSize }} bytes</span>
        </span>
        <span v-if="schema.source" class="meta-badge" :class="schema.source">
          {{ schema.source }}
        </span>
      </div>
    </div>

    <!-- Tabs -->
    <div class="schema-tabs">
      <button class="tab" :class="{ active: activeTab === 'fields' }" @click="activeTab = 'fields'">
        Fields
      </button>
      <button class="tab" :class="{ active: activeTab === 'import' }" @click="activeTab = 'import'">
        Import Proto
      </button>
      <button
        class="tab"
        :class="{ active: activeTab === 'preview' }"
        @click="activeTab = 'preview'"
      >
        Rust Code
      </button>
    </div>

    <!-- Fields Tab -->
    <div v-if="activeTab === 'fields'" class="tab-content fields-tab">
      <div v-if="schema.fields.length === 0" class="empty-state">
        <p>No fields defined. Add fields manually or import from a .proto file.</p>
        <button v-if="!readonly" class="add-btn" @click="addField">Add Field</button>
      </div>

      <div v-else class="field-list">
        <div
          v-for="(field, index) in schema.fields"
          :key="index"
          class="field-item"
          :class="{ editing: editingFieldIndex === index }"
        >
          <div
            class="field-header"
            @click="editingFieldIndex = editingFieldIndex === index ? null : index"
          >
            <span class="field-offset">@{{ field.offset }}</span>
            <span class="field-name">{{ field.name }}</span>
            <span class="field-type">{{ getDisplayType(field.type) }}</span>
            <span class="field-size">{{ field.size }}B</span>
            <button
              v-if="!readonly"
              class="remove-btn"
              @click.stop="removeField(index)"
              title="Remove field"
            >
              &times;
            </button>
          </div>

          <!-- Expanded edit form -->
          <div v-if="editingFieldIndex === index && !readonly" class="field-edit">
            <div class="edit-row">
              <label>Name</label>
              <input
                :value="field.name"
                @input="updateField(index, { name: ($event.target as HTMLInputElement).value })"
              />
            </div>
            <div class="edit-row">
              <label>Type</label>
              <select
                :value="typeof field.type === 'string' ? field.type : ''"
                @change="
                  updateField(index, {
                    type: ($event.target as HTMLSelectElement).value as JITPrimitiveType,
                  })
                "
              >
                <option v-for="t in primitiveTypes" :key="t" :value="t">{{ t }}</option>
              </select>
            </div>
            <div class="edit-row">
              <label>Proto Tag</label>
              <input
                type="number"
                :value="field.protoTag"
                @input="
                  updateField(index, {
                    protoTag: parseInt(($event.target as HTMLInputElement).value) || undefined,
                  })
                "
              />
            </div>
            <div class="edit-row checkbox">
              <label>
                <input
                  type="checkbox"
                  :checked="field.required"
                  @change="
                    updateField(index, { required: ($event.target as HTMLInputElement).checked })
                  "
                />
                Required
              </label>
            </div>
          </div>
        </div>
      </div>

      <button v-if="!readonly && schema.fields.length > 0" class="add-field-btn" @click="addField">
        + Add Field
      </button>
    </div>

    <!-- Import Tab -->
    <div v-if="activeTab === 'import'" class="tab-content import-tab">
      <div class="import-actions">
        <button class="sample-btn" @click="loadSample">Load Sample</button>
      </div>

      <textarea
        v-model="protoContent"
        class="proto-input"
        placeholder="Paste your .proto file content here..."
      ></textarea>

      <div v-if="protoErrors.length" class="error-list">
        <div v-for="(error, idx) in protoErrors" :key="idx" class="error-item">
          {{ error }}
        </div>
      </div>

      <div v-if="parsedSchemas.length > 0" class="parsed-schemas">
        <h4>Found {{ parsedSchemas.length }} message(s):</h4>
        <div
          v-for="s in parsedSchemas"
          :key="s.name"
          class="parsed-schema-item"
          @click="loadSchema(s)"
        >
          <span class="schema-name">{{ s.name }}</span>
          <span class="schema-fields">{{ s.fields.length }} fields</span>
        </div>
      </div>

      <button class="import-btn" :disabled="!protoContent.trim()" @click="importProto">
        Import Selected
      </button>
    </div>

    <!-- Preview Tab -->
    <div v-if="activeTab === 'preview'" class="tab-content preview-tab">
      <pre class="rust-code">{{ generatedRustCode }}</pre>
      <button class="copy-btn" @click="navigator.clipboard.writeText(generatedRustCode)">
        Copy to Clipboard
      </button>
    </div>
  </div>
</template>

<style scoped>
.ordo-schema-editor {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--ordo-bg-panel);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  overflow: hidden;
}

.schema-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  background: var(--ordo-bg-item);
  border-bottom: 1px solid var(--ordo-border-color);
}

.schema-name-input {
  font-size: 16px;
  font-weight: 600;
  padding: 4px 8px;
  border: 1px solid transparent;
  border-radius: var(--ordo-radius-sm);
  background: transparent;
  color: var(--ordo-text-primary);
}

.schema-name-input:focus {
  outline: none;
  border-color: var(--ordo-primary-500);
  background: var(--ordo-bg-input);
}

.schema-meta {
  display: flex;
  align-items: center;
  gap: 16px;
}

.meta-item {
  font-size: 12px;
}

.meta-label {
  color: var(--ordo-text-tertiary);
}

.meta-value {
  color: var(--ordo-text-primary);
  font-weight: 500;
}

.meta-badge {
  font-size: 10px;
  padding: 2px 8px;
  border-radius: 10px;
  text-transform: uppercase;
  font-weight: 600;
}

.meta-badge.protobuf {
  background: rgba(59, 130, 246, 0.2);
  color: #3b82f6;
}

.meta-badge.manual {
  background: rgba(34, 197, 94, 0.2);
  color: #22c55e;
}

.schema-tabs {
  display: flex;
  border-bottom: 1px solid var(--ordo-border-color);
}

.tab {
  flex: 1;
  padding: 10px 16px;
  background: transparent;
  border: none;
  color: var(--ordo-text-secondary);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s;
  border-bottom: 2px solid transparent;
}

.tab:hover {
  background: var(--ordo-bg-item-hover);
  color: var(--ordo-text-primary);
}

.tab.active {
  color: var(--ordo-primary-500);
  border-bottom-color: var(--ordo-primary-500);
}

.tab-content {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}

/* Fields Tab */
.empty-state {
  text-align: center;
  padding: 40px;
  color: var(--ordo-text-tertiary);
}

.add-btn,
.add-field-btn {
  padding: 8px 16px;
  background: var(--ordo-primary-500);
  color: #fff;
  border: none;
  border-radius: var(--ordo-radius-sm);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.15s;
}

.add-btn:hover,
.add-field-btn:hover {
  background: var(--ordo-primary-600);
}

.add-field-btn {
  margin-top: 12px;
  width: 100%;
  background: transparent;
  border: 1px dashed var(--ordo-border-color);
  color: var(--ordo-text-secondary);
}

.add-field-btn:hover {
  background: var(--ordo-bg-item-hover);
  border-color: var(--ordo-primary-500);
  color: var(--ordo-primary-500);
}

.field-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.field-item {
  background: var(--ordo-bg-item);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  overflow: hidden;
}

.field-item.editing {
  border-color: var(--ordo-primary-500);
}

.field-header {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  cursor: pointer;
}

.field-header:hover {
  background: var(--ordo-bg-item-hover);
}

.field-offset {
  font-family: var(--ordo-font-mono);
  font-size: 11px;
  color: var(--ordo-text-tertiary);
  min-width: 40px;
}

.field-name {
  flex: 1;
  font-weight: 500;
  font-size: 13px;
}

.field-type {
  font-family: var(--ordo-font-mono);
  font-size: 11px;
  padding: 2px 8px;
  background: var(--ordo-bg-tertiary);
  border-radius: var(--ordo-radius-xs);
  color: var(--ordo-primary-500);
}

.field-size {
  font-size: 11px;
  color: var(--ordo-text-tertiary);
}

.remove-btn {
  background: transparent;
  border: none;
  color: var(--ordo-text-tertiary);
  font-size: 16px;
  cursor: pointer;
  padding: 0 4px;
}

.remove-btn:hover {
  color: var(--ordo-error);
}

.field-edit {
  padding: 12px;
  background: var(--ordo-bg-tertiary);
  border-top: 1px solid var(--ordo-border-color);
}

.edit-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 8px;
}

.edit-row label {
  min-width: 80px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
}

.edit-row input,
.edit-row select {
  flex: 1;
  padding: 6px 10px;
  background: var(--ordo-bg-input);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  color: var(--ordo-text-primary);
  font-size: 12px;
}

.edit-row.checkbox label {
  display: flex;
  align-items: center;
  gap: 8px;
}

/* Import Tab */
.import-actions {
  margin-bottom: 12px;
}

.sample-btn {
  padding: 6px 12px;
  background: var(--ordo-bg-item);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  color: var(--ordo-text-secondary);
  font-size: 11px;
  cursor: pointer;
}

.sample-btn:hover {
  background: var(--ordo-bg-item-hover);
}

.proto-input {
  width: 100%;
  min-height: 200px;
  padding: 12px;
  background: var(--ordo-bg-editor);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  color: var(--ordo-text-primary);
  font-family: var(--ordo-font-mono);
  font-size: 12px;
  line-height: 1.5;
  resize: vertical;
}

.error-list {
  margin-top: 8px;
}

.error-item {
  padding: 8px;
  background: rgba(239, 68, 68, 0.1);
  border-left: 3px solid #ef4444;
  color: #ef4444;
  font-size: 12px;
  margin-bottom: 4px;
}

.parsed-schemas {
  margin-top: 16px;
}

.parsed-schemas h4 {
  font-size: 12px;
  margin-bottom: 8px;
  color: var(--ordo-text-secondary);
}

.parsed-schema-item {
  display: flex;
  justify-content: space-between;
  padding: 10px 12px;
  background: var(--ordo-bg-item);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  margin-bottom: 4px;
  cursor: pointer;
}

.parsed-schema-item:hover {
  background: var(--ordo-bg-item-hover);
  border-color: var(--ordo-primary-500);
}

.schema-fields {
  font-size: 11px;
  color: var(--ordo-text-tertiary);
}

.import-btn {
  margin-top: 16px;
  width: 100%;
  padding: 10px;
  background: var(--ordo-primary-500);
  color: #fff;
  border: none;
  border-radius: var(--ordo-radius-sm);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
}

.import-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.import-btn:hover:not(:disabled) {
  background: var(--ordo-primary-600);
}

/* Preview Tab */
.rust-code {
  padding: 16px;
  background: var(--ordo-bg-editor);
  border-radius: var(--ordo-radius-sm);
  font-family: var(--ordo-font-mono);
  font-size: 12px;
  line-height: 1.6;
  overflow-x: auto;
  white-space: pre;
  color: var(--ordo-text-primary);
}

.copy-btn {
  margin-top: 12px;
  padding: 8px 16px;
  background: var(--ordo-bg-item);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  color: var(--ordo-text-secondary);
  font-size: 12px;
  cursor: pointer;
}

.copy-btn:hover {
  background: var(--ordo-bg-item-hover);
  color: var(--ordo-text-primary);
}
</style>
