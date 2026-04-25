<script setup lang="ts">
/**
 * OrdoFlowToolbar - Flow editor toolbar
 * 流程编辑器工具栏
 */
import { ref } from 'vue';
import OrdoIcon from '../icons/OrdoIcon.vue';
import { useI18n } from '../../locale';
import type { LayoutDirection } from './utils/layout';

type NodeType = 'decision' | 'action' | 'terminal' | 'sub_rule';

export interface Props {
  edgeStyle: 'bezier' | 'step';
  layoutDirection: LayoutDirection;
  hasSelection: boolean;
}

defineProps<Props>();

const emit = defineEmits<{
  'add-node': [type: NodeType];
  'start-node-drag': [type: NodeType];
  'end-node-drag': [];
  'add-group': [];
  'delete-node': [];
  'auto-layout': [];
  'set-edge-style': [style: 'bezier' | 'step'];
  'set-layout-direction': [direction: LayoutDirection];
}>();

const { t } = useI18n();

const FLOW_NODE_DRAG_TYPE = 'application/x-ordo-flow-node';
const dragImageEl = ref<HTMLDivElement | null>(null);

function startNodeDrag(event: DragEvent, type: NodeType) {
  if (!event.dataTransfer) return;

  event.dataTransfer.effectAllowed = 'copy';
  event.dataTransfer.setData(FLOW_NODE_DRAG_TYPE, type);
  event.dataTransfer.setData('text/plain', type);
  if (dragImageEl.value) {
    event.dataTransfer.setDragImage(dragImageEl.value, 0, 0);
  }
  emit('start-node-drag', type);
}
</script>

<template>
  <div class="flow-toolbar">
    <div ref="dragImageEl" class="drag-image-placeholder" aria-hidden="true"></div>

    <!-- Add Node Group -->
    <div class="toolbar-group">
      <span class="toolbar-label">{{ t('flow.add') }}</span>
      <button
        class="toolbar-btn"
        :title="t('step.decision')"
        draggable="true"
        @click="emit('add-node', 'decision')"
        @dragstart="startNodeDrag($event, 'decision')"
        @dragend="emit('end-node-drag')"
      >
        <OrdoIcon name="decision" :size="16" class="icon-decision" />
        <span class="btn-text">{{ t('step.decision') }}</span>
      </button>
      <button
        class="toolbar-btn"
        :title="t('step.action')"
        draggable="true"
        @click="emit('add-node', 'action')"
        @dragstart="startNodeDrag($event, 'action')"
        @dragend="emit('end-node-drag')"
      >
        <OrdoIcon name="action" :size="16" class="icon-action" />
        <span class="btn-text">{{ t('step.action') }}</span>
      </button>
      <button
        class="toolbar-btn"
        :title="t('step.terminal')"
        draggable="true"
        @click="emit('add-node', 'terminal')"
        @dragstart="startNodeDrag($event, 'terminal')"
        @dragend="emit('end-node-drag')"
      >
        <OrdoIcon name="terminal" :size="16" class="icon-terminal" />
        <span class="btn-text">{{ t('step.terminal') }}</span>
      </button>
      <button
        class="toolbar-btn"
        :title="t('step.subRule')"
        draggable="true"
        @click="emit('add-node', 'sub_rule')"
        @dragstart="startNodeDrag($event, 'sub_rule')"
        @dragend="emit('end-node-drag')"
      >
        <OrdoIcon name="sub_rule" :size="16" class="icon-sub-rule" />
        <span class="btn-text">{{ t('step.subRule') }}</span>
      </button>

      <div class="toolbar-divider-v"></div>

      <button class="toolbar-btn" :title="t('flow.createGroup')" @click="emit('add-group')">
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          class="icon-group"
        >
          <rect x="3" y="3" width="18" height="18" rx="2" stroke-dasharray="4 2" />
          <path d="M8 12h8M12 8v8" />
        </svg>
        <span class="btn-text">{{ t('flow.group') }}</span>
      </button>
    </div>

    <div class="toolbar-divider"></div>

    <!-- Layout Group -->
    <div class="toolbar-group">
      <span class="toolbar-label">{{ t('flow.layout') }}</span>
      <button class="toolbar-btn" :title="t('flow.autoLayout')" @click="emit('auto-layout')">
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
        >
          <rect x="3" y="3" width="7" height="7" rx="1" />
          <rect x="14" y="3" width="7" height="7" rx="1" />
          <rect x="3" y="14" width="7" height="7" rx="1" />
          <rect x="14" y="14" width="7" height="7" rx="1" />
        </svg>
        <span class="btn-text">{{ t('flow.auto') }}</span>
      </button>

      <select
        class="toolbar-select"
        :value="layoutDirection"
        :title="t('flow.direction')"
        @change="
          emit(
            'set-layout-direction',
            ($event.target as HTMLSelectElement).value as LayoutDirection
          )
        "
      >
        <option value="LR">{{ t('flow.lr') }}</option>
        <option value="TB">{{ t('flow.tb') }}</option>
        <option value="RL">{{ t('flow.rl') }}</option>
        <option value="BT">{{ t('flow.bt') }}</option>
      </select>
    </div>

    <div class="toolbar-divider"></div>

    <!-- Edge Style Group -->
    <div class="toolbar-group">
      <span class="toolbar-label">{{ t('flow.edge') }}</span>
      <button
        class="toolbar-btn"
        :class="{ active: edgeStyle === 'bezier' }"
        :title="t('flow.bezier')"
        @click="emit('set-edge-style', 'bezier')"
      >
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
        >
          <path d="M4 20 Q 12 4, 20 20" />
        </svg>
        <span class="btn-text">{{ t('flow.bezier') }}</span>
      </button>
      <button
        class="toolbar-btn"
        :class="{ active: edgeStyle === 'step' }"
        :title="t('flow.step')"
        @click="emit('set-edge-style', 'step')"
      >
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
        >
          <path d="M4 4 L 4 12 L 20 12 L 20 20" />
        </svg>
        <span class="btn-text">{{ t('flow.step') }}</span>
      </button>
    </div>

    <div class="toolbar-spacer"></div>

    <!-- Actions Group -->
    <div class="toolbar-group">
      <button
        class="toolbar-btn danger"
        :disabled="!hasSelection"
        :title="t('flow.deleteSelected')"
        @click="emit('delete-node')"
      >
        <OrdoIcon name="delete" :size="16" />
        <span class="btn-text">{{ t('common.delete') }}</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.flow-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: var(--ordo-bg-panel);
  border-bottom: 1px solid var(--ordo-border-color);
  flex-shrink: 0;
}

