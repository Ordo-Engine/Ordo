<script setup lang="ts">
import { ref, computed } from 'vue';
import type { ASTNode } from './types';

const props = defineProps<{
  node: ASTNode;
  depth?: number;
  highlightPath?: number[];
}>();

const depth = computed(() => props.depth ?? 0);
const isExpanded = ref(depth.value < 3); // Auto-expand first 3 levels

const hasChildren = computed(() => props.node.children && props.node.children.length > 0);

const nodeColor = computed(() => {
  switch (props.node.node_type) {
    case 'literal':
      return '#a6e3a1';
    case 'field':
      return '#89b4fa';
    case 'binary':
      return '#f9e2af';
    case 'unary':
      return '#fab387';
    case 'call':
      return '#cba6f7';
    case 'conditional':
      return '#f38ba8';
    case 'array':
      return '#94e2d5';
    case 'object':
      return '#eba0ac';
    case 'exists':
      return '#89dceb';
    case 'coalesce':
      return '#b4befe';
    default:
      return '#cdd6f4';
  }
});

const nodeIcon = computed(() => {
  switch (props.node.node_type) {
    case 'literal':
      return '📌';
    case 'field':
      return '📎';
    case 'binary':
      return '⚡';
    case 'unary':
      return '❗';
    case 'call':
      return '📞';
    case 'conditional':
      return '❓';
    case 'array':
      return '📋';
    case 'object':
      return '📦';
    case 'exists':
      return '🔍';
    case 'coalesce':
      return '🔀';
    default:
      return '•';
  }
});

function toggleExpand() {
  if (hasChildren.value) {
    isExpanded.value = !isExpanded.value;
  }
}
</script>

<template>
  <div class="ast-node" :style="{ '--indent': depth }">
    <div
      class="node-header"
      :class="{ expandable: hasChildren, expanded: isExpanded }"
      @click="toggleExpand"
    >
      <span v-if="hasChildren" class="expand-icon">
        {{ isExpanded ? '▼' : '▶' }}
      </span>
      <span v-else class="expand-placeholder" />
      <span class="node-icon">{{ nodeIcon }}</span>
      <span class="node-label" :style="{ color: nodeColor }">{{ node.label }}</span>
      <span class="node-type">({{ node.node_type }})</span>
      <span v-if="node.value" class="node-value">= {{ node.value }}</span>
    </div>

    <div v-if="isExpanded && hasChildren" class="node-children">
      <OrdoASTTree
        v-for="(child, index) in node.children"
        :key="index"
        :node="child"
        :depth="depth + 1"
      />
    </div>
  </div>
</template>

<style scoped>
.ast-node {
  font-family: 'JetBrains Mono', 'Fira Code', monospace;
  font-size: 12px;
}

.node-header {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  padding-left: calc(var(--indent) * 16px + 8px);
  border-radius: 4px;
  cursor: default;
  transition: background 0.15s;
}

.node-header.expandable {
  cursor: pointer;
}

.node-header:hover {
  background: rgba(137, 180, 250, 0.1);
}

.expand-icon {
  width: 12px;
  font-size: 10px;
  color: #6c7086;
  flex-shrink: 0;
}

.expand-placeholder {
  width: 12px;
  flex-shrink: 0;
}

.node-icon {
  font-size: 12px;
  flex-shrink: 0;
}

.node-label {
  font-weight: 500;
}

.node-type {
  color: #6c7086;
  font-size: 10px;
}

.node-value {
  color: #a6adc8;
  font-size: 11px;
  margin-left: auto;
}

.node-children {
  border-left: 1px dashed #45475a;
  margin-left: calc(var(--indent) * 16px + 14px);
}
</style>
