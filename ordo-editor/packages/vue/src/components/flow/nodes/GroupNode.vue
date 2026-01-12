<script setup lang="ts">
/**
 * GroupNode - Background container for organizing steps visually
 * 分组节点 - 用于可视化组织步骤的背景容器
 *
 * This is NOT a regular node - it's a background container that:
 * - Always stays at the bottom layer (lowest z-index)
 * - Has no connection handles
 * - Can be resized by dragging edges
 * - Other nodes can be placed "inside" it visually
 */
import { ref, computed, watch } from 'vue';
import { NodeResizer } from '@vue-flow/node-resizer';
import type { StepGroup } from '@ordo/editor-core';
import OrdoIcon from '../../icons/OrdoIcon.vue';
import { GROUP_COLORS } from '@ordo/editor-core';
import { useI18n } from '../../../locale';

export interface Props {
  id: string;
  data: {
    group?: StepGroup;
    groupId?: string;
    name?: string;
    description?: string;
    color?: string;
    stepIds?: string[];
  };
  selected?: boolean;
}

const props = defineProps<Props>();

const { t } = useI18n();

const emit = defineEmits<{
  update: [group: StepGroup];
  delete: [];
}>();

// Get group data (support both old and new format)
const groupData = computed(() => ({
  id: props.data?.groupId || props.data?.group?.id || props.id,
  name: props.data?.name || props.data?.group?.name || t('flow.group'),
  description: props.data?.description || props.data?.group?.description || '',
  color: props.data?.color || props.data?.group?.color || '#3c3c3c',
  stepIds: props.data?.stepIds || props.data?.group?.stepIds || [],
}));

// Editing state
const isEditing = ref(false);
const editName = ref(groupData.value.name);
const titleInput = ref<HTMLInputElement | null>(null);

// Group color
const groupColor = computed(() => {
  const color = groupData.value.color;
  if (color && color in GROUP_COLORS) {
    return GROUP_COLORS[color as keyof typeof GROUP_COLORS];
  }
  return color || '#3c3c3c';
});

// Step count
const stepCount = computed(() => groupData.value.stepIds.length);

// Watch for external name changes
watch(
  () => groupData.value.name,
  (newName) => {
    if (newName && !isEditing.value) {
      editName.value = newName;
    }
  }
);

// Start editing name
function startEdit(event: MouseEvent) {
  event.stopPropagation();
  editName.value = groupData.value.name;
  isEditing.value = true;
  // Focus input after Vue updates DOM
  setTimeout(() => {
    titleInput.value?.focus();
    titleInput.value?.select();
  }, 0);
}

// Save name
function saveName() {
  isEditing.value = false;
  if (editName.value.trim() && editName.value !== groupData.value.name) {
    const group = props.data?.group || {
      id: groupData.value.id,
      name: groupData.value.name,
      description: groupData.value.description,
      color: groupData.value.color,
      position: { x: 0, y: 0 },
      size: { width: 300, height: 200 },
      stepIds: groupData.value.stepIds,
    };
    emit('update', {
      ...group,
      name: editName.value.trim(),
    });
  }
}

// Handle key press in input
function handleKeydown(event: KeyboardEvent) {
  event.stopPropagation();
  if (event.key === 'Enter') {
    saveName();
  } else if (event.key === 'Escape') {
    isEditing.value = false;
    editName.value = groupData.value.name;
  }
}

// Delete group
function deleteGroup(event: MouseEvent) {
  event.stopPropagation();
  emit('delete');
}
</script>

<template>
  <div class="group-node-container" :class="{ selected }" :style="{ '--group-color': groupColor }">
    <!-- Resizer - only visible when selected -->
    <NodeResizer
      :min-width="200"
      :min-height="120"
      :is-visible="selected"
      color="transparent"
      :handle-style="{
        width: '8px',
        height: '8px',
        background: 'var(--group-color)',
        border: 'none',
        borderRadius: '2px',
      }"
      :line-style="{
        border: 'none',
      }"
    />

    <!-- Group Header Bar -->
    <div class="group-header">
      <span class="group-label" v-if="!isEditing" @dblclick="startEdit">
        {{ groupData.name }}
      </span>
      <input
        v-else
        ref="titleInput"
        v-model="editName"
        class="group-label-input"
        @blur="saveName"
        @keydown="handleKeydown"
        @click.stop
      />

      <span class="step-badge" v-if="stepCount > 0">{{ stepCount }}</span>

      <button
        v-if="selected"
        class="delete-btn"
        @click="deleteGroup"
        :title="t('flow.deleteGroup')"
      >
        <OrdoIcon name="delete" :size="12" />
      </button>
    </div>

    <!-- Group Body - transparent, just for visual -->
    <div class="group-body">
      <div class="drop-zone" v-if="stepCount === 0 && selected">
        <span>{{ t('flow.groupDropZone') }}</span>
      </div>
    </div>
  </div>
</template>

<style>
/* Import resizer styles globally */
@import '@vue-flow/node-resizer/dist/style.css';
</style>

<style scoped>
.group-node-container {
  width: 100%;
  height: 100%;
  background: color-mix(in srgb, var(--group-color) 8%, transparent);
  border: 1px dashed color-mix(in srgb, var(--group-color) 60%, transparent);
  border-radius: 6px;
  display: flex;
  flex-direction: column;
  pointer-events: all;
  /* Make sure group is always behind other nodes */
  z-index: -1 !important;
}

.group-node-container.selected {
  border-style: solid;
  border-color: var(--group-color);
  background: color-mix(in srgb, var(--group-color) 12%, transparent);
}

/* Header */
.group-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  background: color-mix(in srgb, var(--group-color) 25%, transparent);
  border-radius: 5px 5px 0 0;
  border-bottom: 1px solid color-mix(in srgb, var(--group-color) 30%, transparent);
  min-height: 28px;
}

.group-label {
  font-size: 11px;
  font-weight: 600;
  color: var(--ordo-text-secondary, #b0b0b0);
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  cursor: text;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.group-label-input {
  flex: 1;
  font-size: 11px;
  font-weight: 600;
  color: var(--ordo-text-primary, #e0e0e0);
  background: rgba(0, 0, 0, 0.3);
  border: 1px solid var(--group-color);
  border-radius: 3px;
  padding: 2px 6px;
  outline: none;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.step-badge {
  font-size: 9px;
  font-weight: 700;
  color: var(--ordo-text-tertiary, #888);
  background: rgba(0, 0, 0, 0.2);
  padding: 2px 6px;
  border-radius: 8px;
  min-width: 18px;
  text-align: center;
}

.delete-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  background: transparent;
  border: none;
  border-radius: 3px;
  color: var(--ordo-text-tertiary, #888);
  cursor: pointer;
  transition: all 0.15s;
  opacity: 0.6;
}

.delete-btn:hover {
  background: rgba(255, 255, 255, 0.1);
  color: var(--ordo-error, #f44336);
  opacity: 1;
}

/* Body */
.group-body {
  flex: 1;
  position: relative;
  min-height: 60px;
}

.drop-zone {
  position: absolute;
  inset: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px dashed color-mix(in srgb, var(--group-color) 40%, transparent);
  border-radius: 4px;
  font-size: 10px;
  color: var(--ordo-text-tertiary, #666);
  text-align: center;
  padding: 8px;
}
</style>