.toolbar-group {
  display: flex;
  align-items: center;
  gap: 4px;
}

.toolbar-label {
  font-size: 10px;
  font-weight: 600;
  color: var(--ordo-text-tertiary);
  text-transform: uppercase;
  margin-right: 4px;
}

.toolbar-btn {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 6px 10px;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  background: var(--ordo-bg-item);
  color: var(--ordo-text-secondary);
  font-size: 11px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s;
}

.toolbar-btn:hover:not(:disabled) {
  background: var(--ordo-bg-item-hover);
  color: var(--ordo-text-primary);
  border-color: var(--ordo-text-tertiary);
}

.toolbar-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.toolbar-btn.active {
  background: var(--ordo-accent-bg);
  border-color: var(--ordo-accent);
  color: var(--ordo-accent);
}

.toolbar-btn.danger:hover:not(:disabled) {
  background: var(--ordo-error-bg, rgba(229, 20, 0, 0.1));
  color: var(--ordo-error);
  border-color: var(--ordo-error);
}

.btn-text {
  display: none;
}

@media (min-width: 768px) {
  .btn-text {
    display: inline;
  }
}

.toolbar-select {
  padding: 6px 8px;
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-sm);
  background: var(--ordo-bg-item);
  color: var(--ordo-text-secondary);
  font-size: 11px;
  cursor: pointer;
}

.toolbar-select:hover {
  border-color: var(--ordo-text-tertiary);
}

.toolbar-divider {
  width: 1px;
  height: 24px;
  background: var(--ordo-border-color);
  margin: 0 4px;
}

.toolbar-spacer {
  flex: 1;
}

.icon-decision {
  color: var(--ordo-node-decision);
}
.icon-action {
  color: var(--ordo-node-action);
}
.icon-terminal {
  color: var(--ordo-node-terminal);
}
.icon-sub-rule {
  color: var(--ordo-node-sub-rule);
}
.icon-group {
  color: var(--ordo-text-tertiary);
}

.toolbar-divider-v {
  width: 1px;
  height: 20px;
  background: var(--ordo-border-color);
  margin: 0 4px;
}

.drag-image-placeholder {
  position: fixed;
  top: -9999px;
  left: -9999px;
  width: 1px;
  height: 1px;
  opacity: 0;
  pointer-events: none;
}
</style>
